use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use futures_util::FutureExt;
use lettuce_contracts::{ApiError, ApiEvent};
use lettuce_conversations::ConversationChangeFeed;

use super::ApiContext;
use super::error::IntoApiError;

/// How long the feed gathers changes after the database signalled one,
/// which bounds the events one conversation sends to four a second.
pub(crate) const CONVERSATION_FEED_COALESCE: Duration = Duration::from_millis(250);
const FEED_PAGE: u32 = 500;

/// Follows the conversation change feed and publishes each changed
/// conversation once per read, as `ApiEvent::ConversationChanged` or, when a
/// purge removed it, `ApiEvent::ConversationRemoved`. It reads only after the
/// database signalled a committed conversation change.
pub(crate) struct ConversationFeed {
    position: u64,
    reads: Arc<AtomicUsize>,
}

impl ConversationFeed {
    /// Starts after every change recorded so far.
    pub(crate) async fn start(context: &ApiContext) -> Result<Self, ApiError> {
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
        })
    }

    /// How many times the feed has read changes.
    #[cfg(test)]
    pub(crate) fn reads(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.reads)
    }

    pub(crate) async fn run(mut self, context: ApiContext, shutdown: impl Future<Output = ()>) {
        let shutdown = shutdown.fuse();
        futures_util::pin_mut!(shutdown);
        loop {
            tokio::select! {
                () = &mut shutdown => break,
                () = context.conversations_changed() => {}
            }
            tokio::select! {
                () = &mut shutdown => break,
                () = tokio::time::sleep(CONVERSATION_FEED_COALESCE) => {}
            }
            if let Err(error) = self.publish(&context).await {
                tracing::warn!(code = ?error.code, message = %error.message, "conversation changes could not be published");
            }
        }
    }

    /// Publishes every change since the last call.
    pub(crate) async fn publish(&mut self, context: &ApiContext) -> Result<(), ApiError> {
        loop {
            let after = self.position;
            self.reads.fetch_add(1, Ordering::Relaxed);
            let changes = context
                .blocking(move |context| {
                    context
                        .backend()
                        .database()
                        .conversation_changes_since(after, FEED_PAGE)
                        .map_err(IntoApiError::into_api_error)
                })
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
