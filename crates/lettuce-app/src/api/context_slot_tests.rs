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
