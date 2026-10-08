use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

pub(crate) type Validity = Arc<dyn Fn() -> bool + Send + Sync>;

struct Pending<E> {
    events: VecDeque<(E, Option<Validity>)>,
    initializing: bool,
    draining: bool,
}

pub(crate) struct SerialEvents<E> {
    consumer: Arc<dyn Fn(E) -> bool + Send + Sync>,
    pending: Mutex<Pending<E>>,
    open: AtomicBool,
    finished: AtomicBool,
}

impl<E> SerialEvents<E> {
    pub(crate) fn new(consumer: impl Fn(E) -> bool + Send + Sync + 'static) -> Self {
        Self {
            consumer: Arc::new(consumer),
            pending: Mutex::new(Pending {
                events: VecDeque::new(),
                initializing: true,
                draining: false,
            }),
            open: AtomicBool::new(true),
            finished: AtomicBool::new(false),
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open.load(Ordering::Acquire)
    }

    pub(crate) fn send(&self, event: E) -> bool {
        if self.finished.load(Ordering::Acquire) {
            return false;
        }
        self.enqueue(event, false, None)
    }

    pub(crate) fn finish(&self, event: E) {
        if !self.finished.swap(true, Ordering::AcqRel) {
            self.enqueue(event, true, None);
        }
    }

    pub(crate) fn send_if(&self, event: E, valid: Validity) -> bool {
        if self.finished.load(Ordering::Acquire) {
            return false;
        }
        self.enqueue(event, false, Some(valid))
    }

    fn enqueue(&self, event: E, terminal: bool, valid: Option<Validity>) -> bool {
        if !self.open.load(Ordering::Acquire) {
            return false;
        }
        let drain = {
            let mut pending = self
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !terminal && self.finished.load(Ordering::Acquire) {
                return false;
            }
            if valid.is_none() || pending.events.len() < 256 {
                pending.events.push_back((event, valid));
            }
            if pending.initializing || pending.draining {
                false
            } else {
                pending.draining = true;
                true
            }
        };
        if drain {
            self.drain();
        }
        self.open.load(Ordering::Acquire)
    }

    pub(crate) fn initialize(&self, replay: Vec<E>) {
        let drain = {
            let mut pending = self
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for event in replay.into_iter().rev() {
                pending.events.push_front((event, None));
            }
            pending.initializing = false;
            if pending.draining {
                false
            } else {
                pending.draining = true;
                true
            }
        };
        if drain {
            self.drain();
        }
    }

    fn drain(&self) {
        loop {
            let event = {
                let mut pending = self
                    .pending
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                match pending.events.pop_front() {
                    Some(event) => event,
                    None => {
                        pending.draining = false;
                        return;
                    }
                }
            };
            let (event, valid) = event;
            if valid.as_ref().is_some_and(|valid| !valid()) {
                continue;
            }
            if !(self.consumer)(event) {
                self.open.store(false, Ordering::Release);
                let mut pending = self
                    .pending
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                pending.events.clear();
                pending.draining = false;
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_text_and_terminal_events_are_never_dropped() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let consumer = Arc::clone(&received);
        let delivery = SerialEvents::new(move |event| {
            consumer.lock().expect("consumer events").push(event);
            true
        });
        for event in 0..300 {
            assert!(delivery.send(event));
        }
        delivery.finish(300);
        delivery.initialize(Vec::new());
        assert_eq!(
            *received.lock().expect("received events"),
            (0..=300).collect::<Vec<_>>()
        );
    }
}
