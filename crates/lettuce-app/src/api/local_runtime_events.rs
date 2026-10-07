use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
};

use lettuce_conversations::{InferenceOutcome, InferencePort, InferenceRequest, PortError};
use lettuce_jobs::handle::CancellationToken;
use lettuce_local_llm::generation::LlamaHostEvent;
use lettuce_types::{GenerationAttemptId, GenerationTurnId, JobId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum FlowId {
    Turn(GenerationTurnId),
    Job(JobId),
}

type Consumer = Arc<dyn Fn(LlamaHostEvent) + Send + Sync>;

struct Flow {
    consumer: Consumer,
    cancellation: CancellationToken,
    active: Mutex<bool>,
    parent: Option<Arc<Flow>>,
}

#[derive(Default)]
pub(crate) struct RuntimeEventRouter {
    flows: Mutex<HashMap<FlowId, Arc<Flow>>>,
    attempts: Mutex<HashMap<GenerationAttemptId, Arc<Flow>>>,
    global: Mutex<Option<Consumer>>,
}

impl std::fmt::Debug for RuntimeEventRouter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RuntimeEventRouter")
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(crate) struct FlowRegistration {
    router: Arc<RuntimeEventRouter>,
    id: FlowId,
    flow: Arc<Flow>,
    owned: bool,
}

impl Drop for FlowRegistration {
    fn drop(&mut self) {
        if !self.owned {
            return;
        }
        *lock(&self.flow.active) = false;
        let mut flows = lock(&self.router.flows);
        if flows
            .get(&self.id)
            .is_some_and(|flow| Arc::ptr_eq(flow, &self.flow))
        {
            flows.remove(&self.id);
        }
    }
}

pub(crate) struct AttemptRegistration {
    router: Arc<RuntimeEventRouter>,
    id: GenerationAttemptId,
    flow: Arc<Flow>,
}

impl Drop for AttemptRegistration {
    fn drop(&mut self) {
        *lock(&self.flow.active) = false;
        let mut attempts = lock(&self.router.attempts);
        if attempts
            .get(&self.id)
            .is_some_and(|flow| Arc::ptr_eq(flow, &self.flow))
        {
            attempts.remove(&self.id);
        }
    }
}

impl RuntimeEventRouter {
    pub(crate) fn set_global(&self, consumer: impl Fn(LlamaHostEvent) + Send + Sync + 'static) {
        *lock(&self.global) = Some(Arc::new(consumer));
    }

    pub(crate) fn register_flow(
        self: &Arc<Self>,
        id: FlowId,
        cancellation: CancellationToken,
        consumer: impl Fn(LlamaHostEvent) + Send + Sync + 'static,
    ) -> FlowRegistration {
        let flow = Arc::new(Flow {
            consumer: Arc::new(consumer),
            cancellation,
            active: Mutex::new(true),
            parent: None,
        });
        let mut flows = lock(&self.flows);
        if let Some(existing) = flows.get(&id) {
            return FlowRegistration {
                router: self.clone(),
                id,
                flow: existing.clone(),
                owned: false,
            };
        }
        flows.insert(id, flow.clone());
        FlowRegistration {
            router: self.clone(),
            id,
            flow,
            owned: true,
        }
    }

    pub(crate) fn register_attempt(
        self: &Arc<Self>,
        id: GenerationAttemptId,
        owner: Option<FlowId>,
    ) -> Option<AttemptRegistration> {
        let parent = owner.and_then(|id| lock(&self.flows).get(&id).cloned())?;
        let flow = Arc::new(Flow {
            consumer: parent.consumer.clone(),
            cancellation: parent.cancellation.clone(),
            active: Mutex::new(true),
            parent: Some(parent),
        });
        lock(&self.attempts).insert(id, flow.clone());
        Some(AttemptRegistration {
            router: self.clone(),
            id,
            flow,
        })
    }

    pub(crate) fn emit(&self, event: LlamaHostEvent) {
        let request_id = match &event {
            LlamaHostEvent::ModelLoadProgress(progress) => progress.request_id.as_deref(),
            LlamaHostEvent::Heartbeat { request_id, .. }
            | LlamaHostEvent::Notice { request_id, .. } => request_id.as_deref(),
            _ => None,
        };
        if let Some(id) = request_id.and_then(|id| id.parse().ok()) {
            let flow = lock(&self.attempts).get(&id).cloned();
            if let Some(flow) = flow {
                let active = lock(&flow.active);
                let parent_active = flow.parent.as_ref().map(|parent| lock(&parent.active));
                if *active
                    && parent_active.as_deref().is_none_or(|active| *active)
                    && !flow.cancellation.is_cancelled()
                {
                    (flow.consumer)(event);
                }
            }
            return;
        }
        if matches!(event, LlamaHostEvent::RuntimeReportUpdated { .. }) {
            let consumer = lock(&self.global).clone();
            if let Some(consumer) = consumer {
                consumer(event);
            }
        } else {
            tracing::debug!(?event, "unrouted local runtime event");
        }
    }

    fn owner(&self, request: &InferenceRequest) -> Option<FlowId> {
        let flows = lock(&self.flows);
        let turn = FlowId::Turn(request.turn_id);
        if flows.contains_key(&turn) {
            Some(turn)
        } else {
            request
                .cancellation
                .map(FlowId::Job)
                .filter(|id| flows.contains_key(id))
        }
    }
}

pub(crate) struct RoutedInference {
    pub(crate) inference: Arc<dyn InferencePort>,
    pub(crate) router: Arc<RuntimeEventRouter>,
}

#[async_trait::async_trait]
impl InferencePort for RoutedInference {
    async fn run(&self, request: InferenceRequest) -> Result<InferenceOutcome, PortError> {
        let _registration = self
            .router
            .register_attempt(request.attempt_id, self.router.owner(&request));
        self.inference.run(request).await
    }
}

fn notice_code(
    notice: lettuce_local_llm::generation::LlamaNotice,
) -> lettuce_contracts::RuntimeNoticeCode {
    match notice {
        lettuce_local_llm::generation::LlamaNotice::MtpDisabledForVision => {
            lettuce_contracts::RuntimeNoticeCode::MtpDisabledForVision
        }
        lettuce_local_llm::generation::LlamaNotice::KvCacheMovedToRam => {
            lettuce_contracts::RuntimeNoticeCode::KvCacheMovedToRam
        }
    }
}

impl super::ApiContext {
    pub(crate) fn register_runtime_turn_events(
        &self,
        turn_id: GenerationTurnId,
        cancellation: CancellationToken,
    ) -> FlowRegistration {
        let context = self.downgrade();
        self.backend().local_runtime_events().register_flow(
            FlowId::Turn(turn_id),
            cancellation,
            move |event| {
                let Some(context) = context.upgrade() else {
                    return;
                };
                if let LlamaHostEvent::Notice { notice, .. } = event {
                    context.live_generation_event(
                        turn_id,
                        lettuce_contracts::GenerationEvent::Notice {
                            turn_id: turn_id.to_string(),
                            code: notice_code(notice),
                        },
                    );
                }
            },
        )
    }

    pub(crate) fn register_runtime_job_events(
        &self,
        job_id: JobId,
        cancellation: CancellationToken,
    ) -> FlowRegistration {
        let context = self.downgrade();
        self.backend().local_runtime_events().register_flow(
            FlowId::Job(job_id),
            cancellation,
            move |event| {
                let Some(context) = context.upgrade() else {
                    return;
                };
                let event = match event {
                    LlamaHostEvent::Notice { notice, .. } => lettuce_contracts::JobEvent::Notice {
                        code: notice_code(notice),
                    },
                    LlamaHostEvent::Heartbeat { heartbeat, .. } => {
                        lettuce_contracts::JobEvent::Throughput {
                            tokens: heartbeat.tokens,
                            tokens_per_second: heartbeat.tokens_per_second,
                        }
                    }
                    _ => return,
                };
                context.jobs().deliver(job_id, event, false);
            },
        )
    }

    pub(crate) fn runtime_report_changed(&self, event: LlamaHostEvent) {
        use lettuce_models::ModelCatalog;
        let LlamaHostEvent::RuntimeReportUpdated { model_path } = event else {
            return;
        };
        let catalog = self.backend().database();
        let result = catalog.provider_accounts().and_then(|accounts| {
            let local = accounts
                .into_iter()
                .filter(|account| account.protocol == lettuce_models::ProviderProtocol::LlamaCpp)
                .map(|account| account.id)
                .collect::<std::collections::HashSet<_>>();
            catalog.model_profiles().map(|models| {
                models
                    .into_iter()
                    .filter(|model| {
                        local.contains(&model.provider_account_id)
                            && model.external_model_id == model_path
                    })
                    .map(|model| model.id.to_string())
                    .collect::<Vec<_>>()
            })
        });
        match result {
            Ok(model_ids) if !model_ids.is_empty() => {
                self.emit(lettuce_contracts::ApiEvent::LocalModelRuntimeReportChanged { model_ids });
            }
            Ok(_) => {}
            Err(error) => {
                tracing::error!(%error, "local runtime report event could not resolve models");
            }
        }
    }
}

#[cfg(test)]
#[path = "local_runtime_events_tests.rs"]
mod tests;
