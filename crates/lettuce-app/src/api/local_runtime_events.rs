use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, MutexGuard, Weak,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
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

#[derive(Clone, PartialEq, Eq)]
struct LoadProgress {
    stage: lettuce_contracts::ModelLoadStage,
    status: lettuce_contracts::ModelLoadStatus,
    percent: u8,
    model_name: String,
    gpus: Option<Vec<lettuce_contracts::ModelLoadGpuProgress>>,
}

impl From<&lettuce_local_llm::engine::ModelLoadProgress> for LoadProgress {
    fn from(progress: &lettuce_local_llm::engine::ModelLoadProgress) -> Self {
        use lettuce_local_llm::engine::{ModelLoadStage, ModelLoadStatus};
        Self {
            stage: match progress.stage {
                ModelLoadStage::GpuOffload => lettuce_contracts::ModelLoadStage::GpuOffload,
                ModelLoadStage::Cpu => lettuce_contracts::ModelLoadStage::Cpu,
                ModelLoadStage::CpuFallback => lettuce_contracts::ModelLoadStage::CpuFallback,
                ModelLoadStage::Finalizing => lettuce_contracts::ModelLoadStage::Finalizing,
            },
            status: match progress.status {
                ModelLoadStatus::Loading => lettuce_contracts::ModelLoadStatus::Loading,
                ModelLoadStatus::Retrying => lettuce_contracts::ModelLoadStatus::Retrying,
                ModelLoadStatus::Loaded => lettuce_contracts::ModelLoadStatus::Loaded,
                ModelLoadStatus::Failed => lettuce_contracts::ModelLoadStatus::Failed,
            },
            percent: progress.percent,
            model_name: progress.model_name.clone(),
            gpus: progress.gpus.as_ref().map(|gpus| {
                gpus.iter()
                    .map(|gpu| lettuce_contracts::ModelLoadGpuProgress {
                        label: gpu.label.clone(),
                        percent: gpu.percent,
                    })
                    .collect()
            }),
        }
    }
}

struct Flow {
    consumer: Consumer,
    cancellation: CancellationToken,
    active: AtomicBool,
    parent: Option<Arc<Flow>>,
    load_progress: Mutex<Option<LoadProgress>>,
    snapshot: Mutex<Option<(GenerationAttemptId, Weak<Flow>, Option<LoadProgress>)>>,
}

enum Dispatch {
    Flow(Weak<Flow>, LlamaHostEvent),
    Global(Consumer, LlamaHostEvent),
    Wake,
    #[cfg(test)]
    Barrier(mpsc::Sender<()>),
}

pub(crate) struct RuntimeEventRouter {
    flows: Mutex<HashMap<FlowId, Arc<Flow>>>,
    attempts: Mutex<HashMap<GenerationAttemptId, Arc<Flow>>>,
    global: Mutex<Option<Consumer>>,
    sender: Option<mpsc::SyncSender<Dispatch>>,
    active: Arc<AtomicBool>,
}

impl Default for RuntimeEventRouter {
    fn default() -> Self {
        let (sender, receiver) = mpsc::sync_channel::<Dispatch>(256);
        let active = Arc::new(AtomicBool::new(true));
        let worker_active = active.clone();
        let worker = std::thread::Builder::new()
            .name("lettuce-runtime-events".into())
            .spawn(move || {
                while let Ok(message) = receiver.recv() {
                    if !worker_active.load(Ordering::Acquire) {
                        break;
                    }
                    match message {
                        Dispatch::Flow(flow, event) => {
                            if let Some(flow) = flow.upgrade()
                                && flow.is_active()
                            {
                                (flow.consumer)(event);
                            }
                        }
                        Dispatch::Global(consumer, event) => consumer(event),
                        Dispatch::Wake => {}
                        #[cfg(test)]
                        Dispatch::Barrier(done) => {
                            let _ = done.send(());
                        }
                    }
                }
            });
        let sender = match worker {
            Ok(_) => Some(sender),
            Err(error) => {
                tracing::error!(%error, "local runtime event dispatcher could not start");
                None
            }
        };
        Self {
            flows: Mutex::new(HashMap::new()),
            attempts: Mutex::new(HashMap::new()),
            global: Mutex::new(None),
            sender,
            active,
        }
    }
}

