use std::sync::Arc;

use lettuce_jobs::handle::CancellationToken;

use super::maintenance::MaintenanceGate;

#[tokio::test]
async fn maintenance_waits_for_current_work_and_holds_new_work_until_released() {
    let gate = Arc::new(MaintenanceGate::default());
    let shutdown = CancellationToken::new();
    let active = gate.work(&shutdown).await.expect("active work");
    let started = Arc::new(tokio::sync::Notify::new());
    let waiting = {
        let gate = gate.clone();
        let shutdown = shutdown.clone();
        let started = started.clone();
        tokio::spawn(async move {
            started.notify_one();
            gate.maintenance(&shutdown).await
        })
    };
    started.notified().await;
    tokio::task::yield_now().await;
    assert!(!waiting.is_finished());
    drop(active);
    let maintenance = waiting.await.expect("maintenance task").expect("lease");
    let newcomer = {
        let gate = gate.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move { gate.work(&shutdown).await })
    };
    tokio::task::yield_now().await;
    assert!(!newcomer.is_finished());
    drop(maintenance);
    newcomer.await.expect("new task").expect("released work");
}

#[tokio::test]
async fn cancelled_maintenance_waiter_does_not_hold_future_work() {
    let gate = Arc::new(MaintenanceGate::default());
    let shutdown = CancellationToken::new();
    let active = gate.work(&shutdown).await.expect("active work");
    let cancellation = CancellationToken::new();
    let waiting = {
        let gate = gate.clone();
        let cancellation = cancellation.clone();
        tokio::spawn(async move { gate.maintenance(&cancellation).await })
    };
    tokio::task::yield_now().await;
    cancellation.cancel();
    assert!(waiting.await.expect("cancelled task").is_none());
    drop(active);
    gate.work(&shutdown).await.expect("future work");
}

#[tokio::test]
async fn shutdown_refuses_work_and_maintenance_without_consuming_a_lease() {
    let gate = MaintenanceGate::default();
    let shutdown = CancellationToken::new();
    shutdown.cancel();
    assert!(gate.work(&shutdown).await.is_none());
    assert!(gate.maintenance(&shutdown).await.is_none());
    gate.work(&CancellationToken::new())
        .await
        .expect("free lease");
}

#[tokio::test]
async fn optimize_admission_replays_and_queued_cancellation_never_runs() {
    use lettuce_contracts as dto;
    use lettuce_jobs::{JobState, JobStore};
    use lettuce_types::{JobId, RequestId};
    let h = super::tests::harness(super::tests::Reply::Text("ok"));
    let request = dto::StorageOptimizeRequest {
        client_operation_id: RequestId::new().to_string(),
    };
    let (first, replay) = tokio::join!(
        super::storage_optimize(&h.context, request.clone()),
        super::storage_optimize(&h.context, request.clone())
    );
    let first = first.expect("first admission");
    assert_eq!(first, replay.expect("replay"));
    let job_id: JobId = first.job_id.parse().expect("job id");
    super::job_cancel(
        &h.context,
        dto::JobCancelRequest {
            job_id: first.job_id,
        },
    )
    .await
    .expect("cancel queued");
    let runner = super::JobRunner::new(h.context.clone(), super::JobHandlers::standard());
    assert!(!runner.run_once().await.expect("runner"));
    assert_eq!(
        h.context
            .backend()
            .database()
            .get(job_id)
            .expect("job")
            .expect("exists")
            .state,
        JobState::Cancelled
    );
    assert_eq!(
        super::storage_optimize(&h.context, request)
            .await
            .expect("cancelled replay")
            .job_id,
        job_id.to_string()
    );
}

