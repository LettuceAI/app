//! Provider-neutral generation, tools, safety, streaming, and cancellation.

#![deny(unsafe_op_in_unsafe_fn)]

use std::{collections::HashMap, sync::Mutex};

use async_trait::async_trait;
use lettuce_conversations::GenerationStreamEventEnvelope;
use lettuce_jobs::handle::CancellationToken;
use lettuce_types::{JobId, RequestId};
use tokio::sync::mpsc;

const STREAM_CAPACITY: usize = 64;

/// Runtime-only delivery and cancellation boundary used by inference
/// orchestrators and provider adapters. Durable job/conversation state remains
/// in their owning domains.
#[async_trait]
pub trait InferenceRuntimePort: Send + Sync {
    async fn emit(
        &self,
        sink_id: RequestId,
        event: GenerationStreamEventEnvelope,
    ) -> Result<(), InferenceRuntimeError>;

    fn is_cancelled(&self, job_id: JobId) -> bool;
}

/// In-process bridge between a running inference request and its application
/// consumer. The bounded channel intentionally applies backpressure.
#[derive(Debug, Default)]
pub struct InferenceRuntime {
    streams: Mutex<HashMap<RequestId, std::sync::Arc<tokio::sync::Mutex<StreamState>>>>,
    cancellations: Mutex<HashMap<JobId, CancellationToken>>,
}

impl InferenceRuntime {
    pub fn register_stream(
        &self,
        sink_id: RequestId,
    ) -> Result<InferenceStreamReceiver, InferenceRuntimeError> {
        let mut streams = self
            .streams
            .lock()
            .map_err(|_| InferenceRuntimeError::Unavailable)?;
        if streams.contains_key(&sink_id) {
            return Err(InferenceRuntimeError::AlreadyRegistered);
        }
        let (sender, receiver) = mpsc::channel(STREAM_CAPACITY);
        streams.insert(
            sink_id,
            std::sync::Arc::new(tokio::sync::Mutex::new(StreamState {
                sender,
                previous: None,
            })),
        );
        Ok(InferenceStreamReceiver { sink_id, receiver })
    }

    pub fn unregister_stream(&self, sink_id: RequestId) -> Result<(), InferenceRuntimeError> {
        self.streams
            .lock()
            .map_err(|_| InferenceRuntimeError::Unavailable)?
            .remove(&sink_id);
        Ok(())
    }

    pub fn register_cancellation(
        &self,
        job_id: JobId,
        token: CancellationToken,
    ) -> Result<(), InferenceRuntimeError> {
        let mut cancellations = self
            .cancellations
            .lock()
            .map_err(|_| InferenceRuntimeError::Unavailable)?;
        if cancellations.contains_key(&job_id) {
            return Err(InferenceRuntimeError::AlreadyRegistered);
        }
        cancellations.insert(job_id, token);
        Ok(())
    }

    pub fn unregister_cancellation(&self, job_id: JobId) -> Result<(), InferenceRuntimeError> {
        self.cancellations
            .lock()
            .map_err(|_| InferenceRuntimeError::Unavailable)?
            .remove(&job_id);
        Ok(())
    }
}

#[async_trait]
impl InferenceRuntimePort for InferenceRuntime {
    async fn emit(
        &self,
        sink_id: RequestId,
        event: GenerationStreamEventEnvelope,
    ) -> Result<(), InferenceRuntimeError> {
        let stream = self
            .streams
            .lock()
            .map_err(|_| InferenceRuntimeError::Unavailable)?
            .get(&sink_id)
            .cloned()
            .ok_or(InferenceRuntimeError::NotRegistered)?;
        let mut stream = stream.lock().await;
        event
            .validate_after(stream.previous.as_ref())
            .map_err(|_| InferenceRuntimeError::InvalidEvent)?;
        let previous = event.clone();
        stream
            .sender
            .send(event)
            .await
            .map_err(|_| InferenceRuntimeError::ConsumerClosed)?;
        stream.previous = Some(previous);
        Ok(())
    }

    fn is_cancelled(&self, job_id: JobId) -> bool {
        self.cancellations
            .lock()
            .ok()
            .and_then(|tokens| tokens.get(&job_id).cloned())
            .is_some_and(|token| token.is_cancelled())
    }
}

#[derive(Debug)]
pub struct InferenceStreamReceiver {
    sink_id: RequestId,
    receiver: mpsc::Receiver<GenerationStreamEventEnvelope>,
}

#[derive(Debug)]
struct StreamState {
    sender: mpsc::Sender<GenerationStreamEventEnvelope>,
    previous: Option<GenerationStreamEventEnvelope>,
}

impl InferenceStreamReceiver {
    #[must_use]
    pub const fn sink_id(&self) -> RequestId {
        self.sink_id
    }

    pub async fn recv(&mut self) -> Option<GenerationStreamEventEnvelope> {
        self.receiver.recv().await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InferenceRuntimeError {
    #[error("runtime registry is unavailable")]
    Unavailable,
    #[error("runtime identity is already registered")]
    AlreadyRegistered,
    #[error("stream sink is not registered")]
    NotRegistered,
    #[error("stream consumer is closed")]
    ConsumerClosed,
    #[error("stream event is invalid")]
    InvalidEvent,
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::{
        GenerationOperation, GenerationStreamEvent, GenerationStreamEventEnvelope,
    };
    use lettuce_jobs::handle::JobHandle;
    use lettuce_types::{GenerationAttemptId, GenerationTurnId};

    use super::*;

    fn event(
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        sequence: u64,
    ) -> GenerationStreamEventEnvelope {
        GenerationStreamEventEnvelope {
            operation: GenerationOperation::Send,
            turn_id,
            attempt_id,
            sequence,
            event: GenerationStreamEvent::TextDelta {
                text: "delta".to_owned(),
            },
        }
    }

    #[tokio::test]
    async fn routes_valid_events_and_reports_closed_consumers() {
        let runtime = InferenceRuntime::default();
        let sink_id = RequestId::new();
        let mut receiver = runtime.register_stream(sink_id).expect("register");
        let turn_id = GenerationTurnId::new();
        let attempt_id = GenerationAttemptId::new();
        let expected = event(turn_id, attempt_id, 1);
        runtime.emit(sink_id, expected.clone()).await.expect("emit");
        assert_eq!(receiver.recv().await, Some(expected));
        assert_eq!(
            runtime.emit(sink_id, event(turn_id, attempt_id, 3)).await,
            Err(InferenceRuntimeError::InvalidEvent)
        );
        drop(receiver);
        assert_eq!(
            runtime.emit(sink_id, event(turn_id, attempt_id, 2)).await,
            Err(InferenceRuntimeError::ConsumerClosed)
        );
        runtime.unregister_stream(sink_id).expect("unregister");
        assert_eq!(
            runtime.emit(sink_id, event(turn_id, attempt_id, 3)).await,
            Err(InferenceRuntimeError::NotRegistered)
        );
    }

    #[test]
    fn cancellation_tokens_are_scoped_by_job_identity() {
        let runtime = InferenceRuntime::default();
        let handle = JobHandle::new(JobId::new());
        runtime
            .register_cancellation(handle.id(), handle.cancellation_token())
            .expect("register cancellation");
        assert!(!runtime.is_cancelled(handle.id()));
        handle.request_cancel();
        assert!(runtime.is_cancelled(handle.id()));
        runtime
            .unregister_cancellation(handle.id())
            .expect("unregister cancellation");
        assert!(!runtime.is_cancelled(handle.id()));
    }
}
