use std::{collections::HashMap, time::Duration};

use futures_util::FutureExt;
use lettuce_contracts::{ApiError, ApiEvent};
use lettuce_jobs::{JobCatalog, JobSnapshot, JobStore};
use lettuce_types::JobId;

use super::{job_event, job_view};
use crate::api::ApiContext;
use crate::api::error::IntoApiError;

/// How long the feed gathers changes after the database signalled one,
/// which bounds the updates one job sends to four a second.
pub(crate) const FEED_COALESCE: Duration = Duration::from_millis(250);
const FEED_PAGE: u32 = 500;

/// Follows the job store's change feed and publishes each changed job as
/// `ApiEvent::JobUpdated` and to its watch streams, once per read with its
/// latest state. It reads only after the database signalled a committed job
/// change.
pub(crate) struct JobFeed {
    position: u64,
    downloads: HashMap<JobId, (u64, i64)>,
}

impl JobFeed {
    /// Starts after every change recorded so far.
    pub(crate) async fn start(context: &ApiContext) -> Result<Self, ApiError> {
        let position = context
            .blocking(|context| {
                context
                    .backend()
                    .database()
                    .job_change_position()
                    .map_err(IntoApiError::into_api_error)
            })
            .await?;
        Ok(Self {
            position,
            downloads: HashMap::new(),
        })
    }

    pub(crate) async fn run(mut self, context: ApiContext, shutdown: impl Future<Output = ()>) {
        let shutdown = shutdown.fuse();
        futures_util::pin_mut!(shutdown);
        loop {
            if let Err(error) = self.publish(&context).await {
                tracing::warn!(code = ?error.code, message = %error.message, "job changes could not be published");
            }
            tokio::select! {
                () = &mut shutdown => break,
                () = context.jobs().changed() => {}
            }
            tokio::select! {
                () = &mut shutdown => break,
                () = tokio::time::sleep(FEED_COALESCE) => {}
            }
        }
    }

    /// Bytes a second since the job's previous published download progress.
    fn speed(&mut self, job: &JobSnapshot) -> Option<u64> {
        let Some(bytes) = job.progress.bytes.as_ref().filter(|_| !job.is_terminal()) else {
            self.downloads.remove(&job.id);
            return None;
        };
        let now = job.updated_at.get();
        let previous = self.downloads.insert(job.id, (bytes.completed, now));
        let (before, then) = previous?;
        let elapsed = u64::try_from(now.checked_sub(then)?)
            .ok()
            .filter(|ms| *ms > 0)?;
        Some(bytes.completed.checked_sub(before)?.saturating_mul(1000) / elapsed)
    }

    /// Publishes every change since the last call.
    pub(crate) async fn publish(&mut self, context: &ApiContext) -> Result<(), ApiError> {
        loop {
            let after = self.position;
            let (position, full, changed) = context
                .blocking(move |context| {
                    let database = context.backend().database();
                    let changes = database
                        .job_changes_since(after, FEED_PAGE)
                        .map_err(IntoApiError::into_api_error)?;
                    let position = changes.last().map_or(after, |change| change.position);
                    let mut changed = Vec::with_capacity(changes.len());
                    for change in &changes {
                        if let Some(job) = database
                            .get(change.job_id)
                            .map_err(IntoApiError::into_api_error)?
                        {
                            changed.push(job);
                        }
                    }
                    Ok((position, changes.len() >= FEED_PAGE as usize, changed))
                })
                .await?;
            self.position = position;
            for job in changed {
                let mut view = job_view(context, &job)?;
                view.progress.bytes_per_second = self.speed(&job);
                context.emit(ApiEvent::JobUpdated {
                    job: Box::new(view.clone()),
                });
                if context.jobs().watching(job.id) {
                    let (event, terminal) = job_event(&job, view);
                    context.jobs().deliver(job.id, event, terminal);
                }
            }
            if !full || position == after {
                return Ok(());
            }
        }
    }
}
