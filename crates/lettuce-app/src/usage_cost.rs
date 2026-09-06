use async_trait::async_trait;
use lettuce_models::{ProviderAccount, ProviderAccountRepository, ProviderProtocol};
use lettuce_providers::{ProviderRequestError, RemoteProviders};
use lettuce_settings::SecretStore;
use lettuce_types::{JobId, TimestampMillis, UsageEventId};
use lettuce_usage::{
    JobInferenceUsageResult, JobUsageLedger, OpenRouterEndpointPricing,
    OpenRouterGenerationDetails, UsageCost, UsageCostBasis, UsageCostLedger, UsageLedgerError,
};

#[async_trait]
pub trait OpenRouterBillingPort: Send + Sync {
    async fn generation(
        &self,
        account: &ProviderAccount,
        id: &str,
    ) -> Result<Option<OpenRouterGenerationDetails>, ProviderRequestError>;
    async fn endpoints(
        &self,
        account: &ProviderAccount,
        model: &str,
    ) -> Result<Vec<OpenRouterEndpointPricing>, ProviderRequestError>;
}

#[async_trait]
impl<S: SecretStore + ?Sized> OpenRouterBillingPort for RemoteProviders<S> {
    async fn generation(
        &self,
        account: &ProviderAccount,
        id: &str,
    ) -> Result<Option<OpenRouterGenerationDetails>, ProviderRequestError> {
        self.openrouter_generation_details(account, id).await
    }

