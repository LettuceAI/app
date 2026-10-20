use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::watch;

/// Counts the work in flight (running jobs and conversation generations)
/// and tells watchers only when the count moves between zero and non-zero.
pub(crate) struct ActiveWork {
    count: Mutex<usize>,
    active: watch::Sender<bool>,
}

impl Default for ActiveWork {
    fn default() -> Self {
        Self {
            count: Mutex::new(0),
            active: watch::channel(false).0,
        }
    }
}

impl ActiveWork {
    pub(crate) fn begin(self: &Arc<Self>) -> ActiveWorkGuard {
        let mut count = self.count.lock().unwrap_or_else(PoisonError::into_inner);
        *count += 1;
        if *count == 1 {
            self.active.send_replace(true);
        }
        ActiveWorkGuard(Arc::clone(self))
    }

    pub(crate) fn watch(&self) -> watch::Receiver<bool> {
        self.active.subscribe()
    }
}

/// Keeps one unit of work counted until it drops.
pub(crate) struct ActiveWorkGuard(Arc<ActiveWork>);

impl Drop for ActiveWorkGuard {
    fn drop(&mut self) {
        let mut count = self.0.count.lock().unwrap_or_else(PoisonError::into_inner);
        *count -= 1;
        if *count == 0 {
            self.0.active.send_replace(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn changes(receiver: &mut watch::Receiver<bool>) -> Vec<bool> {
        let mut seen = Vec::new();
        if receiver.has_changed().expect("sender alive") {
            seen.push(*receiver.borrow_and_update());
        }
        seen
    }

    #[test]
    fn starts_inactive_and_signals_only_zero_crossings() {
        let work = Arc::new(ActiveWork::default());
        let mut receiver = work.watch();
        assert!(!*receiver.borrow());
        assert!(changes(&mut receiver).is_empty());

        let first = work.begin();
        assert_eq!(changes(&mut receiver), [true]);

        let second = work.begin();
        let third = work.begin();
        assert!(changes(&mut receiver).is_empty());

        drop(second);
        drop(first);
        assert!(changes(&mut receiver).is_empty());
        assert!(*receiver.borrow());

        drop(third);
        assert_eq!(changes(&mut receiver), [false]);
        assert!(changes(&mut receiver).is_empty());
    }

    #[test]
    fn work_that_starts_and_ends_between_reads_collapses_to_the_latest_state() {
        let work = Arc::new(ActiveWork::default());
        let mut receiver = work.watch();
        drop(work.begin());
        drop(work.begin());
        assert_eq!(changes(&mut receiver), [false]);
        let guard = work.begin();
        assert_eq!(changes(&mut receiver), [true]);
        drop(guard);
        assert_eq!(changes(&mut receiver), [false]);
    }

    #[test]
    fn a_new_watcher_sees_the_current_state() {
        let work = Arc::new(ActiveWork::default());
        let guard = work.begin();
        assert!(*work.watch().borrow());
        drop(guard);
        assert!(!*work.watch().borrow());
    }

    #[tokio::test]
    async fn a_watcher_is_woken_by_the_transition() {
        let work = Arc::new(ActiveWork::default());
        let mut receiver = work.watch();
        let waiter = tokio::spawn(async move {
            receiver.changed().await.expect("sender alive");
            *receiver.borrow_and_update()
        });
        let guard = work.begin();
        assert!(waiter.await.expect("waiter"));
        drop(guard);
    }
}
