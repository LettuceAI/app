use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::sync::watch;

/// How long the combined signal must stay false before the service stops,
/// so the gap between one job settling and the next being claimed does not
/// stop and restart it.
pub(crate) const SETTLE_GRACE: Duration = Duration::from_secs(5);

/// The OR of every live context's active-work signal.
pub(crate) struct ActiveWorkAggregate {
    entries: Mutex<HashMap<u64, bool>>,
    next: AtomicU64,
    active: watch::Sender<bool>,
}

impl Default for ActiveWorkAggregate {
    fn default() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            next: AtomicU64::new(0),
            active: watch::channel(false).0,
        }
    }
}

impl ActiveWorkAggregate {
    pub(crate) fn watch(&self) -> watch::Receiver<bool> {
        self.active.subscribe()
    }

    /// Mirrors one context's signal until its sender closes, then drops the
    /// context's entry.
    pub(crate) async fn follow(self: Arc<Self>, mut signal: watch::Receiver<bool>) {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        loop {
            self.set(id, Some(*signal.borrow_and_update()));
            if signal.changed().await.is_err() {
                break;
            }
        }
        self.set(id, None);
    }

    fn set(&self, id: u64, value: Option<bool>) {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        match value {
            Some(value) => {
                entries.insert(id, value);
            }
            None => {
                entries.remove(&id);
            }
        }
        let any = entries.values().any(|active| *active);
        self.active.send_if_modified(|current| {
            let changed = *current != any;
            *current = any;
            changed
        });
    }
}

/// Applies `true` at once and `false` only after the signal stayed false for
/// `grace`. `apply` reports whether the change took effect; a change that
/// did not is tried again at the next change of the signal.
pub(crate) async fn drive<F, Fut>(mut signal: watch::Receiver<bool>, grace: Duration, mut apply: F)
where
    F: FnMut(bool) -> Fut,
    Fut: Future<Output = bool>,
{
    let mut applied = false;
    loop {
        let wanted = *signal.borrow_and_update();
        if wanted && !applied {
            applied = apply(true).await;
        } else if !wanted && applied {
            tokio::select! {
                () = tokio::time::sleep(grace) => {
                    applied = !apply(false).await;
                }
                changed = signal.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    continue;
                }
            }
        }
        if signal.changed().await.is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn settle() {
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
    }

    fn calls() -> Arc<Mutex<Vec<bool>>> {
        Arc::new(Mutex::new(Vec::new()))
    }

    fn recorder(
        log: &Arc<Mutex<Vec<bool>>>,
        results: Vec<bool>,
    ) -> impl FnMut(bool) -> std::future::Ready<bool> + use<> {
        let log = Arc::clone(log);
        let mut results = results.into_iter();
        move |value| {
            log.lock().expect("log").push(value);
            std::future::ready(results.next().unwrap_or(true))
        }
    }

    #[tokio::test(start_paused = true)]
    async fn the_aggregate_is_the_or_of_every_context() {
        let aggregate = Arc::new(ActiveWorkAggregate::default());
        let result = aggregate.watch();
        let (old, old_signal) = watch::channel(false);
        let (new, new_signal) = watch::channel(false);
        tokio::spawn(Arc::clone(&aggregate).follow(old_signal));
        old.send_replace(true);
        settle().await;
        assert!(*result.borrow());

        tokio::spawn(Arc::clone(&aggregate).follow(new_signal));
        settle().await;
        assert!(*result.borrow());

        new.send_replace(true);
        settle().await;
        old.send_replace(false);
        settle().await;
        assert!(*result.borrow());

        drop(old);
        settle().await;
        assert!(*result.borrow());

        new.send_replace(false);
        settle().await;
        assert!(!*result.borrow());
    }

    #[tokio::test(start_paused = true)]
    async fn a_closed_context_that_was_active_stops_counting() {
        let aggregate = Arc::new(ActiveWorkAggregate::default());
        let result = aggregate.watch();
        let (context, signal) = watch::channel(false);
        tokio::spawn(Arc::clone(&aggregate).follow(signal));
        context.send_replace(true);
        settle().await;
        assert!(*result.borrow());
        drop(context);
        settle().await;
        assert!(!*result.borrow());
    }

    #[tokio::test(start_paused = true)]
    async fn true_applies_at_once_and_false_waits_for_the_grace_delay() {
        let (signal, receiver) = watch::channel(false);
        let log = calls();
        tokio::spawn(drive(receiver, SETTLE_GRACE, recorder(&log, vec![])));
        settle().await;
        assert!(log.lock().expect("log").is_empty());

        signal.send_replace(true);
        settle().await;
        assert_eq!(*log.lock().expect("log"), [true]);

        signal.send_replace(false);
        settle().await;
        tokio::time::advance(SETTLE_GRACE - Duration::from_millis(1)).await;
        settle().await;
        assert_eq!(*log.lock().expect("log"), [true]);

        tokio::time::advance(Duration::from_millis(1)).await;
        settle().await;
        assert_eq!(*log.lock().expect("log"), [true, false]);
    }

    #[tokio::test(start_paused = true)]
    async fn a_settle_then_claim_gap_collapses_into_no_change() {
        let (signal, receiver) = watch::channel(false);
        let log = calls();
        tokio::spawn(drive(receiver, SETTLE_GRACE, recorder(&log, vec![])));
        signal.send_replace(true);
        settle().await;
        for _ in 0..3 {
            signal.send_replace(false);
            settle().await;
            tokio::time::advance(Duration::from_secs(2)).await;
            settle().await;
            signal.send_replace(true);
            settle().await;
        }
        tokio::time::advance(Duration::from_secs(60)).await;
        settle().await;
        assert_eq!(*log.lock().expect("log"), [true]);
    }

    #[tokio::test(start_paused = true)]
    async fn a_change_that_did_not_take_effect_is_tried_again() {
        let (signal, receiver) = watch::channel(false);
        let log = calls();
        tokio::spawn(drive(
            receiver,
            SETTLE_GRACE,
            recorder(&log, vec![false, true]),
        ));
        signal.send_replace(true);
        settle().await;
        assert_eq!(*log.lock().expect("log"), [true]);

        signal.send_replace(false);
        settle().await;
        signal.send_replace(true);
        settle().await;
        assert_eq!(*log.lock().expect("log"), [true, true]);

        signal.send_replace(false);
        settle().await;
        tokio::time::advance(SETTLE_GRACE).await;
        settle().await;
        assert_eq!(*log.lock().expect("log"), [true, true, false]);
    }
}
