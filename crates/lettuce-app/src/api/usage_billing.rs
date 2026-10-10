use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_jobs::{
    CancellationPolicy, CancellationReason, ClaimRef, JobError, JobErrorCode, JobKind, JobMutation,
    JobOutcome, JobSnapshot, JobSpec, JobState, JobStore, JobSubject, OutcomeRef, RecoveryPolicy,
    ResourceAvailability, ResourceClass, SubjectKind, WorkerId, handle::CancellationToken,
};
use lettuce_types::{JobId, RequestId};
use lettuce_usage::{
    USAGE_AUTO_COST_KEY_PREFIX, UsageCostLedger, UsageCostScope, UsageCostTarget,
    UsageCostTargetReader, UsageLedgerError,
};

use super::{
    ApiContext,
    error::{IntoApiError, api_error, parse_id},
    jobs::{ClaimedJob, JobHandler, JobLane, JobProgressSink},
};

const AUTOMATIC: &str = "usage-cost-capture";
const RECALCULATE: &str = "usage-recalculate-costs";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BillingDetail {
    targets: Vec<UsageCostTarget>,
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BillingResult {
    priced: u64,
    cleared: u64,
}

fn storage(error: impl std::fmt::Display) -> ApiError {
    ApiError {
        code: ApiErrorCode::Unavailable,
        message: error.to_string(),
        details: Some(dto::ApiErrorDetails::UsageStorage),
    }
}

pub async fn usage_recalculate_costs(
    context: &ApiContext,
    request: dto::UsageRecalculateCostsRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let id: RequestId = parse_id(&request.client_operation_id, "client_operation_id")?;
    let key = format!("{RECALCULATE}:{id}");
    let job_id = context
        .blocking(move |context| {
            let targets = context
                .backend()
                .database()
                .missing_cost_targets(UsageCostScope::Recalculate)
                .map_err(storage)?;
            admit(context, RECALCULATE, &key, id, targets)
        })
        .await?;
    context.jobs().wake();
    Ok(dto::JobAccepted {
        job_id: job_id.to_string(),
    })
}

fn admit(
    context: &ApiContext,
    subject: &str,
    key: &str,
    request: RequestId,
    targets: Vec<UsageCostTarget>,
) -> Result<JobId, ApiError> {
    let spec = JobSpec::new(
        JobKind::Maintenance,
        JobSubject::new(SubjectKind::Maintenance, subject).map_err(storage)?,
        OutcomeRef::Request(request),
    )
    .with_resources(vec![ResourceClass::Network])
    .with_policies(RecoveryPolicy::Restart, CancellationPolicy::Cooperative);
    let digest = super::jobs::local::digest(&subject)?;
    let detail = serde_json::to_value(BillingDetail { targets }).map_err(storage)?;
    Ok(context
        .backend()
        .database()
        .admit_job_with_detail(spec, key, &digest, &detail)
        .map_err(IntoApiError::into_api_error)?
        .id)
}

pub(super) async fn admit_automatic(context: &ApiContext, job_id: JobId) -> Result<(), ApiError> {
    recover(context, Some(job_id)).await
}

pub(super) async fn recover_automatic(context: &ApiContext) -> Result<(), ApiError> {
    recover(context, None).await
}

/// Startup reconciliation is optional: a failure leaves a notice and
/// startup continues.
pub(super) async fn recover_at_startup(context: &ApiContext) {
    if let Err(error) = recover_automatic(context).await {
        skipped_at_startup(context, &error);
    }
}

pub(super) fn skipped_at_startup(context: &ApiContext, error: &ApiError) {
    tracing::warn!(code = ?error.code, message = %error.message, "usage cost capture recovery skipped at startup");
    if let Err(notice) = context
        .backend()
        .database()
        .record_usage_cost_capture_skipped(context.now())
    {
        tracing::error!(%notice, "the skipped cost capture notice could not be recorded");
    }
}

async fn recover(context: &ApiContext, job_id: Option<JobId>) -> Result<(), ApiError> {
    let created = context
        .blocking(move |context| {
            let targets = context
                .backend()
                .database()
                .missing_cost_targets(UsageCostScope::Automatic { job_id })
                .map_err(storage)?;
            let created = !targets.is_empty();
            for target in targets {
                let key = format!("{USAGE_AUTO_COST_KEY_PREFIX}{}", target.event_id);
                let request = RequestId::from_uuid(super::jobs::local::stable_uuid(&[
                    AUTOMATIC,
                    &target.event_id.to_string(),
                ]));
                admit(context, AUTOMATIC, &key, request, vec![target])?;
            }
            Ok(created)
        })
        .await?;
    if created {
        context.jobs().wake();
    }
    Ok(())
}

pub(super) fn result_view(
    context: &ApiContext,
    job: &JobSnapshot,
) -> Result<Option<dto::JobResultDto>, ApiError> {
    if job.kind != JobKind::Maintenance
        || !matches!(job.subject.id.as_str(), AUTOMATIC | RECALCULATE)
    {
        return Ok(None);
    }
    let record = context
        .backend()
        .database()
        .job_detail(job.id)
        .map_err(storage)?
        .ok_or_else(|| storage("usage cost job detail is missing"))?;
    serde_json::from_value::<BillingDetail>(record.detail).map_err(storage)?;
    if job.state != JobState::Succeeded {
        return Ok(None);
    }
    let result: BillingResult = serde_json::from_value(
        record
            .result
            .ok_or_else(|| storage("usage cost job result is missing"))?,
    )
    .map_err(storage)?;
    Ok(Some(dto::JobResultDto::UsageCostsUpdated {
        priced: result.priced,
        cleared: result.cleared,
    }))
}

pub(super) struct UsageBillingHandler;

#[async_trait]
impl JobHandler for UsageBillingHandler {
    fn kinds(&self) -> &[JobKind] {
        &[JobKind::Maintenance]
    }
    fn lane(&self, _context: &ApiContext, job: &JobSnapshot) -> Option<JobLane> {
        matches!(job.subject.id.as_str(), AUTOMATIC | RECALCULATE)
            .then(|| JobLane("usage-costs".into()))
    }

    async fn claim(
        &self,
        context: &ApiContext,
        job: &JobSnapshot,
        worker_id: WorkerId,
    ) -> Result<Option<Box<dyn ClaimedJob>>, ApiError> {
        let job = job.clone();
        context
            .blocking(move |context| {
                let database = context.backend().database();
                let detail = database
                    .job_detail(job.id)
                    .map_err(storage)?
                    .and_then(|record| serde_json::from_value::<BillingDetail>(record.detail).ok());
                let at = context.now().max(job.updated_at);
                let lease = Duration::from_millis(
                    u64::try_from(i64::MAX.saturating_sub(at.get())).map_err(storage)?,
                );
                let Some(claim) = database
                    .claim(job.id, worker_id, at, lease, &ResourceAvailability::all())
                    .map_err(IntoApiError::into_api_error)?
                else {
                    return Ok(None);
                };
                if let Err(error) = database.append_and_transition(JobMutation::Start {
                    claim: claim.claim.clone(),
                    at,
                }) {
                    if database
                        .get(job.id)
                        .map_err(IntoApiError::into_api_error)?
                        .is_some_and(|job| job.state == JobState::CancellationRequested)
                    {
                        crate::models::artifact_install::finish_claimed_cancellation(
                            database,
                            &claim.claim,
                            at,
                        )
                        .map_err(IntoApiError::into_api_error)?;
                        return Ok(None);
                    }
                    return Err(error.into_api_error());
                }
                let Some(detail) = detail else {
                    database
                        .append_and_transition(JobMutation::Fail {
                            claim: claim.claim,
                            error: job_error(
                                JobErrorCode::IntegrityFailure,
                                false,
                                "usage-cost-invalid",
                            ),
                            at,
                        })
                        .map_err(IntoApiError::into_api_error)?;
                    return Ok(None);
                };
                Ok(Some(Box::new(ClaimedBilling {
                    job_id: job.id,
                    claim: claim.claim,
                    input: claim.input_ref,
                    detail,
                    cancellation: CancellationToken::new(),
                }) as Box<dyn ClaimedJob>))
            })
            .await
    }
}

struct ClaimedBilling {
    job_id: JobId,
    claim: ClaimRef,
    input: OutcomeRef,
    detail: BillingDetail,
    cancellation: CancellationToken,
}

fn job_error(code: JobErrorCode, retryable: bool, label: &str) -> JobError {
    JobError::new(code, retryable, label).expect("usage cost failure label")
}

fn capture_error(error: crate::UsageCostCaptureError) -> JobError {
    use crate::UsageCostCaptureError;
    use lettuce_providers::ProviderRequestError;
    let (code, retryable, label) = match error {
        UsageCostCaptureError::Account(lettuce_models::ModelRepositoryError::NotFound) => (
            JobErrorCode::ResourceUnavailable,
            false,
            "usage-account-missing",
        ),
        UsageCostCaptureError::Account(_)
        | UsageCostCaptureError::Ledger(UsageLedgerError::Storage) => {
            (JobErrorCode::StorageFailure, true, "usage-cost-storage")
        }
        UsageCostCaptureError::Ledger(UsageLedgerError::Conflict) => {
            (JobErrorCode::IntegrityFailure, false, "usage-cost-conflict")
        }
        UsageCostCaptureError::Ledger(UsageLedgerError::Invalid) => {
            (JobErrorCode::IntegrityFailure, false, "usage-cost-invalid")
        }
        UsageCostCaptureError::Provider(ProviderRequestError::Unavailable) => (
            JobErrorCode::ResourceUnavailable,
            true,
            "usage-billing-unavailable",
        ),
        UsageCostCaptureError::Provider(ProviderRequestError::Malformed) => (
            JobErrorCode::IntegrityFailure,
            false,
            "usage-billing-malformed",
        ),
        UsageCostCaptureError::Provider(ProviderRequestError::CredentialRejected) => (
            JobErrorCode::Authentication,
            true,
            "usage-billing-credentials",
        ),
        UsageCostCaptureError::Provider(ProviderRequestError::Rejected) => {
            (JobErrorCode::InvalidInput, false, "usage-billing-rejected")
        }
        UsageCostCaptureError::Provider(ProviderRequestError::Unsupported) => (
            JobErrorCode::CapabilityUnavailable,
            false,
            "usage-billing-unsupported",
        ),
    };
    job_error(code, retryable, label)
}

enum BillingOutcome {
    Finished(BillingResult),
    Failed(JobError),
    Cancelled,
}

#[async_trait]
impl ClaimedJob for ClaimedBilling {
    fn cancellation(&self) -> CancellationToken {
        self.cancellation.clone()
    }
    async fn run(
        self: Box<Self>,
        context: ApiContext,
        _progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        let outcome = tokio::select! {
            biased;
            () = self.cancellation.cancelled() => BillingOutcome::Cancelled,
            result = self.capture(&context) => match result { Ok(result) => BillingOutcome::Finished(result), Err(error) if error.code == JobErrorCode::Cancelled => BillingOutcome::Cancelled, Err(error) => BillingOutcome::Failed(error) },
        };
        context
            .blocking(move |context| self.settle(context, outcome))
            .await
    }
}

impl ClaimedBilling {
    async fn capture(&self, context: &ApiContext) -> Result<BillingResult, JobError> {
        let database = context.backend().database();
        let mut result = BillingResult::default();
        let mut billing = None;
        for target in &self.detail.targets {
            if self.cancellation.is_cancelled() {
                return Err(job_error(
                    JobErrorCode::Cancelled,
                    false,
                    "usage-cost-cancelled",
                ));
            }
            if database
                .cleared_cost_target(target)
                .map_err(|error| capture_error(error.into()))?
            {
                result.cleared = result.cleared.checked_add(1).ok_or_else(|| {
                    job_error(JobErrorCode::IntegrityFailure, false, "usage-cost-invalid")
                })?;
                continue;
            }
            if database
                .get_job_cost(target.event_id)
                .map_err(|error| capture_error(error.into()))?
                .is_some()
            {
                result.priced = result.priced.checked_add(1).ok_or_else(|| {
                    job_error(JobErrorCode::IntegrityFailure, false, "usage-cost-invalid")
                })?;
                continue;
            }
            if billing.is_none() {
                billing = Some(context.usage_billing().map_err(|_| {
                    job_error(
                        JobErrorCode::ResourceUnavailable,
                        true,
                        "usage-billing-unavailable",
                    )
                })?);
            }
            let captured = context
                .backend()
                .usage_costs(billing.as_ref().expect("billing port").as_ref())
                .capture_job(target.job_id, target.event_id, context.now())
                .await;
            match captured {
                Ok(Some(_)) => {
                    result.priced = result.priced.checked_add(1).ok_or_else(|| {
                        job_error(JobErrorCode::IntegrityFailure, false, "usage-cost-invalid")
                    })?
                }
                outcome => {
                    if database
                        .cleared_cost_target(target)
                        .map_err(|error| capture_error(error.into()))?
                    {
                        result.cleared = result.cleared.checked_add(1).ok_or_else(|| {
                            job_error(JobErrorCode::IntegrityFailure, false, "usage-cost-invalid")
                        })?;
                    } else {
                        return Err(match outcome {
                            Err(error) => capture_error(error),
                            _ => job_error(
                                JobErrorCode::IntegrityFailure,
                                false,
                                "usage-cost-invalid",
                            ),
                        });
                    }
                }
            }
        }
        Ok(result)
    }

    fn settle(&self, context: &ApiContext, mut outcome: BillingOutcome) -> Result<(), ApiError> {
        let database = context.backend().database();
        let job = database
            .get(self.job_id)
            .map_err(IntoApiError::into_api_error)?
            .ok_or_else(|| api_error(ApiErrorCode::NotFound, "usage cost job was not found"))?;
        if job.state.is_terminal() {
            return Ok(());
        }
        let at = context.now().max(job.updated_at);
        if let BillingOutcome::Finished(result) = &outcome {
            let recorded = serde_json::to_value(result)
                .map_err(storage)
                .and_then(|result| {
                    database
                        .record_job_detail_result(self.job_id, &result)
                        .map_err(storage)
                });
            if !matches!(recorded, Ok(true)) {
                outcome = BillingOutcome::Failed(job_error(
                    JobErrorCode::StorageFailure,
                    true,
                    "usage-cost-storage",
                ));
            }
        }
        let mutation = match outcome {
            BillingOutcome::Finished(_) => JobMutation::Succeed {
                claim: self.claim.clone(),
                outcome: JobOutcome::Success {
                    result_ref: self.input.clone(),
                },
                at,
            },
            BillingOutcome::Failed(error) => JobMutation::Fail {
                claim: self.claim.clone(),
                error,
                at,
            },
            BillingOutcome::Cancelled => {
                if job.state != JobState::CancellationRequested {
                    database
                        .append_and_transition(JobMutation::RequestCancellation {
                            id: self.job_id,
                            reason: CancellationReason::Shutdown,
                            at,
                        })
                        .map_err(IntoApiError::into_api_error)?;
                }
                crate::models::artifact_install::finish_claimed_cancellation(
                    database,
                    &self.claim,
                    at,
                )
                .map_err(IntoApiError::into_api_error)?;
                return Ok(());
            }
        };
        database
            .append_and_transition(mutation)
            .map_err(IntoApiError::into_api_error)?;
        Ok(())
    }
}