    async fn endpoints(
        &self,
        account: &ProviderAccount,
        model: &str,
    ) -> Result<Vec<OpenRouterEndpointPricing>, ProviderRequestError> {
        self.openrouter_endpoint_pricing(account, model).await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UsageCostCaptureError {
    #[error(transparent)]
    Ledger(#[from] UsageLedgerError),
    #[error(transparent)]
    Provider(#[from] ProviderRequestError),
    #[error(transparent)]
    Account(#[from] lettuce_models::ModelRepositoryError),
}

#[derive(Debug)]
pub struct UsageCostCoordinator<'a, R: ?Sized, P: ?Sized> {
    repository: &'a R,
    provider: &'a P,
}

impl<'a, R, P> UsageCostCoordinator<'a, R, P>
where
    R: JobUsageLedger + UsageCostLedger + ProviderAccountRepository + ?Sized,
    P: OpenRouterBillingPort + ?Sized,
{
    pub fn new(repository: &'a R, provider: &'a P) -> Self {
        Self {
            repository,
            provider,
        }
    }

    pub async fn capture_job(
        &self,
        job_id: JobId,
        event_id: UsageEventId,
        captured_at: TimestampMillis,
    ) -> Result<Option<UsageCost>, UsageCostCaptureError> {
        let event = self
            .repository
            .job_usage(job_id)?
            .into_iter()
            .find(|event| event.id == event_id)
            .ok_or(UsageLedgerError::Invalid)?;
        if let Some(cost) = self.repository.get_job_cost(event_id)? {
            return Ok(Some(cost));
        }
        let Some(JobInferenceUsageResult::Response {
            usage: Some(_),
            provider_response_id: Some(response_id),
        }) = &event.result
        else {
            return Ok(None);
        };
        let Some(account) =
            ProviderAccountRepository::get(self.repository, event.provider_account_id)?
        else {
            return Ok(None);
        };
        if !account.enabled
            || !account.provider_kind.eq_ignore_ascii_case("openrouter")
            || account.protocol != ProviderProtocol::OpenAiCompatible
            || account.revision != event.provider_account_revision
        {
            return Ok(None);
        }
        let Some(generation) = self.provider.generation(&account, response_id).await? else {
            return Ok(None);
        };
        if generation.generation_id != *response_id {
            return Err(UsageLedgerError::Invalid.into());
        }
        let endpoints = self.provider.endpoints(&account, &generation.model).await?;
        let Some(basis) =
            UsageCostBasis::from_openrouter_job(&event, generation, &endpoints, captured_at)?
        else {
            return Ok(None);
        };
        match self.repository.record_job_cost(event_id, basis) {
            Ok(cost) => Ok(Some(cost)),
            Err(UsageLedgerError::Conflict) => self
                .repository
                .get_job_cost(event_id)?
                .map(Some)
                .ok_or_else(|| UsageLedgerError::Conflict.into()),
            Err(error) => Err(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_conversations::{InferenceUsage, ProviderReportedCost};
    use lettuce_database::Database;
    use lettuce_jobs::{JobKind, JobSpec, JobStore, JobSubject, OutcomeRef, SubjectKind};
    use lettuce_models::ProviderConfig;
    use lettuce_types::{GenerationAttemptId, ModelProfileId, ProviderAccountId, Revision};
    use lettuce_usage::{JobInferenceUsage, ModelPricing};
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    struct Billing {
        generation: Mutex<Result<Option<OpenRouterGenerationDetails>, ProviderRequestError>>,
        endpoints: Vec<OpenRouterEndpointPricing>,
        calls: AtomicUsize,
    }

    #[async_trait]
    impl OpenRouterBillingPort for Billing {
        async fn generation(
            &self,
            _: &ProviderAccount,
            id: &str,
        ) -> Result<Option<OpenRouterGenerationDetails>, ProviderRequestError> {
            assert_eq!(id, "gen-cost");
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.generation.lock().expect("generation lock").clone()
        }
        async fn endpoints(
            &self,
            _: &ProviderAccount,
            model: &str,
        ) -> Result<Vec<OpenRouterEndpointPricing>, ProviderRequestError> {
            assert_eq!(model, "author/actual-model");
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.endpoints.clone())
        }
    }

    fn fixture(database: &Database) -> (JobInferenceUsage, Billing) {
        let now = TimestampMillis::new(1);
        let account = ProviderAccountRepository::upsert(
            database,
            ProviderAccount {
                id: ProviderAccountId::new(),
                secret_owner_id: lettuce_settings::SecretOwnerId::new(),
                provider_kind: "openrouter".into(),
                protocol: ProviderProtocol::OpenAiCompatible,
                label: "Billing account".into(),
                endpoint: None,
                enabled: true,
                streaming_enabled: true,
                allow_invalid_tls: false,
                api_key_ref: None,
                secret_headers: Vec::new(),
                config: ProviderConfig::Standard,
                revision: Revision::INITIAL,
                created_at: now,
                updated_at: now,
            },
            None,
        )
        .expect("account");
        let job = database
            .create_or_get(
                JobSpec::new(
                    JobKind::ArtifactInstall,
                    JobSubject::new(SubjectKind::ArtifactInstall, "billing-fixture")
                        .expect("billing scenario"),
                    OutcomeRef::ArtifactInstallation(lettuce_types::AssetId::new()),
                )
                .with_resources(vec![lettuce_jobs::ResourceClass::Network]),
            )
            .expect("billing scenario")
            .job;
        let mut event = JobInferenceUsage {
            id: UsageEventId::new(),
            job_id: job.id,
            logical_attempt_id: GenerationAttemptId::new(),
            model_profile_id: ModelProfileId::new(),
            model_revision: Revision::INITIAL,
            provider_account_id: account.id,
            provider_account_revision: account.revision,
            admitted_at: now,
            result: None,
        };
        database
            .admit_job_usage(event.clone())
            .expect("billing scenario");
        let result = JobInferenceUsageResult::Response {
            provider_response_id: Some("gen-cost".into()),
            usage: Some(InferenceUsage {
                input_tokens: 120,
                output_tokens: 30,
                cached_input_tokens: Some(10),
                cache_write_tokens: Some(5),
                reasoning_tokens: Some(2),
                web_search_requests: Some(0),
                provider_reported_cost: ProviderReportedCost::new(0.2),
            }),
        };
        database
            .settle_job_usage(event.id, result.clone())
            .expect("billing scenario");
        event.result = Some(result);
        let pricing = ModelPricing {
            prompt: "0.001".into(),
            completion: "0.002".into(),
            request: String::new(),
            image: String::new(),
            image_output: String::new(),
            web_search: String::new(),
            internal_reasoning: String::new(),
            input_cache_read: "0.0001".into(),
            input_cache_write: String::new(),
        };
        let billing = Billing {
            generation: Mutex::new(Ok(Some(OpenRouterGenerationDetails {
                generation_id: "gen-cost".into(),
                model: "author/actual-model".into(),
                provider_name: Some("Routed".into()),
                native_prompt_tokens: Some(150),
                native_completion_tokens: Some(40),
                normalized_prompt_tokens: Some(120),
                normalized_completion_tokens: Some(30),
                native_cached_tokens: Some(20),
                native_reasoning_tokens: Some(3),
                total_cost: ProviderReportedCost::new(0.3),
            }))),
            endpoints: vec![
                OpenRouterEndpointPricing {
                    provider_name: "Unrelated".into(),
                    provider_display_name: None,
                    tag: Some("unrelated".into()),
                    pricing: pricing.clone(),
                },
                OpenRouterEndpointPricing {
                    provider_name: "Routed".into(),
                    provider_display_name: None,
                    tag: Some("routed/fp8".into()),
                    pricing,
                },
            ],
            calls: AtomicUsize::new(0),
        };
        (event, billing)
    }

    #[tokio::test]
    async fn routed_job_cost_retains_native_evidence_and_replays_after_reopen() {
        let path =
            std::env::temp_dir().join(format!("lettuce-billing-{}.sqlite", uuid::Uuid::new_v4()));
        let db = Database::open(&path).expect("billing scenario");
        let (event, billing) = fixture(&db);
        let old_basis = UsageCostBasis {
            model_profile_id: event.model_profile_id,
            provider_account_id: event.provider_account_id,
            source: "Old manual basis".into(),
            captured_at: TimestampMillis::new(1),
            pricing: billing.endpoints[1].pricing.clone(),
            input: lettuce_usage::OpenRouterCostInput::default(),
            openrouter: None,
        };
        let mut json = serde_json::to_value(&old_basis).expect("billing scenario");
        json.as_object_mut()
            .expect("billing scenario")
            .remove("openrouter");
        assert_eq!(
            serde_json::from_value::<UsageCostBasis>(json).expect("billing scenario"),
            old_basis
        );
        let cost = UsageCostCoordinator::new(&db, &billing)
            .capture_job(event.job_id, event.id, TimestampMillis::new(2))
            .await
            .expect("billing scenario")
            .expect("billing scenario");
        assert_eq!(cost.cost.total_cost, 0.3);
        assert_eq!(cost.basis.input.prompt_tokens, 150);
        assert_eq!(cost.basis.input.completion_tokens, 40);
        assert_eq!(cost.basis.input.cached_prompt_tokens, 20);
        assert_eq!(cost.basis.input.reasoning_tokens, 3);
        assert_eq!(
            cost.basis
                .openrouter
                .as_ref()
                .expect("billing scenario")
                .endpoint
                .tag
                .as_deref(),
            Some("routed/fp8")
        );
        assert_eq!(
            db.job_usage(event.job_id).expect("billing scenario"),
            std::slice::from_ref(&event)
        );
        let mut forged = cost.basis.clone();
        forged
            .openrouter
            .as_mut()
            .expect("billing scenario")
            .generation
            .generation_id = "gen-other".into();
        assert!(forged.calculate_job(&event).is_err());
        let mut wrong_price = cost.basis.clone();
        wrong_price.pricing.prompt = "1".into();
        assert!(wrong_price.calculate_job(&event).is_err());
        drop(db);
        let db = Database::open(&path).expect("billing scenario");
        *billing.generation.lock().expect("generation lock") =
            Err(ProviderRequestError::Unavailable);
        ProviderAccountRepository::delete(&db, event.provider_account_id)
            .expect("billing scenario");
        let replay = UsageCostCoordinator::new(&db, &billing)
            .capture_job(event.job_id, event.id, TimestampMillis::new(99))
            .await
            .expect("billing scenario")
            .expect("billing scenario");
        assert_eq!(replay.basis, cost.basis);
        assert_eq!(billing.calls.load(Ordering::SeqCst), 2);
        assert_eq!(
            db.job_usage(event.job_id).expect("billing scenario"),
            std::slice::from_ref(&event)
        );
        assert!(
            UsageCostCoordinator::new(&db, &billing)
                .capture_job(JobId::new(), event.id, TimestampMillis::new(99))
                .await
                .is_err()
        );
        drop(db);
        std::fs::remove_file(path).expect("billing scenario");
    }

    #[tokio::test]
    async fn unavailable_dispatches_and_changed_accounts_do_not_fetch_prices() {
        let db = Database::open_in_memory().expect("billing scenario");
        let (event, billing) = fixture(&db);
        for result in [
            None,
            Some(JobInferenceUsageResult::InferenceFailed),
            Some(JobInferenceUsageResult::Cancelled),
            Some(JobInferenceUsageResult::Response {
                usage: None,
                provider_response_id: Some("gen-cost".into()),
            }),
        ] {
            let mut pending = event.clone();
            pending.id = UsageEventId::new();
            pending.result = None;
            db.admit_job_usage(pending.clone())
                .expect("billing scenario");
            if let Some(result) = result {
                db.settle_job_usage(pending.id, result)
                    .expect("billing scenario");
            }
            assert!(
                UsageCostCoordinator::new(&db, &billing)
                    .capture_job(event.job_id, pending.id, TimestampMillis::new(2))
                    .await
                    .expect("billing scenario")
                    .is_none()
            );
        }
        let mut account = ProviderAccountRepository::get(&db, event.provider_account_id)
            .expect("billing scenario")
            .expect("billing scenario");
        let revision = account.revision;
        account.label = "Changed account".into();
        ProviderAccountRepository::upsert(&db, account, Some(revision)).expect("billing scenario");
        assert!(
            UsageCostCoordinator::new(&db, &billing)
                .capture_job(event.job_id, event.id, TimestampMillis::new(2))
                .await
                .expect("billing scenario")
                .is_none()
        );
        assert_eq!(billing.calls.load(Ordering::SeqCst), 0);
        assert!(
            db.get_job_cost(event.id)
                .expect("billing scenario")
                .is_none()
        );
    }

    #[tokio::test]
    async fn missing_or_ambiguous_billing_inputs_leave_no_cost_and_can_retry() {
        let db = Database::open_in_memory().expect("billing scenario");
        let (event, mut billing) = fixture(&db);
        let original = billing.generation.lock().expect("generation lock").clone();
        *billing.generation.lock().expect("generation lock") = Ok(None);
        assert!(
            UsageCostCoordinator::new(&db, &billing)
                .capture_job(event.job_id, event.id, TimestampMillis::new(2))
                .await
                .expect("billing scenario")
                .is_none()
        );
        *billing.generation.lock().expect("generation lock") =
            Err(ProviderRequestError::CredentialRejected);
        assert!(matches!(
            UsageCostCoordinator::new(&db, &billing)
                .capture_job(event.job_id, event.id, TimestampMillis::new(2))
                .await,
            Err(UsageCostCaptureError::Provider(
                ProviderRequestError::CredentialRejected
            ))
        ));
        *billing.generation.lock().expect("generation lock") = original.clone();
        billing.endpoints.push(billing.endpoints[1].clone());
        assert!(
            UsageCostCoordinator::new(&db, &billing)
                .capture_job(event.job_id, event.id, TimestampMillis::new(2))
                .await
                .expect("billing scenario")
                .is_none()
        );
        billing.endpoints.pop();
        let generation = original
            .expect("billing scenario")
            .expect("billing scenario");
        for field in 0..5 {
            let mut changed = generation.clone();
            match field {
                0 => changed.provider_name = None,
                1 => changed.provider_name = Some("Unknown".into()),
                2 => changed.native_prompt_tokens = None,
                3 => changed.native_completion_tokens = None,
                _ => changed.native_cached_tokens = None,
            }
            let mut record = event.clone();
            if let Some(JobInferenceUsageResult::Response {
                usage: Some(usage), ..
            }) = &mut record.result
            {
                if field == 4 {
                    usage.cached_input_tokens = None;
                }
            }
            assert!(
                UsageCostBasis::from_openrouter_job(
                    &record,
                    changed,
                    &billing.endpoints,
                    TimestampMillis::new(2)
                )
                .expect("billing scenario")
                .is_none()
            );
        }
        for field in 0..3 {
            let mut record = event.clone();
            let mut generation = generation.clone();
            if let Some(JobInferenceUsageResult::Response {
                usage: Some(usage), ..
            }) = &mut record.result
            {
                match field {
                    0 => usage.cache_write_tokens = None,
                    1 => usage.web_search_requests = None,
                    _ => {
                        usage.reasoning_tokens = None;
                        generation.native_reasoning_tokens = None;
                    }
                }
            }
            assert!(
                UsageCostBasis::from_openrouter_job(
                    &record,
                    generation,
                    &billing.endpoints,
                    TimestampMillis::new(2)
                )
                .expect("billing scenario")
                .is_none()
            );
        }
        assert!(
            db.get_job_cost(event.id)
                .expect("billing scenario")
                .is_none()
        );
        *billing.generation.lock().expect("generation lock") = Ok(Some(generation));
        assert!(
            UsageCostCoordinator::new(&db, &billing)
                .capture_job(event.job_id, event.id, TimestampMillis::new(3))
                .await
                .expect("billing scenario")
                .is_some()
        );
        assert_eq!(
            db.job_usage(event.job_id).expect("billing scenario"),
            [event]
        );
    }
}
