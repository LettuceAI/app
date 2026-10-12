use std::sync::Arc;

use super::tests::{Reply, harness};
use super::ApiContextSlot;

#[test]
fn context_slot_swaps_snapshots_and_blocks_commands_requiring_recovery() {
    let first = Arc::new(harness(Reply::Text("first")).context);
    let second = Arc::new(harness(Reply::Text("second")).context);
    let slot = ApiContextSlot::new(first.clone());
    let held = slot.snapshot().expect("first snapshot");
    assert!(Arc::ptr_eq(&held, &first));
    assert!(Arc::ptr_eq(&slot.swap(second.clone()).expect("swap"), &first));
    assert!(Arc::ptr_eq(&slot.snapshot().expect("second snapshot"), &second));
    assert!(Arc::ptr_eq(&held, &first));
    slot.require_recovery();
    let failure = slot.snapshot().expect_err("reopen required");
    assert_eq!(failure.code, lettuce_contracts::ApiErrorCode::Unavailable);
    assert_eq!(failure.details, Some(lettuce_contracts::ApiErrorDetails::Backup { reason: lettuce_contracts::BackupFailureReason::RecoveryRequired }));
}

#[tokio::test]
async fn review_context_is_published_only_after_restart_recovery_finishes() {
    use lettuce_types::JobId;
    let first = Arc::new(harness(Reply::Text("first")).context);
    let (_root, _location, fresh) = super::app::reset_tests::harness();
    let fresh = Arc::new(fresh.context);
    let slot = Arc::new(ApiContextSlot::new(first.clone()));
    let recovery_guard = fresh.provider_write_guard().await;
    let starting_slot = slot.clone();
    let starting_context = fresh.clone();
    let mut starting = tokio::spawn(async move { starting_slot.start_reloaded(starting_context, &JobId::new().to_string()).await });
    assert!(tokio::time::timeout(std::time::Duration::from_millis(100), &mut starting).await.is_err(), "startup waits for its ordinary provider recovery guard");
    assert!(Arc::ptr_eq(&slot.snapshot().expect("old context during recovery"), &first));
    drop(recovery_guard);
    let (old, workers) = starting.await.expect("reload task").expect("reload");
    assert!(Arc::ptr_eq(&old, &first));
    assert!(Arc::ptr_eq(&slot.snapshot().expect("recovered context"), &fresh));
    assert!(workers.steps().contains(&super::StartupStep::RecoverAfterRestart));
    assert!(workers.steps().contains(&super::StartupStep::StartWorkers));
    workers.stop_for_restore().await.expect("stop workers");
}

#[tokio::test]
async fn review_failed_startup_blocks_snapshots_without_publishing_or_reloading() {
    let first = Arc::new(harness(Reply::Text("first")).context);
    let (_root, _location, fresh) = super::app::reset_tests::harness();
    let events = fresh.events.clone();
    let fresh = Arc::new(fresh.context);
    let slot = ApiContextSlot::new(first.clone());
    lettuce_database::Database::lock_file_writes(&fresh.database_files().expect("files").active).expect("failure injection").set_fenced(true).expect("fence before closing");
    fresh.backend().database().close_for_reset().expect("startup database failure");
    let failure = slot.start_reloaded(fresh, &lettuce_types::JobId::new().to_string()).await.expect_err("startup fails");
    assert_eq!(failure.code, lettuce_contracts::ApiErrorCode::Unavailable);
    let failure = slot.snapshot().expect_err("reopen required");
    assert_eq!(failure.details, Some(lettuce_contracts::ApiErrorDetails::Backup { reason: lettuce_contracts::BackupFailureReason::RecoveryRequired }));
    assert!(events.events().iter().all(|event| !matches!(event, lettuce_contracts::ApiEvent::DatabaseReloaded)));
}
