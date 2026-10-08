use super::local_runtime_events::FlowId;
use super::tests::{RecordingStream, Reply, harness};
use lettuce_jobs::handle::CancellationToken;
use lettuce_local_llm::generation::{LlamaHostEvent, LlamaNotice};
use lettuce_types::{GenerationAttemptId, GenerationTurnId, JobId};
use std::sync::{Arc, Barrier, mpsc};
use std::time::Duration;

#[test]
fn runtime_ingress_does_not_wait_for_production_job_watch_initialization() {
    let env = harness(Reply::Text("Hello."));
    let job = JobId::new();
    let flow = env
        .context
        .register_runtime_job_events(job, CancellationToken::new());
    let router = env.context.backend().local_runtime_events().clone();
    let id = GenerationAttemptId::new();
    let attempt = router.register_attempt(id, Some(FlowId::Job(job)));
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    struct Sink;
    impl super::JobEventSink for Sink {
        fn emit(&self, _: lettuce_contracts::JobEvent) -> bool {
            true
        }
    }
    let context = env.context.clone();
    let e = entered.clone();
    let r = release.clone();
    let watch = std::thread::spawn(move || {
        let _: Result<(), ()> = context.jobs().watch(job, Arc::new(Sink), || {
            e.wait();
            r.wait();
            Err(())
        });
    });
    entered.wait();
    let (done, received) = mpsc::channel();
    let emit = std::thread::spawn(move || {
        router.emit(LlamaHostEvent::Notice {
            request_id: Some(id.to_string()),
            notice: LlamaNotice::MtpDisabledForVision,
        });
        done.send(()).expect("done");
    });
    let returned = received.recv_timeout(Duration::from_millis(250)).is_ok();
    release.wait();
    watch.join().expect("watch");
    emit.join().expect("emit");
    drop((attempt, flow));
    assert!(
        returned,
        "native ingress waited for the real job watch registry"
    );
}

#[test]
fn runtime_ingress_does_not_wait_for_a_slow_turn_ui_consumer() {
    let env = harness(Reply::Text("Hello."));
    let turn = GenerationTurnId::new();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    struct Sink {
        entered: Arc<Barrier>,
        release: Arc<Barrier>,
    }
    impl super::GenerationEventSink for Sink {
        fn emit(&self, _: lettuce_contracts::GenerationEvent) {
            self.entered.wait();
            self.release.wait();
        }
    }
    env.context.attach_stream(
        turn,
        Arc::new(Sink {
            entered: entered.clone(),
            release: release.clone(),
        }),
    );
    let flow = env
        .context
        .register_runtime_turn_events(turn, CancellationToken::new());
    let router = env.context.backend().local_runtime_events().clone();
    let id = GenerationAttemptId::new();
    let attempt = router.register_attempt(id, Some(FlowId::Turn(turn)));
    let shutdown_router = router.clone();
    let (done, received) = mpsc::channel();
    let emit = std::thread::spawn(move || {
        router.emit(LlamaHostEvent::Notice {
            request_id: Some(id.to_string()),
            notice: LlamaNotice::MtpDisabledForVision,
        });
        done.send(()).expect("done");
    });
    entered.wait();
    let returned = received.recv_timeout(Duration::from_millis(250)).is_ok();
    shutdown_router.shutdown();
    drop((attempt, flow));
    release.wait();
    emit.join().expect("emit");
    assert!(
        returned,
        "native ingress waited for a production turn sink callback"
    );
}

#[test]
fn late_turn_attach_replays_the_current_model_load_without_another_engine_update() {
    let env = harness(Reply::Text("Hello."));
    let turn = GenerationTurnId::new();
    let flow = env
        .context
        .register_runtime_turn_events(turn, CancellationToken::new());
    let router = env.context.backend().local_runtime_events().clone();
    let id = GenerationAttemptId::new();
    let attempt = router.register_attempt(id, Some(FlowId::Turn(turn)));
    router.emit(LlamaHostEvent::ModelLoadProgress(
        lettuce_local_llm::engine::ModelLoadProgress {
            request_id: Some(id.to_string()),
            model_path: "model.gguf".into(),
            model_name: "Model".into(),
            backend_path: "cpu".into(),
            stage: lettuce_local_llm::engine::ModelLoadStage::Cpu,
            status: lettuce_local_llm::engine::ModelLoadStatus::Loading,
            progress: 0.42,
            percent: 42,
            gpus: None,
        },
    ));
    let late = Arc::new(RecordingStream::default());
    env.context.attach_stream(turn, late.clone());
    let found = late.events().iter().any(|e| {
        matches!(
            e,
            lettuce_contracts::GenerationEvent::ModelLoading { percent: 42, .. }
        )
    });
    drop((attempt, flow));
    assert!(
        found,
        "late turn listener never learned the retained 42% load state"
    );
}

