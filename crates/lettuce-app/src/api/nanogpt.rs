use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

use async_trait::async_trait;
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_conversations::{InferenceOutcome, InferencePort, InferenceRequest, PortError};
use lettuce_image_generation::{
    ImageProviderError, ImageProviderPort, ProviderImageOutput, ProviderImageRequest,
};
use lettuce_providers::{NanoGptUsage, NanoGptUsageError, QuotaWindow};
use lettuce_types::ProviderAccountId;
use tokio::sync::watch;

use super::{ApiContext, context::WeakApiContext, error::api_error};

type UsageResult = Result<dto::NanoGptUsageView, ApiError>;
type UsageWatch = watch::Receiver<Option<UsageResult>>;

struct AccountCheck {
    checked: Option<tokio::time::Instant>,
    in_flight: bool,
    result: watch::Sender<Option<UsageResult>>,
}

#[derive(Default)]
pub(crate) struct QuotaState(Arc<Mutex<HashMap<ProviderAccountId, AccountCheck>>>);

pub(crate) struct QuotaClaim {
    state: Arc<Mutex<HashMap<ProviderAccountId, AccountCheck>>>,
    id: ProviderAccountId,
    result: watch::Sender<Option<UsageResult>>,
}

impl Drop for QuotaClaim {
    fn drop(&mut self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(check) = state.get_mut(&self.id) {
            check.checked = Some(tokio::time::Instant::now());
            check.in_flight = false;
        }
        if self.result.borrow().is_none() {
            self.result.send_replace(Some(Err(api_error(
                ApiErrorCode::Cancelled,
                "quota check was cancelled",
            ))));
        }
    }
}

impl QuotaState {
    fn begin(&self, id: ProviderAccountId, force: bool) -> (Option<QuotaClaim>, UsageWatch) {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let check = state.entry(id).or_insert_with(|| AccountCheck {
            checked: None,
            in_flight: false,
            result: watch::channel(None).0,
        });
        if check.in_flight
            || !force
                && check
                    .checked
                    .is_some_and(|checked| checked.elapsed() < std::time::Duration::from_secs(300))
        {
            return (None, check.result.subscribe());
        }
        check.in_flight = true;
        check.result = watch::channel(None).0;
        (
            Some(QuotaClaim {
                state: self.0.clone(),
                id,
                result: check.result.clone(),
            }),
            check.result.subscribe(),
        )
    }

    #[cfg(test)]
    pub(crate) fn claim(&self, id: ProviderAccountId, force: bool) -> Option<QuotaClaim> {
        self.begin(id, force).0
    }
}

fn usage_error(error: NanoGptUsageError) -> ApiError {
    use dto::ProviderQuotaFailure as Reason;
    let (reason, code, status, provider_message) = match error {
        NanoGptUsageError::WrongProvider => (
            Reason::WrongProvider,
            ApiErrorCode::InvalidInput,
            None,
            None,
        ),
        NanoGptUsageError::MissingApiKey => (
            Reason::MissingApiKey,
            ApiErrorCode::InvalidInput,
            None,
            None,
        ),
        NanoGptUsageError::CredentialsUnavailable => (
            Reason::CredentialsUnavailable,
            ApiErrorCode::Unavailable,
            None,
            None,
        ),
        NanoGptUsageError::Transport => (Reason::Transport, ApiErrorCode::Unavailable, None, None),
        NanoGptUsageError::Malformed => (Reason::Malformed, ApiErrorCode::Malformed, None, None),
        NanoGptUsageError::ProviderRejected { status, message } => (
            Reason::ProviderRejected,
            ApiErrorCode::Unavailable,
            Some(status),
            message,
        ),
    };
    ApiError {
        code,
        message: "provider quota check failed".into(),
        details: Some(dto::ApiErrorDetails::ProviderQuota {
            reason,
            status,
            provider_message,
        }),
    }
}

fn window_view(window: QuotaWindow) -> dto::NanoGptQuotaWindow {
    dto::NanoGptQuotaWindow {
        used: window.used,
        remaining: window.remaining,
        limit: window.limit,
        percent_used: window.percent_used,
        reset_at: window.reset_at,
        unit: window.unit,
    }
}

pub(super) fn warning(usage: &NanoGptUsage) -> Option<(String, u8, dto::ProviderQuotaLevel)> {
    let (kind, window) = usage
        .weekly
        .as_ref()
        .map(|window| ("weekly", window))
        .or_else(|| usage.daily.as_ref().map(|window| ("daily", window)))
        .or_else(|| usage.monthly.as_ref().map(|window| ("monthly", window)))?;
    let percent = match (window.used, window.limit) {
        (Some(used), Some(limit)) if limit > 0.0 => used / limit,
        _ => window.percent_used?,
    };
    let (threshold, level) = if percent >= 1.0 {
        (100, dto::ProviderQuotaLevel::Exhausted)
    } else if percent >= 0.9 {
        (90, dto::ProviderQuotaLevel::AlmostExhausted)
    } else if percent >= 0.75 {
        (75, dto::ProviderQuotaLevel::NearLimit)
    } else {
        return None;
    };
    let window = window
        .reset_at
        .clone()
        .or_else(|| usage.current_period_end.clone())
        .unwrap_or_else(|| kind.to_owned());
    Some((window, threshold, level))
}

