use std::sync::Arc;

use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_jobs::SystemClock;
use lettuce_transfer::LegacyDatabasePreflightError;
use lettuce_types::OperationId;

use super::tests::{Reply, harness_in};

#[tokio::test]
async fn bin_storage_reports_upgrade_without_reading_it() {
    let root = std::env::temp_dir().join(format!("lettuce-legacy-state-{}", OperationId::new()));
    std::fs::create_dir_all(root.join("lettuce")).expect("folder");
    std::fs::write(root.join("lettuce/settings.bin"), b"old storage").expect("bin");
    let harness = harness_in(Reply::Text("Hello"), Arc::new(SystemClock), None, Some(root.clone()), Arc::new(super::NoModels));
    let workers = super::startup(&harness.context).await.expect("startup");
    workers.started().await;
    assert_eq!(super::app_status(&harness.context).await.expect("status").legacy_state, dto::LegacyState::RequiresLegacyUpgrade { reason: dto::LegacyUpgradeReason::BinStorage, required_app_version: "2.2.5".into() });
    assert_eq!(super::legacy_state(&harness.context).await.expect("state"), dto::LegacyState::RequiresLegacyUpgrade { reason: dto::LegacyUpgradeReason::BinStorage, required_app_version: "2.2.5".into() });
    assert_eq!(std::fs::read(root.join("lettuce/settings.bin")).expect("retained"), b"old storage");
    std::fs::remove_file(root.join("lettuce/settings.bin")).expect("remove bin");
    assert_eq!(super::legacy_state(&harness.context).await.expect("absent"), dto::LegacyState::Absent);
    workers.stop().await;
    std::fs::remove_dir_all(root).expect("remove folder");
}

#[test]
fn schema_91_maps_to_upgrade_and_newer_or_invalid_sources_fail_typed() {
    assert_eq!(super::legacy_state::failure_state(LegacyDatabasePreflightError::UnsupportedVersion { found: 91, minimum: 92, maximum: 96 }).expect("upgrade"), dto::LegacyState::RequiresLegacyUpgrade { reason: dto::LegacyUpgradeReason::Schema { found: 91 }, required_app_version: "2.2.5".into() });
    let newer = super::legacy_state::failure_state(LegacyDatabasePreflightError::UnsupportedVersion { found: 97, minimum: 92, maximum: 96 }).expect_err("newer");
    assert_eq!(newer.code, ApiErrorCode::Unsupported);
    assert!(newer.details.is_some());
    let unavailable = super::legacy_state::failure_state(LegacyDatabasePreflightError::Unavailable).expect_err("unavailable");
    assert_eq!(unavailable.code, ApiErrorCode::Unavailable);
    assert!(unavailable.details.is_some());
}

#[tokio::test]
async fn fix11a_truncated_legacy_database_does_not_block_startup_or_status() {
    invalid_legacy_startup(b"truncated SQLite", "malformed", false).await;
}

#[tokio::test]
async fn fix11a_newer_legacy_database_does_not_block_startup_or_status() {
    invalid_legacy_startup(include_bytes!("../../../lettuce-database/tests/fixtures/legacy-schema-97.db"), "unsupported", false).await;
}

#[cfg(unix)]
#[tokio::test]
async fn fix11a_unreadable_legacy_database_does_not_block_startup_or_status() {
    invalid_legacy_startup(b"unreadable SQLite", "unavailable", true).await;
}

async fn invalid_legacy_startup(bytes: &[u8], code: &str, unreadable: bool) {
    let (root, _location, harness) = super::app_reset_tests::harness();
    let legacy = root.join("lettuce/app.db");
    std::fs::create_dir_all(legacy.parent().expect("parent")).expect("legacy folder");
    std::fs::write(&legacy, bytes).expect("legacy database");
    #[cfg(unix)]
    if unreadable {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&legacy, std::fs::Permissions::from_mode(0o0)).expect("unreadable");
    }
    let result = super::startup(&harness.context).await;
    #[cfg(unix)]
    if unreadable && result.is_err() {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&legacy, std::fs::Permissions::from_mode(0o600)).expect("cleanup permissions");
    }
    let workers = result.expect("legacy detection cannot prevent startup");
    workers.started_result().await.expect("workers ready");
    let status = super::app_status(&harness.context).await.expect("status remains available");
    let state = super::legacy_state(&harness.context).await.expect("typed state");
    assert_eq!(status.legacy_state, state);
    let state = serde_json::to_value(state).expect("state JSON");
    assert_eq!(state["type"], "failed");
    assert_eq!(state["error"]["code"], code);
    assert!(!status.legacy_database_detected);
    #[cfg(unix)]
    if unreadable {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&legacy, std::fs::Permissions::from_mode(0o600)).expect("cleanup permissions");
    }
    assert_eq!(std::fs::read(legacy).expect("legacy kept"), bytes);
    workers.stop_for_restore().await.expect("stop");
}