#[tokio::test]
async fn optimize_runner_waits_for_work_and_shutdown_cancels_the_waiter() {
    use lettuce_contracts as dto;
    use lettuce_jobs::{JobState, JobStore};
    use lettuce_types::{JobId, RequestId};
    let h = super::tests::harness(super::tests::Reply::Text("ok"));
    let active = h
        .context
        .maintenance()
        .work(&CancellationToken::new())
        .await
        .expect("running work");
    let accepted = super::storage_optimize(
        &h.context,
        dto::StorageOptimizeRequest {
            client_operation_id: RequestId::new().to_string(),
        },
    )
    .await
    .expect("admission");
    let job_id: JobId = accepted.job_id.parse().expect("job id");
    let runner = super::JobRunner::new(h.context.clone(), super::JobHandlers::standard());
    assert!(runner.run_once().await.expect("started"));
    assert_eq!(
        h.context
            .backend()
            .database()
            .get(job_id)
            .expect("job")
            .expect("exists")
            .state,
        JobState::Running
    );
    h.context.begin_shutdown();
    runner.wait_idle().await;
    assert_eq!(
        h.context
            .backend()
            .database()
            .get(job_id)
            .expect("job")
            .expect("exists")
            .state,
        JobState::Cancelled
    );
    drop(active);
}

#[tokio::test]
async fn optimize_executes_after_work_releases_and_another_worker_cannot_claim_it() {
    use lettuce_contracts as dto;
    use lettuce_jobs::{JobState, JobStore};
    use lettuce_types::{JobId, RequestId};
    let h = super::tests::harness(super::tests::Reply::Text("ok"));
    let active = h
        .context
        .maintenance()
        .work(&CancellationToken::new())
        .await
        .expect("running work");
    let accepted = super::storage_optimize(
        &h.context,
        dto::StorageOptimizeRequest {
            client_operation_id: RequestId::new().to_string(),
        },
    )
    .await
    .expect("admission");
    let job_id: JobId = accepted.job_id.parse().expect("job id");
    let runner = super::JobRunner::new(h.context.clone(), super::JobHandlers::standard());
    assert!(runner.run_once().await.expect("started"));
    let other = super::JobRunner::new(h.context.clone(), super::JobHandlers::standard());
    assert!(!other.run_once().await.expect("claimed elsewhere"));
    assert_eq!(
        h.context
            .backend()
            .database()
            .get(job_id)
            .expect("job")
            .expect("exists")
            .state,
        JobState::Running
    );
    drop(active);
    runner.wait_idle().await;
    let view = super::job_get(
        &h.context,
        dto::JobGetRequest {
            job_id: job_id.to_string(),
        },
    )
    .await
    .expect("view");
    assert_eq!(view.state, dto::JobStateDto::Succeeded);
    assert_eq!(view.result, Some(dto::JobResultDto::StorageOptimized));
}

#[tokio::test]
async fn optimize_rejects_a_receipt_with_a_different_digest() {
    use lettuce_contracts as dto;
    use lettuce_jobs::{JobKind, JobSpec, JobSubject, OutcomeRef, SubjectKind};
    use lettuce_types::RequestId;
    let h = super::tests::harness(super::tests::Reply::Text("ok"));
    let id = RequestId::new();
    h.context
        .backend()
        .database()
        .admit_job_with_detail(
            JobSpec::new(
                JobKind::Maintenance,
                JobSubject::new(SubjectKind::Maintenance, "storage-optimize").expect("subject"),
                OutcomeRef::Request(id),
            )
            .with_resources(vec![lettuce_jobs::ResourceClass::DiskWrite]),
            &format!("storage-optimize:{id}"),
            "different-digest",
            &serde_json::json!({"kind": "storage_optimize"}),
        )
        .expect("previous receipt");
    let error = super::storage_optimize(
        &h.context,
        dto::StorageOptimizeRequest {
            client_operation_id: id.to_string(),
        },
    )
    .await
    .expect_err("conflicting receipt");
    assert_eq!(error.code, dto::ApiErrorCode::Conflict);
}

