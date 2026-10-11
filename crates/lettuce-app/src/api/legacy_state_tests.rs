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
