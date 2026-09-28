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
    /// Whether the stream is still open afterwards.
    fn send(&self, event: JobEvent) -> bool {
        !self.finished.load(Ordering::Acquire) && self.sink.emit(event)
    }

    fn finish(&self, event: JobEvent) {
        if !self.finished.swap(true, Ordering::AcqRel) {
            self.sink.emit(event);
        }
    }
}

/// The API's per-process job state: watch streams, the cancellation tokens
/// of running jobs, install work waiting for the runner, the text a running
/// job streamed so far, the runner's wake-up and the signal of committed job
/// changes.
pub(crate) struct JobHostState {
    watches: Mutex<HashMap<JobId, Vec<Arc<JobWatch>>>>,
    running: Mutex<HashMap<JobId, CancellationToken>>,
    installs: Mutex<HashMap<JobId, InstallWork>>,
    streamed: Mutex<HashMap<JobId, (String, String)>>,
    wake: tokio::sync::Notify,
    changed: Arc<tokio::sync::Notify>,
}

impl Default for JobHostState {
    fn default() -> Self {
        Self {
            watches: Mutex::new(HashMap::new()),
            running: Mutex::new(HashMap::new()),
            installs: Mutex::new(HashMap::new()),
            streamed: Mutex::new(HashMap::new()),
            wake: tokio::sync::Notify::new(),
            changed: Arc::new(tokio::sync::Notify::new()),
        }
    }
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

    /// The signal the database raises after a committed job change.
    pub(crate) fn change_signal(&self) -> Arc<tokio::sync::Notify> {
        Arc::clone(&self.changed)
    }

    pub(crate) async fn changed(&self) {
        self.changed.notified().await;
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
        } else if watch.send(event) {
            let streamed = lock(&self.streamed).get(&job_id).cloned();
            let open = match streamed {
                Some((text, reasoning)) => watch.send(JobEvent::TextDelta {
                    text: (!text.is_empty()).then_some(text),
                    reasoning: (!reasoning.is_empty()).then_some(reasoning),
                }),
                None => true,
            };
            if open {
                watches.entry(job_id).or_default().push(watch);
            }
        }
        Ok(value)
    }

    /// Sends a job's change to its watches; a terminal event ends them, and
    /// a closed stream is dropped.
    pub(crate) fn deliver(&self, job_id: JobId, event: JobEvent, terminal: bool) {
        let mut watches = lock(&self.watches);
        if terminal {
            for watch in watches.remove(&job_id).unwrap_or_default() {
                watch.finish(event.clone());
            }
            return;
        }
        if let Some(list) = watches.get_mut(&job_id) {
            list.retain(|watch| watch.send(event.clone()));
            if list.is_empty() {
                watches.remove(&job_id);
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
        {
            let mut streamed = lock(&self.streamed);
            let so_far = streamed.entry(job_id).or_default();
            so_far.0.push_str(text.as_deref().unwrap_or_default());
            so_far.1.push_str(reasoning.as_deref().unwrap_or_default());
        }
        let mut watches = lock(&self.watches);
        if let Some(list) = watches.get_mut(&job_id) {
            list.retain(|watch| {
                watch.send(JobEvent::TextDelta {
                    text: text.clone(),
                    reasoning: reasoning.clone(),
                })
            });
            if list.is_empty() {
                watches.remove(&job_id);
            }
        }
    }

    pub(crate) fn start_running(&self, job_id: JobId, cancellation: CancellationToken) {
        lock(&self.running).insert(job_id, cancellation);
    }

    pub(crate) fn finish_running(&self, job_id: JobId) {
        lock(&self.running).remove(&job_id);
        lock(&self.streamed).remove(&job_id);
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

    /// The folders the installs this process holds write below.
    pub(crate) fn install_roots(&self) -> Vec<(JobId, std::path::PathBuf)> {
        lock(&self.installs)
            .iter()
            .map(|(job_id, work)| (*job_id, work.root().to_path_buf()))
            .collect()
    }

    pub(crate) fn has_install(&self, job_id: JobId) -> bool {
        lock(&self.installs).contains_key(&job_id)
    }

    pub(crate) fn forget_install(&self, job_id: JobId) {
        lock(&self.installs).remove(&job_id);
    }
}
