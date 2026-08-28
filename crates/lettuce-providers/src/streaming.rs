use lettuce_conversations::{
    GenerationStreamEvent, GenerationStreamEventEnvelope, InferenceOutcome, InferenceRequest,
    ProviderFailure, ProviderFailureKind,
};
use lettuce_inference::InferenceRuntimePort;
use lettuce_network::{JsonResponse, JsonResponseStream};
use lettuce_types::JobId;
use std::future::Future;

use crate::common::AdapterError;
use crate::stream_framing::{FramingError, StreamFormat, StreamFramer};
use crate::stream_normalize::{
    StreamDelta, StreamNormalizeError, StreamNormalizer, StreamProtocol,
};

pub(crate) async fn consume_stream(
    mut response: JsonResponseStream,
    format: StreamFormat,
    protocol: StreamProtocol,
    runtime: &dyn InferenceRuntimePort,
    request: &InferenceRequest,
) -> Result<InferenceOutcome, AdapterError> {
    if !(200..300).contains(&response.status) {
        return read_error_response(response, runtime, request.cancellation).await;
    }
    let mut framer = StreamFramer::new(format);
    let provider_request_id = response.request_id.clone();
    let mut normalizer = StreamNormalizer::new(protocol, provider_request_id.clone());
    let mut sequence = 0_u64;
    loop {
        ensure_not_cancelled(runtime, request)?;
        let Some(chunk) = next_chunk(&mut response, runtime, request.cancellation).await? else {
            break;
        };
        for record in framer.push(&chunk).map_err(map_framing)? {
            for delta in normalizer
                .consume(&record)
                .map_err(|error| map_normalize(error, provider_request_id.clone()))?
            {
                sequence = sequence.checked_add(1).ok_or(AdapterError::Transport)?;
                emit(runtime, request, sequence, delta).await?;
            }
        }
    }
    framer.finish().map_err(map_framing)?;
    let (tail, outcome) = normalizer
        .finish()
        .map_err(|error| map_normalize(error, provider_request_id))?;
    for delta in tail {
        sequence = sequence.checked_add(1).ok_or(AdapterError::Transport)?;
        emit(runtime, request, sequence, delta).await?;
    }
    Ok(outcome)
}

pub(crate) async fn await_cancelable<T, F>(
    runtime: &dyn InferenceRuntimePort,
    cancellation: Option<JobId>,
    future: F,
) -> Result<T, AdapterError>
where
    F: Future<Output = Result<T, AdapterError>>,
{
    let Some(job_id) = cancellation else {
        return future.await;
    };
    if runtime.is_cancelled(job_id) {
        return Err(AdapterError::Cancelled);
    }
    tokio::pin!(future);
    tokio::select! {
        result = &mut future => result,
        cancellation = runtime.cancelled(job_id) => match cancellation {
            Ok(()) => Err(AdapterError::Cancelled),
            Err(_) => Err(AdapterError::Transport),
        }
    }
}

async fn next_chunk(
    response: &mut JsonResponseStream,
    runtime: &dyn InferenceRuntimePort,
    cancellation: Option<JobId>,
) -> Result<Option<Vec<u8>>, AdapterError> {
    await_cancelable(runtime, cancellation, async {
        response.next_chunk().await.map_err(Into::into)
    })
    .await
}

async fn read_error_response(
    mut response: JsonResponseStream,
    runtime: &dyn InferenceRuntimePort,
    cancellation: Option<JobId>,
) -> Result<InferenceOutcome, AdapterError> {
    let mut body = Vec::new();
    while let Some(chunk) = next_chunk(&mut response, runtime, cancellation).await? {
        body.extend_from_slice(&chunk);
    }
    let response = JsonResponse {
        status: response.status,
        body,
        request_id: response.request_id,
        retry_after: response.retry_after,
    };
    Err(AdapterError::from_response(&response).unwrap_or(AdapterError::Transport))
}

fn ensure_not_cancelled(
    runtime: &dyn InferenceRuntimePort,
    request: &InferenceRequest,
) -> Result<(), AdapterError> {
    if request
        .cancellation
        .is_some_and(|job_id| runtime.is_cancelled(job_id))
    {
        Err(AdapterError::Cancelled)
    } else {
        Ok(())
    }
}

async fn emit(
    runtime: &dyn InferenceRuntimePort,
    request: &InferenceRequest,
    sequence: u64,
    delta: StreamDelta,
) -> Result<(), AdapterError> {
    let Some(sink_id) = request.stream_sink else {
        return Ok(());
    };
    ensure_not_cancelled(runtime, request)?;
    let event = match delta {
        StreamDelta::Text(text) => GenerationStreamEvent::TextDelta { text },
        StreamDelta::Reasoning(text) => GenerationStreamEvent::ReasoningDelta { text },
    };
    await_cancelable(runtime, request.cancellation, async {
        runtime
            .emit(
                sink_id,
                GenerationStreamEventEnvelope {
                    operation: request.operation,
                    turn_id: request.turn_id,
                    attempt_id: request.attempt_id,
                    sequence,
                    event,
                },
            )
            .await
            .map_err(|_| AdapterError::Transport)
    })
    .await
}

fn map_framing(error: FramingError) -> AdapterError {
    match error {
        FramingError::InvalidUtf8 | FramingError::PrematureEof => AdapterError::MalformedResponse,
        FramingError::RecordTooLarge(_) => AdapterError::Transport,
    }
}

fn map_normalize(error: StreamNormalizeError, request_id: Option<String>) -> AdapterError {
    match error {
        StreamNormalizeError::MalformedJson
        | StreamNormalizeError::DataAfterTerminal
        | StreamNormalizeError::PrematureEof => AdapterError::MalformedResponse,
        StreamNormalizeError::OutputTooLarge { .. } => AdapterError::Transport,
        StreamNormalizeError::EmptyResponse => AdapterError::EmptyResponse,
        StreamNormalizeError::Provider {
            status,
            code,
            message,
        } => AdapterError::Provider(ProviderFailure {
            kind: match status {
                Some(401 | 403) => ProviderFailureKind::CredentialRejected,
                Some(408 | 429 | 500..=599) | None => ProviderFailureKind::Unavailable,
                Some(_) => ProviderFailureKind::RequestRejected,
            },
            status: status.unwrap_or(500),
            code,
            message,
            request_id,
        }),
    }
}
