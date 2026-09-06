//! Small request-scoped handle used by adapters and executors.

use std::sync::Arc;

use lettuce_types::JobId;

#[derive(Debug, Clone)]
pub struct CancellationToken {
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    notification: Arc<tokio::sync::Notify>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationToken {
    #[must_use]
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            notification: Arc::new(tokio::sync::Notify::new()),
        }
    }

    pub fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Release);
        self.notification.notify_waiters();
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::Acquire)
    }

    pub async fn cancelled(&self) {
        let notified = self.notification.notified();
        tokio::pin!(notified);
        if self.is_cancelled() {
            return;
        }
        notified.await;
    }
}

#[derive(Debug, Clone)]
pub struct JobHandle {
    id: JobId,
    cancellation: CancellationToken,
}

impl JobHandle {
    #[must_use]
    pub fn new(id: JobId) -> Self {
        Self {
            id,
            cancellation: CancellationToken::new(),
        }
    }

    #[must_use]
    pub fn with_cancellation(id: JobId, cancellation: CancellationToken) -> Self {
        Self { id, cancellation }
    }

    #[must_use]
    pub const fn id(&self) -> JobId {
        self.id
    }

    #[must_use]
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    pub fn request_cancel(&self) {
        self.cancellation.cancel();
    }
}
