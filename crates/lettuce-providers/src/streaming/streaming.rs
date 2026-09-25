use lettuce_conversations::{
    GenerationStreamEvent, GenerationStreamEventEnvelope, InferenceOutcome, InferenceRequest,
    ProviderFailure, ProviderFailureKind,
};
use lettuce_inference::InferenceRuntimePort;
use lettuce_network::{JsonResponse, JsonResponseStream};
use lettuce_types::JobId;
use std::future::Future;

use crate::common::AdapterError;
use crate::streaming::stream_framing::{FramingError, StreamFormat, StreamFramer};
use crate::streaming::stream_normalize::{
    StreamDelta, StreamNormalizeError, StreamNormalizer, StreamProtocol,
};

pub(crate) async fn consume_stream(
    response: JsonResponseStream,
    format: StreamFormat,
    protocol: StreamProtocol,
    runtime: &dyn InferenceRuntimePort,
    request: &InferenceRequest,
) -> Result<InferenceOutcome, AdapterError> {
    let (outcome, _) =
        consume_stream_with_provider_replay(response, format, protocol, runtime, request).await?;
    Ok(outcome)
}

pub(crate) async fn consume_stream_with_provider_replay(
    response: JsonResponseStream,
    format: StreamFormat,
    protocol: StreamProtocol,
    runtime: &dyn InferenceRuntimePort,
    request: &InferenceRequest,
) -> Result<(InferenceOutcome, Option<Vec<u8>>), AdapterError> {
    if !(200..300).contains(&response.status) {
        return read_error_response(response, runtime, request.cancellation)
            .await
            .map(|outcome| (outcome, None));
    }
    let provider_request_id = response.request_id.clone();
    let mut normalizer = StreamNormalizer::new(protocol, provider_request_id.clone());
    let mut sequence = 0_u64;
    let mut emitted = EmittedReply::default();
    match stream_deltas(
        response,
        format,
        runtime,
        request,
        &mut normalizer,
        &mut sequence,
        &mut emitted,
    )
    .await
    {
        Ok(()) => {}
        Err(AdapterError::Cancelled) => {
            return normalizer
                .cancelled_outcome(emitted.text, emitted.reasoning)
                .map(|outcome| (outcome, None))
                .ok_or(AdapterError::Cancelled);
        }
        Err(error) => return Err(error),
    }
    let completion = normalizer
        .finish_with_provider_replay()
        .map_err(|error| map_normalize(error, provider_request_id))?;
    for delta in completion.tail {
        sequence = sequence.checked_add(1).ok_or(AdapterError::Transport)?;
        emit(runtime, request, sequence, delta).await?;
    }
    Ok((completion.outcome, completion.provider_replay))
}

/// The text and reasoning deltas that reached the stream sink.
#[derive(Debug, Default)]
struct EmittedReply {
    text: String,
    reasoning: String,
}

/// Streams every record into `normalizer`, emitting its deltas. A
/// cancellation leaves the deltas that reached the sink in `emitted`.
async fn stream_deltas(
    response: JsonResponseStream,
    format: StreamFormat,
    runtime: &dyn InferenceRuntimePort,
    request: &InferenceRequest,
    normalizer: &mut StreamNormalizer,
    sequence: &mut u64,
    emitted: &mut EmittedReply,
) -> Result<(), AdapterError> {
    let mut response = response.without_size_limit();
    let mut framer = StreamFramer::new(format);
    let provider_request_id = response.request_id.clone();
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
                *sequence = sequence.checked_add(1).ok_or(AdapterError::Transport)?;
                emit_recorded(runtime, request, *sequence, delta, emitted).await?;
            }
        }
    }
    if let Some(record) = framer.finish().map_err(map_framing)? {
        for delta in normalizer
            .consume(&record)
            .map_err(|error| map_normalize(error, provider_request_id.clone()))?
        {
            *sequence = sequence.checked_add(1).ok_or(AdapterError::Transport)?;
            emit_recorded(runtime, request, *sequence, delta, emitted).await?;
        }
    }
    Ok(())
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
        content_type: None,
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

async fn emit_recorded(
    runtime: &dyn InferenceRuntimePort,
    request: &InferenceRequest,
    sequence: u64,
    delta: StreamDelta,
    emitted: &mut EmittedReply,
) -> Result<(), AdapterError> {
    emit(runtime, request, sequence, delta.clone()).await?;
    match delta {
        StreamDelta::Text(text) => emitted.text.push_str(&text),
        StreamDelta::Reasoning(text) => emitted.reasoning.push_str(&text),
    }
    Ok(())
}

pub(crate) async fn emit(
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
        FramingError::InvalidUtf8 => AdapterError::MalformedResponse,
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