#[tokio::test]
async fn optimize_orphaned_claim_requeues_and_reuses_the_original_receipt() {
    use lettuce_contracts as dto;
    use lettuce_jobs::{JobMutation, JobState, JobStore, ResourceAvailability, WorkerId};
    use lettuce_types::{JobId, RequestId};
    let h = super::tests::harness(super::tests::Reply::Text("ok"));
    let request = dto::StorageOptimizeRequest {
        client_operation_id: RequestId::new().to_string(),
    };
    let accepted = super::storage_optimize(&h.context, request.clone())
        .await
        .expect("admit");
    let job_id: JobId = accepted.job_id.parse().expect("id");
    let database = h.context.backend().database();
    let claim = database
        .claim(
            job_id,
            WorkerId::new(),
            h.context.now(),
            std::time::Duration::from_secs(60),
            &ResourceAvailability::all(),
        )
        .expect("claim")
        .expect("claimed");
    database
        .append_and_transition(JobMutation::Start {
            claim: claim.claim,
            at: h.context.now(),
        })
        .expect("started before crash");
    h.context.recover_after_restart().expect("restart recovery");
    assert_eq!(
        database.get(job_id).expect("job").expect("exists").state,
        JobState::Queued
    );
    assert_eq!(
        super::storage_optimize(&h.context, request)
            .await
            .expect("receipt"),
        accepted
    );
    let runner = super::JobRunner::new(h.context.clone(), super::JobHandlers::standard());
    assert!(runner.run_once().await.expect("restarted"));
    runner.wait_idle().await;
    assert_eq!(
        database.get(job_id).expect("job").expect("exists").state,
        JobState::Succeeded
    );
}

#[tokio::test]
async fn optimize_waits_for_restore_file_lifecycle_and_cancels_without_writing() {
    use super::tests::{NoImages, Reply, harness_over_files};
    use super::{ApiDatabaseFiles, NoModels};
    use crate::{AppBackend, AppDatabaseLocation};
    use lettuce_contracts as dto;
    use lettuce_jobs::{JobState, JobStore, SystemClock};
    use lettuce_platform::{DirectorySnapshot, FilesystemAuthority};
    use lettuce_types::{JobId, OperationId, RequestId, TimestampMillis};
    let root =
        std::env::temp_dir().join(format!("lettuce-optimize-restore-{}", OperationId::new()));
    let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
        .expect("authority");
    let location =
        AppDatabaseLocation::new(root.join("private-persistent-v2"), &authority).expect("location");
    let active = location.active_path().expect("active");
    let backend = Arc::new(AppBackend::open(&active, TimestampMillis::new(1)).expect("backend"));
    let restore = location.file_lifecycle().await.expect("restore lifecycle");
    let h = harness_over_files(
        backend,
        Reply::Text("ok"),
        Arc::new(SystemClock),
        None,
        Some(root.clone()),
        Arc::new(NoModels),
        Arc::new(NoImages),
        Some(ApiDatabaseFiles { location, active }),
    );
    let accepted = super::storage_optimize(
        &h.context,
        dto::StorageOptimizeRequest {
            client_operation_id: RequestId::new().to_string(),
        },
    )
    .await
    .expect("admit");
    let job_id: JobId = accepted.job_id.parse().expect("id");
    let runner = super::JobRunner::new(h.context.clone(), super::JobHandlers::standard());
    assert!(runner.run_once().await.expect("started"));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), runner.wait_idle())
            .await
            .is_err()
    );
    assert_eq!(
        h.context
            .backend()
            .database()
            .get(job_id)
            .expect("job")
            .expect("exists")
            .state,
        JobState::Running
    );
    super::job_cancel(
        &h.context,
        dto::JobCancelRequest {
            job_id: accepted.job_id,
        },
    )
    .await
    .expect("cancel");
    drop(restore);
    runner.wait_idle().await;
    assert_eq!(
        h.context
            .backend()
            .database()
            .get(job_id)
            .expect("job")
            .expect("exists")
            .state,
        JobState::Cancelled
    );
    drop(runner);
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}
