use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
};

use lettuce_contracts::{ImageProgress, JobEvent};
use lettuce_jobs::handle::CancellationToken;
use lettuce_types::JobId;

use super::install::InstallWork;
use crate::api::events::JobEventSink;

/// One `job_watch` stream; its terminal event is sent at most once.
pub(crate) struct JobWatch {
    delivery: crate::api::serial_events::SerialEvents<JobEvent>,
}

impl JobWatch {
    /// Whether the stream is still open afterwards.
    fn send(&self, event: JobEvent) -> bool {
        self.delivery.send(event)
    }

    fn finish(&self, event: JobEvent) {
        self.delivery.finish(event);
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
    image: Mutex<HashMap<JobId, ImageProgress>>,
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
            image: Mutex::new(HashMap::new()),
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
    #[cfg(test)]
    pub(crate) fn watch<T, E>(
        &self,
        job_id: JobId,
        sink: Arc<dyn JobEventSink>,
        first: impl FnOnce() -> Result<(T, JobEvent, bool), E>,
    ) -> Result<T, E> {
        self.watch_with_load(job_id, sink, first, || None)
    }

    pub(crate) fn watch_with_load<T, E>(
        &self,
        job_id: JobId,
        sink: Arc<dyn JobEventSink>,
        first: impl FnOnce() -> Result<(T, JobEvent, bool), E>,
        load: impl FnOnce() -> Option<(JobEvent, crate::api::serial_events::Validity)>,
    ) -> Result<T, E> {
        let (value, watch, replay) = {
            let mut watches = lock(&self.watches);
            let (value, event, terminal) = first()?;
            let watch = Arc::new(JobWatch {
                delivery: crate::api::serial_events::SerialEvents::new(move |event| {
                    sink.emit(event)
                }),
            });
            let mut replay = vec![event];
            if !terminal {
                if let Some((text, reasoning)) = lock(&self.streamed).get(&job_id).cloned() {
                    replay.push(JobEvent::TextDelta {
                        text: (!text.is_empty()).then_some(text),
                        reasoning: (!reasoning.is_empty()).then_some(reasoning),
                    });
                }
                if let Some(progress) = lock(&self.image).get(&job_id).cloned() {
                    replay.push(JobEvent::ImageProgress { progress });
                }
                if let Some((event, valid)) = load() {
                    watch.delivery.send_if(event, valid);
                }
                watches.entry(job_id).or_default().push(watch.clone());
            }
            (value, watch, replay)
        };
        watch.delivery.initialize(replay);
        if !watch.delivery.is_open() {
            let mut watches = lock(&self.watches);
            if let Some(list) = watches.get_mut(&job_id) {
                list.retain(|existing| !Arc::ptr_eq(existing, &watch));
                if list.is_empty() {
                    watches.remove(&job_id);
                }
            }
        }
        Ok(value)
    }

    pub(crate) fn deliver(&self, job_id: JobId, event: JobEvent, terminal: bool) {
        let watches = {
            let mut registry = lock(&self.watches);
            if terminal {
                registry.remove(&job_id).unwrap_or_default()
            } else {
                registry.get(&job_id).cloned().unwrap_or_default()
            }
        };
        self.send_watches(job_id, watches, event, terminal);
    }

    pub(crate) fn deliver_fenced(
        &self,
        job_id: JobId,
        event: JobEvent,
        valid: crate::api::serial_events::Validity,
        current: impl FnOnce() -> bool,
    ) {
        let watches = {
            let watches = lock(&self.watches);
            if !current() || !valid() {
                return;
            }
            watches.get(&job_id).cloned().unwrap_or_default()
        };
        for watch in watches {
            watch.delivery.send_if(event.clone(), valid.clone());
        }
    }

    fn send_watches(
        &self,
        job_id: JobId,
        watches: Vec<Arc<JobWatch>>,
        event: JobEvent,
        terminal: bool,
    ) {
        let mut closed = Vec::new();
        for watch in watches {
            if terminal {
                watch.finish(event.clone());
            } else if !watch.send(event.clone()) {
                closed.push(watch);
            }
        }
        if !closed.is_empty() {
            let mut registry = lock(&self.watches);
            if let Some(watches) = registry.get_mut(&job_id) {
                watches.retain(|watch| !closed.iter().any(|closed| Arc::ptr_eq(watch, closed)));
                if watches.is_empty() {
                    registry.remove(&job_id);
                }
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
        self.text_delta_inner(job_id, text, reasoning, || {});
    }

    fn text_delta_inner(
        &self,
        job_id: JobId,
        text: Option<String>,
        reasoning: Option<String>,
        appended: impl FnOnce(),
    ) {
        let watches = {
            let watches = lock(&self.watches);
            {
                let mut streamed = lock(&self.streamed);
                let so_far = streamed.entry(job_id).or_default();
                so_far.0.push_str(text.as_deref().unwrap_or_default());
                so_far.1.push_str(reasoning.as_deref().unwrap_or_default());
            }
            appended();
            watches.get(&job_id).cloned().unwrap_or_default()
        };
        self.send_watches(
            job_id,
            watches,
            JobEvent::TextDelta { text, reasoning },
            false,
        );
    }

    pub(crate) fn image_progress(&self, job_id: JobId, progress: ImageProgress) {
        let watches = {
            let watches = lock(&self.watches);
            lock(&self.image).insert(job_id, progress.clone());
            watches.get(&job_id).cloned().unwrap_or_default()
        };
        self.send_watches(job_id, watches, JobEvent::ImageProgress { progress }, false);
    }

    pub(crate) fn start_running(&self, job_id: JobId, cancellation: CancellationToken) {
        lock(&self.running).insert(job_id, cancellation);
    }

    pub(crate) fn finish_running(&self, job_id: JobId) {
        lock(&self.running).remove(&job_id);
        lock(&self.streamed).remove(&job_id);
        lock(&self.image).remove(&job_id);
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

    /// Every install this process holds for the runner.
    pub(crate) fn installs(&self) -> Vec<(JobId, InstallWork)> {
        lock(&self.installs)
            .iter()
            .map(|(job_id, work)| (*job_id, work.clone()))
            .collect()
    }

    pub(crate) fn has_install(&self, job_id: JobId) -> bool {
        lock(&self.installs).contains_key(&job_id)
    }

    pub(crate) fn forget_install(&self, job_id: JobId) {
        lock(&self.installs).remove(&job_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Sink(Mutex<Vec<JobEvent>>);

    impl JobEventSink for Sink {
        fn emit(&self, event: JobEvent) -> bool {
            lock(&self.0).push(event);
            true
        }
    }

    #[test]
    fn text_append_keeps_the_watch_registry_locked_until_delivery() {
        let state = JobHostState::default();
        let job = JobId::new();
        state.text_delta_inner(job, Some("one".into()), None, || {
            assert!(
                matches!(
                    state.watches.try_lock(),
                    Err(std::sync::TryLockError::WouldBlock)
                ),
                "watch admission cannot observe appended text before live delivery"
            );
            assert_eq!(lock(&state.streamed).get(&job).expect("appended").0, "one");
        });
    }

    #[test]
    fn concurrent_watch_catch_up_delivers_each_delta_once() {
        for _ in 0..256 {
            let state = JobHostState::default();
            let job = JobId::new();
            let sink = Arc::new(Sink::default());
            let barrier = std::sync::Barrier::new(2);
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    barrier.wait();
                    state.text_delta(job, Some("one".into()), None);
                });
                barrier.wait();
                state
                    .watch(job, sink.clone(), || {
                        Ok::<_, ()>((
                            (),
                            JobEvent::TextDelta {
                                text: None,
                                reasoning: None,
                            },
                            false,
                        ))
                    })
                    .expect("watch");
            });
            let delivered = lock(&sink.0)
                .iter()
                .filter_map(|event| match event {
                    JobEvent::TextDelta { text, .. } => text.clone(),
                    _ => None,
                })
                .collect::<String>();
            assert_eq!(delivered, "one");
        }
    }
}