fn progress(id: GenerationAttemptId, percent: u8, name: &str) -> LlamaHostEvent {
    LlamaHostEvent::ModelLoadProgress(lettuce_local_llm::engine::ModelLoadProgress {
        request_id: Some(id.to_string()),
        model_path: "model.gguf".into(),
        model_name: name.into(),
        backend_path: "cpu".into(),
        stage: lettuce_local_llm::engine::ModelLoadStage::Cpu,
        status: lettuce_local_llm::engine::ModelLoadStatus::Loading,
        progress: f32::from(percent) / 100.0,
        percent,
        gpus: None,
    })
}

#[derive(Default)]
struct JobRecorder(std::sync::Mutex<Vec<lettuce_contracts::JobEvent>>);
impl super::JobEventSink for JobRecorder {
    fn emit(&self, event: lettuce_contracts::JobEvent) -> bool {
        self.0.lock().expect("events").push(event);
        true
    }
}

#[tokio::test]
async fn late_job_watch_replays_load_state_even_when_the_dispatch_queue_is_full() {
    use lettuce_jobs::{JobKind, JobSpec, JobStore, JobSubject, OutcomeRef, SubjectKind};
    use lettuce_types::RequestId;
    let env = harness(Reply::Text("Hello."));
    let chat = super::tests::launch(&env, "queue-full-source").await;
    let job = JobStore::create_or_get(
        env.context.backend().database(),
        JobSpec::new(
            JobKind::MemoryExtraction,
            JobSubject::new(SubjectKind::Conversation, chat).expect("subject"),
            OutcomeRef::Request(RequestId::new()),
        )
        .with_resources(vec![lettuce_jobs::ResourceClass::Cpu]),
    )
    .expect("job")
    .job;
    let job_token = CancellationToken::new();
    let job_flow = env
        .context
        .register_runtime_job_events(job.id, job_token.clone());
    let router = env.context.backend().local_runtime_events().clone();
    let job_id = GenerationAttemptId::new();
    let job_attempt = router.register_attempt(job_id, Some(FlowId::Job(job.id)));
    let turn = GenerationTurnId::new();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    struct Slow {
        entered: Arc<Barrier>,
        release: Arc<Barrier>,
    }
    impl super::GenerationEventSink for Slow {
        fn emit(&self, _: lettuce_contracts::GenerationEvent) {
            self.entered.wait();
            self.release.wait();
        }
    }
    env.context.attach_stream(
        turn,
        Arc::new(Slow {
            entered: entered.clone(),
            release: release.clone(),
        }),
    );
    let turn_flow = env
        .context
        .register_runtime_turn_events(turn, CancellationToken::new());
    let turn_id = GenerationAttemptId::new();
    let turn_attempt = router.register_attempt(turn_id, Some(FlowId::Turn(turn)));
    router.emit(LlamaHostEvent::Notice {
        request_id: Some(turn_id.to_string()),
        notice: LlamaNotice::MtpDisabledForVision,
    });
    entered.wait();
    for index in 0..400 {
        router.emit(progress(job_id, 42, &format!("Model {index}")));
    }
    let watch = Arc::new(JobRecorder::default());
    let result = super::job_watch(
        &env.context,
        lettuce_contracts::JobWatchRequest {
            job_id: job.id.to_string(),
        },
        watch.clone(),
    )
    .await;
    let events = watch.0.lock().expect("events").clone();
    let repeated = Arc::new(JobRecorder::default());
    let repeated_result = super::job_watch(
        &env.context,
        lettuce_contracts::JobWatchRequest {
            job_id: job.id.to_string(),
        },
        repeated.clone(),
    )
    .await;
    let next_id = GenerationAttemptId::new();
    let next_attempt = router.register_attempt(next_id, Some(FlowId::Job(job.id)));
    let fresh = Arc::new(JobRecorder::default());
    let fresh_result = super::job_watch(
        &env.context,
        lettuce_contracts::JobWatchRequest {
            job_id: job.id.to_string(),
        },
        fresh.clone(),
    )
    .await;
    router.emit(progress(next_id, 67, "Next"));
    let next = Arc::new(JobRecorder::default());
    let next_result = super::job_watch(
        &env.context,
        lettuce_contracts::JobWatchRequest {
            job_id: job.id.to_string(),
        },
        next.clone(),
    )
    .await;
    job_token.cancel();
    let cancelled = Arc::new(JobRecorder::default());
    let cancelled_result = super::job_watch(
        &env.context,
        lettuce_contracts::JobWatchRequest {
            job_id: job.id.to_string(),
        },
        cancelled.clone(),
    )
    .await;
    drop((job_attempt, next_attempt, turn_attempt));
    release.wait();
    router.flush();
    drop((job_flow, turn_flow));
    result.expect("watch");
    repeated_result.expect("repeated");
    fresh_result.expect("fresh");
    next_result.expect("next");
    cancelled_result.expect("cancelled");
    assert!(repeated.0.lock().expect("events").iter().any(|event| matches!(event, lettuce_contracts::JobEvent::ModelLoading { percent:42, model_name, .. } if model_name == "Model 399")));
    assert!(
        !fresh
            .0
            .lock()
            .expect("events")
            .iter()
            .any(|event| matches!(event, lettuce_contracts::JobEvent::ModelLoading { .. }))
    );
    assert!(next.0.lock().expect("events").iter().any(|event| matches!(event, lettuce_contracts::JobEvent::ModelLoading { percent:67, model_name, .. } if model_name == "Next")));
    assert!(
        !cancelled
            .0
            .lock()
            .expect("events")
            .iter()
            .any(|event| matches!(event, lettuce_contracts::JobEvent::ModelLoading { .. }))
    );
    assert!(matches!(
        &events[0],
        lettuce_contracts::JobEvent::Progress { .. }
    ));
    assert!(events.iter().any(|event| matches!(event, lettuce_contracts::JobEvent::ModelLoading { percent:42, model_name, .. } if model_name == "Model 399")));
    assert!(!watch.0.lock().expect("events").iter().any(|event| matches!(event, lettuce_contracts::JobEvent::ModelLoading { model_name, .. } if model_name != "Model 399")));
}