async fn fetch(context: &ApiContext, id: ProviderAccountId) -> UsageResult {
    let account = super::providers::account(context, id.to_string()).await?;
    let providers = context.blocking(super::ollama::remote_providers).await?;
    let usage = providers
        .nanogpt_usage(&account)
        .await
        .map_err(usage_error)?;
    if let Some((window, threshold, level)) = warning(&usage) {
        let emitted = context
            .blocking(move |context| {
                context
                    .backend()
                    .database()
                    .record_provider_quota_warning(id, &window, threshold)
                    .map_err(super::provider_mutations::model_error)
            })
            .await?;
        if emitted {
            context.emit(dto::ApiEvent::ProviderQuota {
                account_id: id.to_string(),
                level,
            });
        }
    }
    Ok(dto::NanoGptUsageView {
        account_id: id.to_string(),
        account_label: account.label,
        active: usage.active,
        state: usage.state,
        weekly: usage.weekly.map(window_view),
        daily: usage.daily.map(window_view),
        monthly: usage.monthly.map(window_view),
        current_period_end: usage.current_period_end,
        grace_until: usage.grace_until,
        fetched_at: context.now().get(),
    })
}

fn spawn(context: &ApiContext, claim: QuotaClaim) {
    let context = context.clone();
    tokio::spawn(async move {
        let result = tokio::select! {
            biased;
            () = context.shutdown_token().cancelled() => Err(api_error(ApiErrorCode::Cancelled, "application is shutting down")),
            result = fetch(&context, claim.id) => result,
        };
        if let Err(error) = &result {
            tracing::warn!(code = ?error.code, "provider quota check failed");
        }
        claim.result.send_replace(Some(result));
    });
}

pub async fn provider_nanogpt_usage(
    context: &ApiContext,
    request: dto::ProviderNanoGptUsageRequest,
) -> UsageResult {
    if context.shutdown_token().is_cancelled() {
        return Err(api_error(
            ApiErrorCode::Cancelled,
            "application is shutting down",
        ));
    }
    let id = super::error::parse_id(&request.account_id, "account_id")?;
    let (claim, mut result) = context.quota().begin(id, true);
    if let Some(claim) = claim {
        spawn(context, claim);
    }
    loop {
        if let Some(result) = result.borrow_and_update().clone() {
            return result;
        }
        tokio::select! {
            () = context.shutdown_token().cancelled() => return Err(api_error(ApiErrorCode::Cancelled, "application is shutting down")),
            changed = result.changed() => if changed.is_err() { return Err(api_error(ApiErrorCode::Cancelled, "quota check ended")); },
        }
    }
}

fn completed(signal: &OnceLock<WeakApiContext>, kind: &str, id: ProviderAccountId) {
    if kind != "nanogpt" {
        return;
    }
    if let Some(context) = signal.get().and_then(WeakApiContext::upgrade) {
        if !context.shutdown_token().is_cancelled() {
            let (claim, _) = context.quota().begin(id, false);
            if let Some(claim) = claim {
                spawn(&context, claim);
            }
        }
    }
}

pub(crate) type QuotaSignal = Arc<OnceLock<WeakApiContext>>;

pub(crate) struct QuotaInference {
    pub inference: Arc<dyn InferencePort>,
    pub signal: QuotaSignal,
}

#[async_trait]
impl InferencePort for QuotaInference {
    async fn run(&self, request: InferenceRequest) -> Result<InferenceOutcome, PortError> {
        let profile = &request.profile.chat_profile;
        let kind = profile.provider_kind.clone();
        let id = profile.provider_account_id;
        let result = self.inference.run(request).await;
        if result.is_ok() {
            completed(&self.signal, &kind, id);
        }
        result
    }
}

pub(crate) struct QuotaImages {
    pub provider: Arc<dyn ImageProviderPort>,
    pub signal: QuotaSignal,
}

#[async_trait]
impl ImageProviderPort for QuotaImages {
    async fn generate(
        &self,
        request: ProviderImageRequest,
    ) -> Result<ProviderImageOutput, ImageProviderError> {
        let kind = request.account.provider_kind.clone();
        let id = request.account.id;
        let result = self.provider.generate(request).await;
        if result.is_ok() {
            completed(&self.signal, &kind, id);
        }
        result
    }
}