impl Flow {
    fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
            && self.parent.as_ref().is_none_or(|parent| {
                parent.active.load(Ordering::Acquire)
                    && lock(&parent.snapshot)
                        .as_ref()
                        .is_some_and(|(_, current, _)| std::ptr::eq(current.as_ptr(), self))
            })
            && !self.cancellation.is_cancelled()
    }
}

impl Drop for RuntimeEventRouter {
    fn drop(&mut self) {
        self.shutdown();
    }
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
        self.flow.active.store(false, Ordering::Release);
        *lock(&self.flow.snapshot) = None;
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
        self.flow.active.store(false, Ordering::Release);
        if let Some(parent) = &self.flow.parent {
            let mut snapshot = lock(&parent.snapshot);
            if snapshot
                .as_ref()
                .is_some_and(|(_, current, _)| current.ptr_eq(&Arc::downgrade(&self.flow)))
            {
                *snapshot = None;
            }
        }
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
    fn enqueue(&self, message: Dispatch) {
        if self.active.load(Ordering::Acquire)
            && let Some(sender) = &self.sender
        {
            let _ = sender.try_send(message);
        }
    }

    pub(crate) fn shutdown(&self) {
        self.active.store(false, Ordering::Release);
        if let Some(sender) = &self.sender {
            let _ = sender.try_send(Dispatch::Wake);
        }
    }