#[test]
fn repeated_turn_attach_uses_current_attempt_and_never_replays_cancelled_or_ended_loads() {
    let env = harness(Reply::Text("Hello."));
    let turn = GenerationTurnId::new();
    let token = CancellationToken::new();
    let flow = env
        .context
        .register_runtime_turn_events(turn, token.clone());
    let router = env.context.backend().local_runtime_events().clone();
    let first_id = GenerationAttemptId::new();
    let first = router.register_attempt(first_id, Some(FlowId::Turn(turn)));
    router.emit(progress(first_id, 42, "First"));
    router.flush();
    for _ in 0..2 {
        let listener = Arc::new(RecordingStream::default());
        env.context.attach_stream(turn, listener.clone());
        assert!(listener.events().iter().any(|event| matches!(
            event,
            lettuce_contracts::GenerationEvent::ModelLoading { percent: 42, .. }
        )));
    }
    let next_id = GenerationAttemptId::new();
    let next = router.register_attempt(next_id, Some(FlowId::Turn(turn)));
    let listener = Arc::new(RecordingStream::default());
    env.context.attach_stream(turn, listener.clone());
    assert!(listener.events().is_empty());
    drop(first);
    router.emit(progress(next_id, 63, "Next"));
    router.flush();
    let listener = Arc::new(RecordingStream::default());
    env.context.attach_stream(turn, listener.clone());
    assert!(listener.events().iter().any(|event| matches!(
        event,
        lettuce_contracts::GenerationEvent::ModelLoading { percent: 63, .. }
    )));
    token.cancel();
    let listener = Arc::new(RecordingStream::default());
    env.context.attach_stream(turn, listener.clone());
    assert!(listener.events().is_empty());
    drop((next, flow));
    let listener = Arc::new(RecordingStream::default());
    env.context.attach_stream(turn, listener.clone());
    assert!(listener.events().is_empty());
}
