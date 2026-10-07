use std::sync::{Arc, Mutex};

use lettuce_jobs::handle::CancellationToken;
use lettuce_local_llm::generation::{GenerationHeartbeat, LlamaHostEvent};
use lettuce_types::{GenerationAttemptId, GenerationTurnId};

use super::{FlowId, RuntimeEventRouter};

fn heartbeat(attempt: Option<GenerationAttemptId>) -> LlamaHostEvent {
    LlamaHostEvent::Heartbeat {
        request_id: attempt.map(|id| id.to_string()),
        heartbeat: GenerationHeartbeat {
            tokens: 2,
            elapsed_ms: 10,
            tokens_per_second: 200.0,
            recent_text: "hi".into(),
        },
    }
}

#[test]
fn concurrent_attempts_never_cross_routes_and_ended_attempts_drop_events() {
    let router = Arc::new(RuntimeEventRouter::default());
    let first = Arc::new(Mutex::new(Vec::new()));
    let second = Arc::new(Mutex::new(Vec::new()));
    let turn_a = GenerationTurnId::new();
    let turn_b = GenerationTurnId::new();
    let flow_a = {
        let events = first.clone();
        router.register_flow(FlowId::Turn(turn_a), CancellationToken::new(), move |event| {
            events.lock().expect("events").push(event);
        })
    };
    let flow_b = {
        let events = second.clone();
        router.register_flow(FlowId::Turn(turn_b), CancellationToken::new(), move |event| {
            events.lock().expect("events").push(event);
        })
    };
    let id_a = GenerationAttemptId::new();
    let id_b = GenerationAttemptId::new();
    let attempt_a = router.register_attempt(id_a, Some(FlowId::Turn(turn_a)));
    let attempt_b = router.register_attempt(id_b, Some(FlowId::Turn(turn_b)));
    std::thread::scope(|scope| {
        scope.spawn(|| router.emit(heartbeat(Some(id_a))));
        scope.spawn(|| router.emit(heartbeat(Some(id_b))));
    });
    assert_eq!(*first.lock().expect("events"), vec![heartbeat(Some(id_a))]);
    assert_eq!(*second.lock().expect("events"), vec![heartbeat(Some(id_b))]);
    drop(attempt_a);
    router.emit(heartbeat(Some(id_a)));
    assert_eq!(first.lock().expect("events").len(), 1);
    drop((attempt_b, flow_a, flow_b));
}

#[test]
fn cancelled_attempts_and_missing_listeners_do_not_receive_or_fail() {
    let router = Arc::new(RuntimeEventRouter::default());
    let events = Arc::new(Mutex::new(Vec::new()));
    let cancellation = CancellationToken::new();
    let turn = GenerationTurnId::new();
    let flow = {
        let events = events.clone();
        router.register_flow(FlowId::Turn(turn), cancellation.clone(), move |event| {
            events.lock().expect("events").push(event);
        })
    };
    let id = GenerationAttemptId::new();
    let attempt = router.register_attempt(id, Some(FlowId::Turn(turn)));
    router.emit(heartbeat(Some(id)));
    cancellation.cancel();
    router.emit(heartbeat(Some(id)));
    assert_eq!(events.lock().expect("events").len(), 1);
    drop((attempt, flow));
    router.emit(heartbeat(Some(id)));
    router.emit(heartbeat(None));
    router.emit(heartbeat(Some(GenerationAttemptId::new())));
}

#[test]
fn notices_keep_the_request_identity_and_unassigned_events_are_not_broadcast() {
    use lettuce_local_llm::generation::LlamaNotice;
    let router = Arc::new(RuntimeEventRouter::default());
    let routed = Arc::new(Mutex::new(Vec::new()));
    let global = Arc::new(Mutex::new(Vec::new()));
    let turn = GenerationTurnId::new();
    let flow = {
        let events = routed.clone();
        router.register_flow(FlowId::Turn(turn), CancellationToken::new(), move |event| {
            events.lock().expect("events").push(event);
        })
    };
    let id = GenerationAttemptId::new();
    let attempt = router.register_attempt(id, Some(FlowId::Turn(turn)));
    let events = global.clone();
    router.set_global(move |event| events.lock().expect("global").push(event));
    let notice = LlamaHostEvent::Notice {
        request_id: Some(id.to_string()),
        notice: LlamaNotice::MtpDisabledForVision,
    };
    router.emit(notice.clone());
    router.emit(LlamaHostEvent::Notice { request_id: None, notice: LlamaNotice::KvCacheMovedToRam });
    router.emit(heartbeat(None));
    router.emit(heartbeat(Some(GenerationAttemptId::new())));
    let report = LlamaHostEvent::RuntimeReportUpdated { model_path: "model.gguf".into() };
    router.emit(report.clone());
    assert_eq!(*routed.lock().expect("events"), vec![notice]);
    assert_eq!(*global.lock().expect("global"), vec![report]);
    drop((attempt, flow));
}

#[test]
fn a_full_ui_channel_never_blocks_runtime_events() {
    let router = Arc::new(RuntimeEventRouter::default());
    let turn = GenerationTurnId::new();
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let flow = router.register_flow(FlowId::Turn(turn), CancellationToken::new(), move |event| {
        let _ = sender.try_send(event);
    });
    let id = GenerationAttemptId::new();
    let attempt = router.register_attempt(id, Some(FlowId::Turn(turn)));
    for _ in 0..100 {
        router.emit(heartbeat(Some(id)));
    }
    assert_eq!(receiver.try_iter().count(), 1);
    drop(receiver);
    router.emit(heartbeat(Some(id)));
    drop((attempt, flow));
}
