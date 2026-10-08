use std::sync::{Arc, Mutex};

use lettuce_contracts::{self as dto, JobEvent};
use lettuce_conversations::InferencePort;
use lettuce_jobs::{JobKind, JobSpec, JobStore, JobSubject, OutcomeRef, SubjectKind};
use lettuce_local_llm::generation::{LlamaHostEvent, LlamaNotice};
use lettuce_types::{GenerationAttemptId, GenerationTurnId, RequestId};

use crate::CompanionMemoryJobOutput;
use crate::api::tests::{Reply, RecordingStream, harness, launch, send};
use crate::api::{ConversationGenerationWorker, JobEventSink, job_watch};
use super::MemoryJobOutput;

#[derive(Default)]
struct RecordingJob(Mutex<Vec<JobEvent>>);

impl JobEventSink for RecordingJob {
    fn emit(&self, event: JobEvent) -> bool {
        self.0.lock().expect("events").push(event);
        true
    }
}

#[tokio::test]
async fn memory_child_inference_routes_runtime_events_and_text_to_its_watch() {
    let harness = harness(Reply::Text("reply"));
    let chat = launch(&harness, "memory-live-source").await;
    send(&harness, &chat, "memory-live-source-send", "Hi", Arc::new(RecordingStream::default())).await.expect("send");
    ConversationGenerationWorker::new(harness.context.clone()).run_once().await.expect("turn");
    let mut request = harness.provider.requests.lock().expect("requests")[0].clone();
    let job = JobStore::create_or_get(harness.context.backend().database(),
        JobSpec::new(JobKind::MemoryExtraction,
            JobSubject::new(SubjectKind::Conversation, chat).expect("subject"),
            OutcomeRef::Request(RequestId::new())).with_resources(vec![lettuce_jobs::ResourceClass::Cpu])).expect("job").job;
    let watch = Arc::new(RecordingJob::default());
    job_watch(&harness.context, dto::JobWatchRequest { job_id: job.id.to_string() }, watch.clone()).await.expect("watch");
    let output = MemoryJobOutput::new(harness.context.clone());
    let parent = output.open(job.id, lettuce_jobs::handle::CancellationToken::new()).expect("open");
    request.turn_id = GenerationTurnId::new();
    request.attempt_id = GenerationAttemptId::new();
    request.profile.chat_profile.provider_protocol = lettuce_models::ProviderProtocol::LlamaCpp;
    request.stream_sink = Some(parent);
    request.cancellation = Some(job.id);
    let attempt = request.attempt_id;
    harness.provider.runtime_events.store(true, std::sync::atomic::Ordering::Release);
    let inference = crate::companion::companion_memory_host::JobOutputInference::new(
        harness.context.inference(), Some(harness.context.backend().inference_runtime()), Some(parent));
    inference.run(request).await.expect("inference");
    output.close(parent).await.expect("close");
    output.finished(job.id);
    let events = watch.0.lock().expect("events").clone();
    assert!(events.iter().any(|event| matches!(event, JobEvent::Notice { code: dto::RuntimeNoticeCode::MtpDisabledForVision })));
    assert!(events.iter().any(|event| matches!(event, JobEvent::Throughput { tokens:4, tokens_per_second } if *tokens_per_second == 400.0)));
    let loading = events.iter().find_map(|event| {
        let value = serde_json::to_value(event).expect("event");
        (value["type"] == "model_loading").then_some(value)
    }).expect("job model loading");
    assert_eq!(loading["stage"], "cpu");
    assert_eq!(loading["status"], "loading");
    assert_eq!(loading["percent"], 42);
    assert_eq!(loading["model_name"], "Local events");
    let text = events.iter().filter_map(|event| match event { JobEvent::TextDelta {text, ..} => text.clone(), _ => None }).collect::<String>();
    assert_eq!(text, "Hello.");
    harness.context.backend().local_runtime_events().emit(LlamaHostEvent::Notice {
        request_id: Some(attempt.to_string()), notice: LlamaNotice::KvCacheMovedToRam,
    });
    assert_eq!(*watch.0.lock().expect("events"), events);
    let child_request = harness.provider.requests.lock().expect("requests")[1].clone();
    assert_ne!(child_request.stream_sink, Some(parent));
    assert_eq!(child_request.attempt_id, attempt);
}
