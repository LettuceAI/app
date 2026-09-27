use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use futures_util::FutureExt;
use lettuce_contracts::{ApiError, ApiEvent};
use lettuce_conversations::{ConversationChange, ConversationChangeFeed};

use super::ApiContext;
use super::error::IntoApiError;
use super::worker::{RETRY_MAX, RETRY_MIN};

/// How long the feed gathers changes after the database signalled one,
/// which bounds the events one conversation sends to four a second.
pub(crate) const CONVERSATION_FEED_COALESCE: Duration = Duration::from_millis(250);
const FEED_PAGE: u32 = 500;

type ChangeRead =
    dyn Fn(&ApiContext, u64, u32) -> Result<Vec<ConversationChange>, ApiError> + Send + Sync;

fn database_changes(
    context: &ApiContext,
    after: u64,
    limit: u32,
) -> Result<Vec<ConversationChange>, ApiError> {
    context
        .backend()
        .database()
        .conversation_changes_since(after, limit)
        .map_err(IntoApiError::into_api_error)
}

/// Follows the conversation change feed and publishes each changed
/// conversation once per read, as `ApiEvent::ConversationChanged` or, when a
/// purge removed it, `ApiEvent::ConversationRemoved`. It reads only after the
/// database signalled a committed conversation change, or to retry a read
/// that failed, after a delay that doubles from 250 ms to 30 s.
pub(crate) struct ConversationFeed {
    position: u64,
    reads: Arc<AtomicUsize>,
    read: Arc<ChangeRead>,
}

impl ConversationFeed {
    /// Starts after every change recorded so far.
    pub(crate) async fn start(context: &ApiContext) -> Result<Self, ApiError> {
        Self::start_with(context, Arc::new(database_changes)).await
    }

    async fn start_with(context: &ApiContext, read: Arc<ChangeRead>) -> Result<Self, ApiError> {
        let position = context
            .blocking(|context| {
                context
                    .backend()
                    .database()
                    .conversation_change_position()
                    .map_err(IntoApiError::into_api_error)
            })
            .await?;
        Ok(Self {
            position,
            reads: Arc::new(AtomicUsize::new(0)),
            read,
        })
    }

    /// A feed whose change reads go through `read`.
    #[cfg(test)]
    pub(crate) async fn start_reading(
        context: &ApiContext,
        read: Arc<ChangeRead>,
    ) -> Result<Self, ApiError> {
        Self::start_with(context, read).await
    }

    /// How many times the feed has read changes.
    #[cfg(test)]
    pub(crate) fn reads(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.reads)
    }

    pub(crate) async fn run(mut self, context: ApiContext, shutdown: impl Future<Output = ()>) {
        let shutdown = shutdown.fuse();
        futures_util::pin_mut!(shutdown);
        let mut retry: Option<Duration> = None;
        loop {
            match retry {
                None => tokio::select! {
                    () = &mut shutdown => break,
                    () = context.conversations_changed() => {}
                },
                Some(delay) => tokio::select! {
                    () = &mut shutdown => break,
                    () = tokio::time::sleep(delay) => {}
                },
            }
            tokio::select! {
                () = &mut shutdown => break,
                () = tokio::time::sleep(CONVERSATION_FEED_COALESCE) => {}
            }
            match self.publish(&context).await {
                Ok(()) => retry = None,
                Err(error) => {
                    let delay = retry.map_or(RETRY_MIN, |delay| (delay * 2).min(RETRY_MAX));
                    retry = Some(delay);
                    tracing::warn!(code = ?error.code, message = %error.message, ?delay, "conversation changes could not be published; retrying");
                }
            }
        }
    }

    /// Publishes every change since the last call.
    pub(crate) async fn publish(&mut self, context: &ApiContext) -> Result<(), ApiError> {
        loop {
            let after = self.position;
            self.reads.fetch_add(1, Ordering::Relaxed);
            let read = Arc::clone(&self.read);
            let changes = context
                .blocking(move |context| read(context, after, FEED_PAGE))
                .await?;
            let full = changes.len() >= FEED_PAGE as usize;
            for change in &changes {
                let conversation_id = change.conversation_id.to_string();
                context.emit(if change.removed {
                    ApiEvent::ConversationRemoved { conversation_id }
                } else {
                    ApiEvent::ConversationChanged { conversation_id }
                });
                self.position = change.position;
            }
            if !full || self.position == after {
                return Ok(());
            }
        }
    }
}
