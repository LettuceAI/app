use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};

use lettuce_contracts::JobEvent;
use lettuce_jobs::handle::CancellationToken;
use lettuce_types::JobId;

use super::install::InstallWork;
use crate::api::events::JobEventSink;

/// One `job_watch` stream; its terminal event is sent at most once.
pub(crate) struct JobWatch {
    sink: Arc<dyn JobEventSink>,
    finished: AtomicBool,
}

impl JobWatch {
    fn send(&self, event: JobEvent) {
        if !self.finished.load(Ordering::Acquire) {
            self.sink.emit(event);
        }
    }

    fn finish(&self, event: JobEvent) {
        if !self.finished.swap(true, Ordering::AcqRel) {
            self.sink.emit(event);
        }
    }
}

/// The API's per-process job state: watch streams, the cancellation tokens
/// of running jobs, install work waiting for the runner, and the runner's
/// wake-up.
#[derive(Default)]
pub(crate) struct JobHostState {
    watches: Mutex<HashMap<JobId, Vec<Arc<JobWatch>>>>,
    running: Mutex<HashMap<JobId, CancellationToken>>,
    installs: Mutex<HashMap<JobId, InstallWork>>,
    wake: tokio::sync::Notify,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl JobHostState {
    pub(crate) fn wake(&self) {
        self.wake.notify_one();
    }

    pub(crate) async fn woken(&self) {
        self.wake.notified().await;
    }

    /// Runs `first` while holding the watch registry, so no delivery runs in
    /// between; `first` returns the event the new watch gets first and
    /// whether that event already ends it.
    pub(crate) fn watch<T, E>(
        &self,
        job_id: JobId,
        sink: Arc<dyn JobEventSink>,
        first: impl FnOnce() -> Result<(T, JobEvent, bool), E>,
    ) -> Result<T, E> {
        let mut watches = lock(&self.watches);
        let (value, event, terminal) = first()?;
        let watch = Arc::new(JobWatch {
            sink,
            finished: AtomicBool::new(false),
        });
        if terminal {
            watch.finish(event);
        } else {
            watch.send(event);
            watches.entry(job_id).or_default().push(watch);
        }
        Ok(value)
    }

    /// Sends a job's change to its watches; a terminal event ends them.
    pub(crate) fn deliver(&self, job_id: JobId, event: JobEvent, terminal: bool) {
        let mut watches = lock(&self.watches);
        if terminal {
            for watch in watches.remove(&job_id).unwrap_or_default() {
                watch.finish(event.clone());
            }
        } else if let Some(list) = watches.get(&job_id) {
            for watch in list {
                watch.send(event.clone());
            }
        }
    }

    pub(crate) fn watching(&self, job_id: JobId) -> bool {
        lock(&self.watches).contains_key(&job_id)
    }

    pub(crate) fn text_delta(
        &self,
        job_id: JobId,
        text: Option<String>,
        reasoning: Option<String>,
    ) {
        if let Some(list) = lock(&self.watches).get(&job_id) {
            for watch in list {
                watch.send(JobEvent::TextDelta {
                    text: text.clone(),
                    reasoning: reasoning.clone(),
                });
            }
        }
    }

    pub(crate) fn start_running(&self, job_id: JobId, cancellation: CancellationToken) {
        lock(&self.running).insert(job_id, cancellation);
    }

    pub(crate) fn finish_running(&self, job_id: JobId) {
        lock(&self.running).remove(&job_id);
    }

    /// Signals the job if this process runs it; returns whether it does.
    pub(crate) fn cancel_running(&self, job_id: JobId) -> bool {
        match lock(&self.running).get(&job_id) {
            Some(token) => {
                token.cancel();
                true
            }
            None => false,
        }
    }

    pub(crate) fn put_install(&self, job_id: JobId, work: InstallWork) {
        lock(&self.installs).insert(job_id, work);
    }

    pub(crate) fn install(&self, job_id: JobId) -> Option<InstallWork> {
        lock(&self.installs).get(&job_id).cloned()
    }

    pub(crate) fn has_install(&self, job_id: JobId) -> bool {
        lock(&self.installs).contains_key(&job_id)
    }

    pub(crate) fn forget_install(&self, job_id: JobId) {
        lock(&self.installs).remove(&job_id);
    }
}