    #[cfg(test)]
    pub(crate) fn flush(&self) {
        if self.active.load(Ordering::Acquire)
            && let Some(sender) = &self.sender
        {
            let (done, received) = mpsc::channel();
            sender
                .send(Dispatch::Barrier(done))
                .expect("event dispatcher");
            received
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("event dispatcher drain");
        }
    }

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
            active: AtomicBool::new(true),
            parent: None,
            load_progress: Mutex::new(None),
            snapshot: Mutex::new(None),
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
            active: AtomicBool::new(true),
            parent: Some(parent),
            load_progress: Mutex::new(None),
            snapshot: Mutex::new(None),
        });
        if let Some(parent) = &flow.parent {
            *lock(&parent.snapshot) = Some((id, Arc::downgrade(&flow), None));
        }
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
                if flow.is_active() {
                    if let LlamaHostEvent::ModelLoadProgress(progress) = &event {
                        let next = LoadProgress::from(progress);
                        let mut previous = lock(&flow.load_progress);
                        if previous.as_ref() == Some(&next) {
                            return;
                        }
                        *previous = Some(next.clone());
                        if let Some(parent) = &flow.parent {
                            let mut snapshot = lock(&parent.snapshot);
                            if let Some((_, current, value)) = snapshot.as_mut()
                                && current.ptr_eq(&Arc::downgrade(&flow))
                            {
                                *value = Some(next);
                            }
                        }
                    }
                    self.enqueue(Dispatch::Flow(Arc::downgrade(&flow), event));
                }
            }
            return;
        }
        if matches!(event, LlamaHostEvent::RuntimeReportUpdated { .. }) {
            let consumer = lock(&self.global).clone();
            if let Some(consumer) = consumer {
                self.enqueue(Dispatch::Global(consumer, event));
            }
        } else {
            tracing::debug!(?event, "unrouted local runtime event");
        }
    }

    pub(crate) fn fence(&self, id: GenerationAttemptId) -> crate::api::serial_events::Validity {
        let flow = lock(&self.attempts).get(&id).map(Arc::downgrade);
        let active = self.active.clone();
        Arc::new(move || {
            active.load(Ordering::Acquire)
                && flow
                    .as_ref()
                    .and_then(Weak::upgrade)
                    .is_some_and(|flow| flow.is_active())
        })
    }

    fn cached_load(
        &self,
        id: FlowId,
    ) -> Option<(LoadProgress, crate::api::serial_events::Validity)> {
        if !self.active.load(Ordering::Acquire) {
            return None;
        }
        let flow = lock(&self.flows).get(&id).cloned()?;
        if !flow.is_active() {
            return None;
        }
        let snapshot = lock(&flow.snapshot);
        let (_, current, progress) = snapshot.as_ref()?;
        let current = current.clone();
        let active = self.active.clone();
        let valid: crate::api::serial_events::Validity = Arc::new(move || {
            active.load(Ordering::Acquire) && current.upgrade().is_some_and(|flow| flow.is_active())
        });
        Some((progress.clone()?, valid))
    }

    pub(crate) fn generation_load(
        &self,
        turn_id: GenerationTurnId,
    ) -> Option<(
        lettuce_contracts::GenerationEvent,
        crate::api::serial_events::Validity,
    )> {
        let (progress, valid) = self.cached_load(FlowId::Turn(turn_id))?;
        Some((
            lettuce_contracts::GenerationEvent::ModelLoading {
                turn_id: turn_id.to_string(),
                stage: progress.stage,
                status: progress.status,
                percent: progress.percent,
                model_name: progress.model_name,
                gpus: progress.gpus,
            },
            valid,
        ))
    }

    pub(crate) fn job_load(
        &self,
        job_id: JobId,
    ) -> Option<(
        lettuce_contracts::JobEvent,
        crate::api::serial_events::Validity,
    )> {
        let (progress, valid) = self.cached_load(FlowId::Job(job_id))?;
        Some((
            lettuce_contracts::JobEvent::ModelLoading {
                stage: progress.stage,
                status: progress.status,
                percent: progress.percent,
                model_name: progress.model_name,
                gpus: progress.gpus,
            },
            valid,
        ))
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
                let request_id = match &event {
                    LlamaHostEvent::ModelLoadProgress(progress) => progress.request_id.as_deref(),
                    LlamaHostEvent::Notice { request_id, .. }
                    | LlamaHostEvent::Heartbeat { request_id, .. } => request_id.as_deref(),
                    _ => None,
                };
                let Some(attempt) = request_id.and_then(|id| id.parse().ok()) else {
                    return;
                };
                let valid = context.backend().local_runtime_events().fence(attempt);
                let event = match event {
                    LlamaHostEvent::ModelLoadProgress(progress) => {
                        let progress = LoadProgress::from(&progress);
                        lettuce_contracts::GenerationEvent::ModelLoading {
                            turn_id: turn_id.to_string(),
                            stage: progress.stage,
                            status: progress.status,
                            percent: progress.percent,
                            model_name: progress.model_name,
                            gpus: progress.gpus,
                        }
                    }
                    LlamaHostEvent::Notice { notice, .. } => {
                        lettuce_contracts::GenerationEvent::Notice {
                            turn_id: turn_id.to_string(),
                            code: notice_code(notice),
                        }
                    }
                    _ => return,
                };
                context.live_runtime_generation_event(turn_id, event, valid);
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
                let request_id = match &event {
                    LlamaHostEvent::ModelLoadProgress(progress) => progress.request_id.as_deref(),
                    LlamaHostEvent::Notice { request_id, .. }
                    | LlamaHostEvent::Heartbeat { request_id, .. } => request_id.as_deref(),
                    _ => None,
                };
                let Some(attempt) = request_id.and_then(|id| id.parse().ok()) else {
                    return;
                };
                let valid = context.backend().local_runtime_events().fence(attempt);
                let event = match event {
                    LlamaHostEvent::ModelLoadProgress(progress) => {
                        let progress = LoadProgress::from(&progress);
                        lettuce_contracts::JobEvent::ModelLoading {
                            stage: progress.stage,
                            status: progress.status,
                            percent: progress.percent,
                            model_name: progress.model_name,
                            gpus: progress.gpus,
                        }
                    }
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
                let current_event = event.clone();
                context.jobs().deliver_fenced(job_id, event, valid, || {
                    !matches!(
                        current_event,
                        lettuce_contracts::JobEvent::ModelLoading { .. }
                    ) || context
                        .backend()
                        .local_runtime_events()
                        .job_load(job_id)
                        .as_ref()
                        .is_some_and(|(load, _)| load == &current_event)
                });
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
                self.emit(
                    lettuce_contracts::ApiEvent::LocalModelRuntimeReportChanged { model_ids },
                );
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
