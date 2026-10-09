use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use async_trait::async_trait;
use lettuce_contracts as dto;
use lettuce_jobs::{JobKind, JobQuery, JobStore};
use lettuce_models::{ProviderAccount, ProviderProtocol};
use lettuce_providers::ProviderRequestError;
use lettuce_types::{JobId, PageRequest, RequestId};
use lettuce_usage::{
    JobUsageLedger, OpenRouterEndpointPricing, OpenRouterGenerationDetails, UsageCostLedger,
};

use super::tests::{Harness, RecordingStream, Reply, harness, launch, send};

struct Billing {
    missing: AtomicBool,
    calls: AtomicUsize,
    ids: Mutex<Vec<String>>,
    blocked: AtomicBool,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

impl Default for Billing {
    fn default() -> Self {
        Self {
            missing: AtomicBool::new(false),
            calls: AtomicUsize::new(0),
            ids: Mutex::new(Vec::new()),
            blocked: AtomicBool::new(false),
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
        }
    }
}

#[async_trait]
impl crate::OpenRouterBillingPort for Billing {
    async fn generation(
        &self,
        _: &ProviderAccount,
        id: &str,
    ) -> Result<Option<OpenRouterGenerationDetails>, ProviderRequestError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.ids.lock().expect("ids").push(id.to_owned());
        self.entered.notify_one();
        if self.blocked.load(Ordering::SeqCst) {
            self.release.notified().await;
        }
        if self.missing.load(Ordering::SeqCst) {
            return Ok(None);
        }
        Ok(Some(OpenRouterGenerationDetails {
            generation_id: id.to_owned(),
            model: "author/actual-model".into(),
            provider_name: Some("Routed".into()),
            native_prompt_tokens: Some(150),
            native_completion_tokens: Some(40),
            normalized_prompt_tokens: Some(120),
            normalized_completion_tokens: Some(30),
            native_cached_tokens: None,
            native_reasoning_tokens: None,
            total_cost: lettuce_conversations::ProviderReportedCost::new(0.3),
        }))
    }

    async fn endpoints(
        &self,
        _: &ProviderAccount,
        _: &str,
    ) -> Result<Vec<OpenRouterEndpointPricing>, ProviderRequestError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(vec![OpenRouterEndpointPricing {
            provider_name: "Routed".into(),
            provider_display_name: None,
            tag: Some("routed".into()),
            pricing: lettuce_usage::ModelPricing {
                prompt: "0.001".into(),
                completion: "0.002".into(),
                request: String::new(),
                image: String::new(),
                image_output: String::new(),
                web_search: String::new(),
                internal_reasoning: String::new(),
                input_cache_read: String::new(),
                input_cache_write: String::new(),
            },
        }])
    }
}

pub(super) fn ready_openrouter(h: &Harness) {
    use lettuce_models::{CapabilityStatus, ModelProfileRepository, ProviderAccountRepository};
    let database = h.context.backend().database();
    let id = crate::launch::tests::seed_model(
        database,
        ProviderProtocol::OpenAiCompatible,
        "openrouter",
    );
    let mut model = ModelProfileRepository::get(database, id)
        .expect("model")
        .expect("exists");
    let revision = model.revision;
    model.config.chat_parameters.temperature = None;
    model.config.capabilities.streaming = CapabilityStatus::Supported;
    let model =
        ModelProfileRepository::upsert(database, model, Some(revision)).expect("model ready");
    let mut account = ProviderAccountRepository::get(database, model.provider_account_id)
        .expect("account")
        .expect("exists");
    let revision = account.revision;
    account.api_key_ref = Some(lettuce_settings::SecretRef::new());
    ProviderAccountRepository::upsert(database, account, Some(revision))
        .expect("configured credential reference");
    crate::launch::tests::set_application_default_model(database, id);
}

