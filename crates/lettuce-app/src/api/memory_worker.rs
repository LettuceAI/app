use std::{collections::VecDeque, sync::Mutex, time::Duration};

use lettuce_contracts::ApiError;
use lettuce_jobs::WorkerId;
use lettuce_types::{ConversationBranchId, ConversationId};

use super::{
    ApiContext, MemoryJobOutput,
    worker::{WorkerStep, drive},
};

#[derive(Debug, Default)]
pub(super) struct MemoryWorkState {
    scheduler: crate::PostTurnMemoryScheduler,
    pending: Mutex<VecDeque<(ConversationId, ConversationBranchId)>>,
    wake: tokio::sync::Notify,
}

impl MemoryWorkState {
    pub(super) fn enqueue(&self, conversation: ConversationId, branch: ConversationBranchId) {
        if self.scheduler.enqueue(conversation, branch) {
            self.pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push_back((conversation, branch));
            self.wake.notify_one();
        }
    }

    fn pop(&self) -> Option<(ConversationId, ConversationBranchId)> {
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop_front()
    }
}

pub(super) struct MemoryWorker {
    context: ApiContext,
    worker_id: WorkerId,
}

impl MemoryWorker {
    pub(super) fn new(context: ApiContext) -> Self {
        Self {
            context,
            worker_id: WorkerId::new(),
        }
    }

    pub(super) async fn run(&self, shutdown: impl Future<Output = ()>) {
        drive(self, shutdown).await;
    }
}

impl WorkerStep for MemoryWorker {
    const LABEL: &'static str = "post-turn-memory";

    async fn step(&self) -> Result<bool, ApiError> {
        if !self.context.accepts_database_work()? {
            return Ok(false);
        }
        let Some(_work) = self
            .context
            .maintenance()
            .work(self.context.shutdown_token())
            .await
        else {
            return Ok(false);
        };
        let state = self.context.memory_work();
        let Some((conversation, branch)) = state.pop() else {
            return Ok(false);
        };
        let embedding = self.context.embedding();
        let output = MemoryJobOutput::new(self.context.clone());
        let backend = self.context.backend();
        backend
            .companion_memory_host(embedding.as_ref(), self.context.inference())
            .with_inference_runtime(backend.inference_runtime())
            .with_job_output(&output)
            .drive(
                &state.scheduler,
                conversation,
                branch,
                self.worker_id,
                Duration::from_secs(60 * 60),
                self.context.clock(),
                &crate::CompanionFollowUpHost::new(backend.database(), self.context.inference()),
            )
            .await;
        Ok(true)
    }

    async fn woken(&self) {
        self.context.memory_work().wake.notified().await;
    }
}
