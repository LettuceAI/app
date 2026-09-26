use lettuce_contracts::{ApiEvent, GenerationEvent, JobEvent};

/// Application-wide events, broadcast by the host to every window.
pub trait ApiEventSink: Send + Sync {
    fn emit(&self, event: ApiEvent);
}

/// The stream of one generation turn, handed in by the call that started
/// it. Delivery is best effort: a closed consumer loses events and the
/// generation continues.
pub trait GenerationEventSink: Send + Sync {
    fn emit(&self, event: GenerationEvent);
}

/// The stream of one watched job, handed in by `job_watch`. Delivery is
/// best effort, like `GenerationEventSink`.
pub trait JobEventSink: Send + Sync {
    /// Returns `false` once the consumer is gone, which ends the watch.
    fn emit(&self, event: JobEvent) -> bool;
}