async fn chat(billing: Arc<Billing>) -> (Harness, lettuce_usage::JobInferenceUsage) {
    let mut h = harness(Reply::Text("Priced reply."));
    h.context = h.context.clone().with_usage_billing(billing);
    ready_openrouter(&h);
    *h.provider.response_hook.lock().expect("response hook") = Some(Arc::new(|_, response| {
        response.usage = Some(lettuce_conversations::InferenceUsage {
            input_tokens: 120,
            output_tokens: 30,
            image_tokens: None,
            audio_tokens: None,
            total_tokens: Some(150),
            cached_input_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
            web_search_requests: None,
            provider_reported_cost: None,
        });
    }));
    let conversation = launch(&h, "billing-launch").await;
    let stream = Arc::new(RecordingStream::default());
    send(&h, &conversation, "billing-send", "Hello", stream.clone())
        .await
        .expect("send");
    let ran = super::ConversationGenerationWorker::new(h.context.clone())
        .run_once()
        .await
        .expect("reply");
    assert!(ran, "generation did not run: {:?}", stream.events());
    assert!(
        matches!(
            stream.events().last(),
            Some(dto::GenerationEvent::Completed { .. })
        ),
        "reply did not complete: {:?}",
        stream.events()
    );
    let owner = h.provider.requests.lock().expect("requests")[0]
        .cancellation
        .expect("owner");
    let dispatch = h
        .context
        .backend()
        .database()
        .job_usage(owner)
        .expect("dispatch")
        .remove(0);
    (h, dispatch)
}

async fn run(h: &Harness) {
    let runner = super::JobRunner::new(h.context.clone(), super::JobHandlers::standard());
    assert!(runner.run_once().await.expect("run"));
    runner.wait_idle().await;
}

fn automatic_job(h: &Harness) -> JobId {
    let jobs = h
        .context
        .backend()
        .database()
        .list(JobQuery {
            state: None,
            kind: Some(JobKind::Maintenance),
            subject: None,
            page: PageRequest::default(),
        })
        .expect("jobs");
    jobs.items
        .into_iter()
        .find(|job| job.subject.id.as_str() == "usage-cost-capture")
        .expect("automatic job")
        .id
}

