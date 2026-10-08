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

    fn emit_if(
        &self,
        event: GenerationEvent,
        valid: std::sync::Arc<dyn Fn() -> bool + Send + Sync>,
    ) {
        if valid() {
            self.emit(event);
        }
    }
}

/// The stream of one watched job, handed in by `job_watch`. Delivery is
/// best effort, like `GenerationEventSink`.
pub trait JobEventSink: Send + Sync {
    /// Returns `false` once the consumer is gone, which ends the watch.
    fn emit(&self, event: JobEvent) -> bool;
}

#[derive(Clone)]
pub(crate) struct GenerationLiveEvents(std::sync::Arc<dyn Fn(GenerationEvent) + Send + Sync>);

impl std::fmt::Debug for GenerationLiveEvents {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("GenerationLiveEvents")
    }
}

impl GenerationLiveEvents {
    pub(crate) fn new(emit: impl Fn(GenerationEvent) + Send + Sync + 'static) -> Self {
        Self(std::sync::Arc::new(emit))
    }

    pub(crate) fn emit(&self, event: GenerationEvent) {
        (self.0)(event);
    }
}
