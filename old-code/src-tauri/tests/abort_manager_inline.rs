//! Gathered from inline tests in src/infra/abort_manager.rs.

use lettuceai_lib::infra::abort_manager::AbortRegistry;
use tokio::sync::oneshot::error::TryRecvError;

#[test]
fn nested_registration_keeps_existing_receiver_alive() {
    let registry = AbortRegistry::new();
    let mut outer_rx = registry.register("req-1".to_string());
    let _inner_rx = registry.register("req-1".to_string());

    assert!(matches!(outer_rx.try_recv(), Err(TryRecvError::Empty)));
    assert!(registry.is_registered("req-1"));
}

#[test]
fn abort_cancels_all_receivers_for_request_id() {
    let registry = AbortRegistry::new();
    let mut outer_rx = registry.register("req-1".to_string());
    let mut inner_rx = registry.register("req-1".to_string());

    registry.abort("req-1").expect("abort should succeed");

    assert!(matches!(outer_rx.try_recv(), Ok(())));
    assert!(matches!(inner_rx.try_recv(), Ok(())));
    assert!(!registry.is_registered("req-1"));
}
