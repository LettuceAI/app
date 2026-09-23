//! Pure mode around every provider request: streamed text is checked as it
//! arrives and stopped when it crosses the level's threshold, and the final
//! answer is checked whole.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lettuce_conversations::{
    GenerationStreamEvent, GenerationStreamEventEnvelope, InferenceOutcome, InferencePort,
    InferenceRequest, MessagePart, PortError, ProviderFailure, ProviderFailureKind,
};
use lettuce_types::{JobId, RequestId};

use crate::content_filter::{ContentFilter, StreamFilterContext};
use crate::{InferenceRuntimeError, InferenceRuntimePort};

/// The failure code of an answer Pure mode blocked.
pub const CONTENT_BLOCKED_CODE: &str = "CONTENT_BLOCKED";

/// The text shown for an answer Pure mode blocked.
pub const CONTENT_BLOCKED_MESSAGE: &str =
    "Response blocked by Pure Mode. Try rephrasing your message.";

/// The shared state of the stream and outcome checks.
#[derive(Debug)]
pub struct PureModeGuard {
    filter: Arc<ContentFilter>,
    clock: fn() -> u64,
    streams: Mutex<HashMap<RequestId, StreamFilterContext>>,
    blocked: Mutex<HashSet<RequestId>>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

impl PureModeGuard {
    #[must_use]
    pub fn new(filter: Arc<ContentFilter>) -> Self {
        Self {
            filter,
            clock: now_ms,
            streams: Mutex::new(HashMap::new()),
            blocked: Mutex::new(HashSet::new()),
        }
    }

    #[must_use]
    pub fn filter(&self) -> &Arc<ContentFilter> {
        &self.filter
    }

    fn check_delta(&self, sink: RequestId, text: &str) -> bool {
        if !self.filter.is_enabled() || text.is_empty() {
            return false;
        }
        let blocked = self
            .streams
            .lock()
            .map(|mut streams| {
                self.filter
                    .check_delta(streams.entry(sink).or_default(), text, (self.clock)())
                    .blocked
            })
            .unwrap_or(false);
        if blocked && let Ok(mut set) = self.blocked.lock() {
            set.insert(sink);
        }
        blocked
    }

    /// The provider's result once Pure mode has seen it: a blocked stream or
    /// a blocked answer becomes the Pure mode failure.
    pub fn settle(
        &self,
        sink: Option<RequestId>,
        result: Result<InferenceOutcome, PortError>,
    ) -> Result<InferenceOutcome, PortError> {
        if self.finish(sink) {
            return Err(blocked_failure());
        }
        let outcome = result?;
        let text = answer_text(&outcome);
        if !text.trim().is_empty()
            && self.filter.is_enabled()
            && self.filter.check_text(&text, (self.clock)()).blocked
        {
            return Err(blocked_failure());
        }
        Ok(outcome)
    }

    fn finish(&self, sink: Option<RequestId>) -> bool {
        let Some(sink) = sink else {
            return false;
        };
        if let Ok(mut streams) = self.streams.lock() {
            streams.remove(&sink);
        }
        self.blocked
            .lock()
            .map(|mut set| set.remove(&sink))
            .unwrap_or(false)
    }
}

fn blocked_failure() -> PortError {
    PortError::Provider(ProviderFailure {
        kind: ProviderFailureKind::RequestRejected,
        status: 400,
        code: Some(CONTENT_BLOCKED_CODE.to_owned()),
        message: Some(CONTENT_BLOCKED_MESSAGE.to_owned()),
        request_id: None,
    })
}

/// A stream sink that refuses text Pure mode blocks, which stops the
/// provider's stream.
#[derive(Debug)]
pub struct PureModeRuntime<R: ?Sized> {
    inner: Arc<R>,
    guard: Arc<PureModeGuard>,
}

impl<R: ?Sized> PureModeRuntime<R> {
    #[must_use]
    pub const fn new(inner: Arc<R>, guard: Arc<PureModeGuard>) -> Self {
        Self { inner, guard }
    }
}

#[async_trait]
impl<R: InferenceRuntimePort + ?Sized> InferenceRuntimePort for PureModeRuntime<R> {
    async fn emit(
        &self,
        sink_id: RequestId,
        event: GenerationStreamEventEnvelope,
    ) -> Result<(), InferenceRuntimeError> {
        if let GenerationStreamEvent::TextDelta { text } = &event.event
            && self.guard.check_delta(sink_id, text)
        {
            return Err(InferenceRuntimeError::ContentBlocked);
        }
        self.inner.emit(sink_id, event).await
    }