#[tokio::test]
async fn automatic_capture_uses_the_stored_response_id_and_is_not_repeated() {
    let billing = Arc::new(Billing::default());
    let (h, dispatch) = chat(billing.clone()).await;
    run(&h).await;
    assert_eq!(*billing.ids.lock().expect("ids"), ["fake-response"]);
    assert_eq!(billing.calls.load(Ordering::SeqCst), 2);
    let cost = h
        .context
        .backend()
        .database()
        .get_job_cost(dispatch.id)
        .expect("cost")
        .expect("recorded");
    assert_eq!(cost.cost.total_cost, 0.3);
    super::usage_billing::recover_automatic(&h.context)
        .await
        .expect("recover missing intent");
    let runner = super::JobRunner::new(h.context.clone(), super::JobHandlers::standard());
    assert!(!runner.run_once().await.expect("idle"));
    assert_eq!(billing.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn missing_generation_fails_the_automatic_job_with_a_typed_reason() {
    let billing = Arc::new(Billing::default());
    billing.missing.store(true, Ordering::SeqCst);
    let (h, dispatch) = chat(billing).await;
    run(&h).await;
    let job = super::job_get(
        &h.context,
        dto::JobGetRequest {
            job_id: automatic_job(&h).to_string(),
        },
    )
    .await
    .expect("job");
    assert_eq!(job.state, dto::JobStateDto::Failed);
    assert_eq!(
        job.failure.expect("typed failure").reason,
        Some(dto::JobFailureReason::UsageBillingUnavailable)
    );
    assert!(
        h.context
            .backend()
            .database()
            .get_job_cost(dispatch.id)
            .expect("no cost")
            .is_none()
    );
}

#[tokio::test]
async fn recalculation_replays_and_keeps_a_cost_captured_by_another_job_immutable() {
    let billing = Arc::new(Billing::default());
    let (h, dispatch) = chat(billing.clone()).await;
    let request = dto::UsageRecalculateCostsRequest {
        client_operation_id: RequestId::new().to_string(),
    };
    let first = super::usage_recalculate_costs(&h.context, request.clone())
        .await
        .expect("admit recalculation");
    assert_eq!(
        super::usage_recalculate_costs(&h.context, request.clone())
            .await
            .expect("replay"),
        first
    );
    run(&h).await;
    run(&h).await;
    let cost = h
        .context
        .backend()
        .database()
        .get_job_cost(dispatch.id)
        .expect("cost")
        .expect("recorded");
    assert_eq!(billing.calls.load(Ordering::SeqCst), 2);
    let job = super::job_get(
        &h.context,
        dto::JobGetRequest {
            job_id: first.job_id.clone(),
        },
    )
    .await
    .expect("result");
    assert_eq!(job.state, dto::JobStateDto::Succeeded);
    assert_eq!(
        job.result,
        Some(dto::JobResultDto::UsageCostsUpdated {
            priced: 1,
            cleared: 0
        })
    );
    assert_eq!(
        super::usage_recalculate_costs(&h.context, request)
            .await
            .expect("completed replay"),
        first
    );
    let empty = super::usage_recalculate_costs(
        &h.context,
        dto::UsageRecalculateCostsRequest {
            client_operation_id: RequestId::new().to_string(),
        },
    )
    .await
    .expect("no missing costs");
    run(&h).await;
    assert_eq!(
        super::job_get(
            &h.context,
            dto::JobGetRequest {
                job_id: empty.job_id
            }
        )
        .await
        .expect("empty result")
        .result,
        Some(dto::JobResultDto::UsageCostsUpdated {
            priced: 0,
            cleared: 0
        })
    );
    let stored = h
        .context
        .backend()
        .database()
        .get_job_cost(dispatch.id)
        .expect("immutable")
        .expect("still stored");
    assert_eq!(stored.basis, cost.basis);
    assert_eq!(
        stored.cost.total_cost.to_bits(),
        cost.cost.total_cost.to_bits()
    );
    assert_eq!(billing.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn clear_during_capture_consumes_the_exact_tombstone_without_resurrection() {
    let billing = Arc::new(Billing::default());
    billing.blocked.store(true, Ordering::SeqCst);
    let (h, dispatch) = chat(billing.clone()).await;
    let runner = super::JobRunner::new(h.context.clone(), super::JobHandlers::standard());
    assert!(runner.run_once().await.expect("started capture"));
    billing.entered.notified().await;
    super::usage_clear_before(
        &h.context,
        dto::UsageClearBeforeRequest {
            before: h.context.now().get().saturating_add(1),
            client_operation_id: RequestId::new().to_string(),
        },
    )
    .await
    .expect("clear settled owner");
    billing.release.notify_one();
    runner.wait_idle().await;
    assert!(
        h.context
            .backend()
            .database()
            .get_job_cost(dispatch.id)
            .expect("cost")
            .is_none()
    );
    let job = super::job_get(
        &h.context,
        dto::JobGetRequest {
            job_id: automatic_job(&h).to_string(),
        },
    )
    .await
    .expect("job");
    assert_eq!(job.state, dto::JobStateDto::Succeeded);
    assert_eq!(
        job.result,
        Some(dto::JobResultDto::UsageCostsUpdated {
            priced: 0,
            cleared: 1
        })
    );
}

#[tokio::test]
async fn cancelling_network_capture_writes_no_cost_and_shutdown_drains_it() {
    let billing = Arc::new(Billing::default());
    billing.blocked.store(true, Ordering::SeqCst);
    let (h, dispatch) = chat(billing.clone()).await;
    let runner = super::JobRunner::new(h.context.clone(), super::JobHandlers::standard());
    assert!(runner.run_once().await.expect("started capture"));
    billing.entered.notified().await;
    super::job_cancel(
        &h.context,
        dto::JobCancelRequest {
            job_id: automatic_job(&h).to_string(),
        },
    )
    .await
    .expect("cancel");
    runner.wait_idle().await;
    assert!(
        h.context
            .backend()
            .database()
            .get_job_cost(dispatch.id)
            .expect("cost")
            .is_none()
    );
    assert_eq!(
        super::job_get(
            &h.context,
            dto::JobGetRequest {
                job_id: automatic_job(&h).to_string()
            }
        )
        .await
        .expect("job")
        .state,
        dto::JobStateDto::Cancelled
    );
    let pending = super::usage_recalculate_costs(
        &h.context,
        dto::UsageRecalculateCostsRequest {
            client_operation_id: RequestId::new().to_string(),
        },
    )
    .await
    .expect("retry missing");
    assert!(runner.run_once().await.expect("retry"));
    billing.entered.notified().await;
    h.context.begin_shutdown();
    runner.wait_idle().await;
    assert_eq!(
        super::job_get(
            &h.context,
            dto::JobGetRequest {
                job_id: pending.job_id
            }
        )
        .await
        .expect("shutdown job")
        .state,
        dto::JobStateDto::Cancelled
    );
    assert!(
        h.context
            .backend()
            .database()
            .get_job_cost(dispatch.id)
            .expect("no lost evidence")
            .is_none()
    );
}

#[tokio::test]
async fn a_second_runner_cannot_claim_the_active_billing_job() {
    let billing = Arc::new(Billing::default());
    billing.blocked.store(true, Ordering::SeqCst);
    let (h, _) = chat(billing.clone()).await;
    let first = super::JobRunner::new(h.context.clone(), super::JobHandlers::standard());
    let second = super::JobRunner::new(h.context.clone(), super::JobHandlers::standard());
    assert!(first.run_once().await.expect("first claim"));
    billing.entered.notified().await;
    assert!(!second.run_once().await.expect("already claimed"));
    billing.release.notify_one();
    first.wait_idle().await;
    second.wait_idle().await;
    assert_eq!(billing.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn restart_after_cost_write_replays_without_another_provider_lookup() {
    use lettuce_jobs::{JobMutation, JobState, ResourceAvailability, WorkerId};
    let billing = Arc::new(Billing::default());
    let (h, dispatch) = chat(billing.clone()).await;
    let database = h.context.backend().database();
    let job_id = automatic_job(&h);
    let claim = database
        .claim(
            job_id,
            WorkerId::new(),
            h.context.now(),
            std::time::Duration::from_secs(60),
            &ResourceAvailability::all(),
        )
        .expect("claim")
        .expect("claimed");
    database
        .append_and_transition(JobMutation::Start {
            claim: claim.claim,
            at: h.context.now(),
        })
        .expect("started");
    let original = h
        .context
        .backend()
        .usage_costs(billing.as_ref())
        .capture_job(dispatch.job_id, dispatch.id, h.context.now())
        .await
        .expect("capture")
        .expect("stored");
    h.context.recover_after_restart().expect("startup recovery");
    assert_eq!(
        database.get(job_id).expect("job").expect("exists").state,
        JobState::Queued
    );
    run(&h).await;
    assert_eq!(
        database.get(job_id).expect("job").expect("exists").state,
        JobState::Succeeded
    );
    assert_eq!(
        database
            .get_job_cost(dispatch.id)
            .expect("cost")
            .expect("exists")
            .basis,
        original.basis
    );
    assert_eq!(billing.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn recalculation_rejects_an_operation_key_with_a_different_digest() {
    use lettuce_jobs::{JobSpec, JobSubject, OutcomeRef, SubjectKind};
    let h = harness(Reply::Text("ok"));
    let id = RequestId::new();
    h.context
        .backend()
        .database()
        .admit_job_with_detail(
            JobSpec::new(
                JobKind::Maintenance,
                JobSubject::new(SubjectKind::Maintenance, "usage-recalculate-costs")
                    .expect("subject"),
                OutcomeRef::Request(id),
            )
            .with_resources(vec![lettuce_jobs::ResourceClass::Network]),
            &format!("usage-recalculate-costs:{id}"),
            "different-digest",
            &serde_json::json!({"targets": []}),
        )
        .expect("existing operation");
    let error = super::usage_recalculate_costs(
        &h.context,
        dto::UsageRecalculateCostsRequest {
            client_operation_id: id.to_string(),
        },
    )
    .await
    .expect_err("conflict");
    assert_eq!(error.code, dto::ApiErrorCode::Conflict);
}