    fn is_cancelled(&self, job_id: JobId) -> bool {
        self.inner.is_cancelled(job_id)
    }

    async fn cancelled(&self, job_id: JobId) -> Result<(), InferenceRuntimeError> {
        self.inner.cancelled(job_id).await
    }
}

/// A provider port whose answers pass Pure mode.
#[derive(Debug)]
pub struct PureModeInference<P: ?Sized> {
    guard: Arc<PureModeGuard>,
    inner: Arc<P>,
}

impl<P: ?Sized> PureModeInference<P> {
    #[must_use]
    pub const fn new(inner: Arc<P>, guard: Arc<PureModeGuard>) -> Self {
        Self { guard, inner }
    }
}

fn answer_text(outcome: &InferenceOutcome) -> String {
    outcome
        .candidates
        .iter()
        .flat_map(|candidate| &candidate.parts)
        .filter_map(|part| match part {
            MessagePart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[async_trait]
impl<P: InferencePort + ?Sized> InferencePort for PureModeInference<P> {
    async fn run(&self, request: InferenceRequest) -> Result<InferenceOutcome, PortError> {
        let sink = request.stream_sink;
        let result = self.inner.run(request).await;
        self.guard.settle(sink, result)
    }
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::{FinishReason, GenerationOperation, InferenceCandidate};
    use lettuce_types::{GenerationAttemptId, GenerationTurnId};

    use super::*;
    use crate::content_filter::PureModeLevel;

    const VIOLENT: &str = "the villain threatened to decapitate and disembowel everyone";

    fn outcome(text: &str) -> InferenceOutcome {
        InferenceOutcome {
            provider_response_id: None,
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts: vec![MessagePart::Text {
                    text: text.to_owned(),
                }],
                tool_calls: Vec::new(),
                provider_replay: None,
            }],
            usage: None,
            finish_reason: FinishReason::Stop,
            provider_finish_reason: None,
            provider_request_id: None,
            warning_codes: Vec::new(),
        }
    }

    fn blocked(result: &Result<InferenceOutcome, PortError>) -> bool {
        matches!(
            result,
            Err(PortError::Provider(failure)) if failure.code.as_deref() == Some(CONTENT_BLOCKED_CODE)
        )
    }

    #[test]
    fn answers_are_checked_whole_at_the_saved_level() {
        let guard = PureModeGuard::new(Arc::new(ContentFilter::new(PureModeLevel::Standard)));
        assert!(blocked(&guard.settle(None, Ok(outcome(VIOLENT)))));
        assert!(guard.settle(None, Ok(outcome("a calm walk"))).is_ok());
        guard.filter().set_level(PureModeLevel::Low);
        assert!(guard.settle(None, Ok(outcome(VIOLENT))).is_ok());
        assert!(matches!(
            guard.settle(None, Err(PortError::Unavailable)),
            Err(PortError::Unavailable)
        ));
    }

    #[tokio::test]
    async fn a_blocked_stream_stops_and_fails_as_pure_mode() {
        let guard = Arc::new(PureModeGuard::new(Arc::new(ContentFilter::new(
            PureModeLevel::Standard,
        ))));
        let inner = Arc::new(crate::InferenceRuntime::default());
        let sink = RequestId::new();
        let mut receiver = inner.register_stream(sink).expect("stream");
        let runtime = PureModeRuntime::new(Arc::clone(&inner), Arc::clone(&guard));
        let (turn_id, attempt_id) = (GenerationTurnId::new(), GenerationAttemptId::new());
        let event = |sequence, text: &str| GenerationStreamEventEnvelope {
            operation: GenerationOperation::Send,
            turn_id,
            attempt_id,
            sequence,
            event: GenerationStreamEvent::TextDelta {
                text: text.to_owned(),
            },
        };
        runtime
            .emit(sink, event(1, "the villain threatened to decap"))
            .await
            .expect("first delta");
        assert!(receiver.recv().await.is_some());
        assert_eq!(
            runtime
                .emit(sink, event(2, "itate and disembowel everyone"))
                .await,
            Err(InferenceRuntimeError::ContentBlocked)
        );
        assert!(blocked(
            &guard.settle(Some(sink), Err(PortError::Cancelled))
        ));
        assert!(guard.settle(Some(sink), Ok(outcome("fine"))).is_ok());
    }
}
