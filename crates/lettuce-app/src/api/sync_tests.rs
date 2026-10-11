use lettuce_contracts::{self as dto, ApiErrorCode};

use super::tests::{Harness, Reply, harness_over};
use super::*;

fn device() -> (Harness, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!("sync-api-{}", lettuce_types::OperationId::new()));
    std::fs::create_dir_all(&root).expect("root");
    let backend = std::sync::Arc::new(
        crate::AppBackend::open(
            root.join("state.sqlite3"),
            lettuce_types::TimestampMillis::new(1),
        )
        .expect("backend"),
    );
    let harness = harness_over(
        backend,
        Reply::Text("unused"),
        std::sync::Arc::new(lettuce_jobs::SystemClock),
        None,
        Some(root.clone()),
        std::sync::Arc::new(NoModels),
        std::sync::Arc::new(super::tests::NoImages),
    );
    (harness, root)
}

async fn wait_status(
    context: &ApiContext,
    predicate: impl Fn(&dto::SyncStatus) -> bool,
) -> dto::SyncStatus {
    let mut status = context.sync_state().subscribe();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let current = status.borrow_and_update().clone();
            if predicate(&current) {
                return current;
            }
            status.changed().await.expect("status sender");
        }
    })
    .await
    .expect("state reached")
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_host_requires_both_steps_and_both_devices_complete() {
    let (host, host_root) = device();
    let (client, client_root) = device();
    let sharing = sync_share_start(&host.context).await.expect("share");
    assert_eq!(
        sync_share_start(&host.context)
            .await
            .expect_err("second share")
            .code,
        ApiErrorCode::Busy
    );
    sync_connect(
        &client.context,
        dto::SyncConnectRequest {
            address: "127.0.0.1".into(),
            port: sharing.port,
            pin: sharing.pin.clone(),
        },
    )
    .await
    .expect("connect");
    wait_status(&host.context, |status| {
        matches!(status, dto::SyncStatus::PendingApproval { .. })
    })
    .await;
    sync_share_approve(
        &host.context,
        dto::SyncShareApproveRequest { approve: true },
    )
    .await
    .expect("approve");
    wait_status(&host.context, |status| {
        matches!(status, dto::SyncStatus::PendingStart { .. })
    })
    .await;
    assert!(matches!(
        sync_status(&client.context).await.expect("client waiting"),
        dto::SyncStatus::Connecting
    ));
    sync_share_begin(&host.context).await.expect("begin");
    wait_status(&host.context, |status| {
        matches!(status, dto::SyncStatus::Completed { .. })
    })
    .await;
    wait_status(&client.context, |status| {
        matches!(status, dto::SyncStatus::Completed { .. })
    })
    .await;
    let states = host
        .events
        .events()
        .into_iter()
        .filter_map(|event| match event {
            dto::ApiEvent::SyncStatusChanged { status } => Some(status),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        states.first(),
        Some(dto::SyncStatus::Sharing { .. })
    ));
    assert!(matches!(
        states.get(1),
        Some(dto::SyncStatus::PendingApproval { .. })
    ));
    assert!(matches!(
        states.get(2),
        Some(dto::SyncStatus::PendingStart { .. })
    ));
    assert!(matches!(
        states.last(),
        Some(dto::SyncStatus::Completed { .. })
    ));
    assert!(states.iter().any(|status| matches!(status, dto::SyncStatus::Syncing { progress, .. } if progress.items_done > 0)), "committed progress must be visible");
    sync_stop(&host.context).await.expect("stop host");
    sync_stop(&client.context).await.expect("stop client");
    std::fs::remove_dir_all(host_root).expect("host cleanup");
    std::fs::remove_dir_all(client_root).expect("client cleanup");
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_decline_and_disconnect_release_both_sessions_for_retry() {
    for begin_pending in [false, true] {
        let (host, host_root) = device();
        let (client, client_root) = device();
        let sharing = sync_share_start(&host.context).await.expect("share");
        sync_connect(
            &client.context,
            dto::SyncConnectRequest {
                address: "127.0.0.1".into(),
                port: sharing.port,
                pin: sharing.pin,
            },
        )
        .await
        .expect("connect");
        wait_status(&host.context, |status| {
            matches!(status, dto::SyncStatus::PendingApproval { .. })
        })
        .await;
        if begin_pending {
            sync_share_approve(
                &host.context,
                dto::SyncShareApproveRequest { approve: true },
            )
            .await
            .expect("approve");
            wait_status(&host.context, |status| {
                matches!(status, dto::SyncStatus::PendingStart { .. })
            })
            .await;
        }
        sync_stop(&client.context).await.expect("disconnect");
        wait_status(&host.context, |status| {
            matches!(status, dto::SyncStatus::Failed { .. })
        })
        .await;
        assert_eq!(
            sync_status(&client.context).await.expect("stopped"),
            dto::SyncStatus::Idle
        );
        sync_stop(&host.context).await.expect("stop host");
        sync_share_start(&host.context).await.expect("host retries");
        sync_stop(&host.context).await.expect("stop retry");
        std::fs::remove_dir_all(host_root).expect("host cleanup");
        std::fs::remove_dir_all(client_root).expect("client cleanup");
    }
}

#[tokio::test]
async fn sync_session_commands_reject_invalid_state_without_receipts() {
    let (host, root) = device();
    assert_eq!(
        sync_share_approve(
            &host.context,
            dto::SyncShareApproveRequest { approve: true }
        )
        .await
        .expect_err("no approval")
        .code,
        ApiErrorCode::Conflict
    );
    assert_eq!(
        sync_share_begin(&host.context)
            .await
            .expect_err("no start")
            .code,
        ApiErrorCode::Conflict
    );
    assert_eq!(
        sync_stop(&host.context).await.expect("idle stop"),
        dto::SyncStatus::Idle
    );
    assert_eq!(
        sync_status(&host.context).await.expect("status"),
        dto::SyncStatus::Idle
    );
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_decline_is_typed_on_both_devices_and_approval_has_no_deadline() {
    let (host, host_root) = device();
    let (client, client_root) = device();
    host.context
        .sync_state()
        .set_test_read_deadline(std::time::Duration::from_millis(20));
    client
        .context
        .sync_state()
        .set_test_read_deadline(std::time::Duration::from_millis(20));
    let sharing = sync_share_start(&host.context).await.expect("share");
    let request = dto::SyncConnectRequest {
        address: "127.0.0.1".into(),
        port: sharing.port,
        pin: sharing.pin,
    };
    sync_connect(&client.context, request.clone())
        .await
        .expect("connect");
    wait_status(&host.context, |status| {
        matches!(status, dto::SyncStatus::PendingApproval { .. })
    })
    .await;
    assert_eq!(
        sync_connect(&client.context, request)
            .await
            .expect_err("second connect")
            .code,
        ApiErrorCode::Busy
    );
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(matches!(
        sync_status(&host.context)
            .await
            .expect("unbounded approval"),
        dto::SyncStatus::PendingApproval { .. }
    ));
    sync_share_approve(
        &host.context,
        dto::SyncShareApproveRequest { approve: false },
    )
    .await
    .expect("decline");
    for context in [&host.context, &client.context] {
        let status = wait_status(context, |status| {
            matches!(status, dto::SyncStatus::Failed { .. })
        })
        .await;
        let dto::SyncStatus::Failed { reason } = status else {
            panic!("failed state");
        };
        assert_eq!(
            reason.details,
            Some(dto::ApiErrorDetails::Sync {
                reason: dto::SyncFailure::Declined
            })
        );
        sync_share_start(context)
            .await
            .expect("retry after decline");
        sync_stop(context).await.expect("stop retry");
    }
    std::fs::remove_dir_all(host_root).expect("host cleanup");
    std::fs::remove_dir_all(client_root).expect("client cleanup");
}

#[tokio::test]
async fn sync_holds_maintenance_work_and_restore_excludes_new_sessions() {
    let (host, root) = device();
    let cancellation = lettuce_jobs::handle::CancellationToken::new();
    let lease = host
        .context
        .maintenance()
        .maintenance(&cancellation)
        .await
        .expect("maintenance");
    assert_eq!(
        sync_share_start(&host.context)
            .await
            .expect_err("maintenance busy")
            .code,
        ApiErrorCode::Busy
    );
    drop(lease);
    sync_share_start(&host.context).await.expect("share");
    let waiting = host.context.maintenance().maintenance(&cancellation);
    tokio::pin!(waiting);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(20), &mut waiting)
            .await
            .is_err()
    );
    sync_stop(&host.context).await.expect("stop releases lease");
    let lease = waiting.await.expect("restore proceeds");
    drop(lease);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test(flavor = "multi_thread")]
async fn shutdown_during_pending_start_cancels_the_session_and_keeps_the_database() {
    let (host, host_root) = device();
    let (client, client_root) = device();
    let sharing = sync_share_start(&host.context).await.expect("share");
    sync_connect(
        &client.context,
        dto::SyncConnectRequest {
            address: "127.0.0.1".into(),
            port: sharing.port,
            pin: sharing.pin,
        },
    )
    .await
    .expect("connect");
    wait_status(&host.context, |status| {
        matches!(status, dto::SyncStatus::PendingApproval { .. })
    })
    .await;
    sync_share_approve(
        &host.context,
        dto::SyncShareApproveRequest { approve: true },
    )
    .await
    .expect("approve");
    wait_status(&host.context, |status| {
        matches!(status, dto::SyncStatus::PendingStart { .. })
    })
    .await;
    host.context.begin_shutdown();
    host.context.stop_sync_session().await;
    assert_eq!(
        sync_status(&host.context).await.expect("shutdown state"),
        dto::SyncStatus::Idle
    );
    wait_status(&client.context, |status| {
        matches!(status, dto::SyncStatus::Failed { .. })
    })
    .await;
    assert!(
        lettuce_characters::CharacterRepository::get(
            host.context.backend().database(),
            host.character_id
        )
        .expect("character")
        .is_some()
    );
    sync_stop(&client.context).await.expect("stop client");
    std::fs::remove_dir_all(host_root).expect("host cleanup");
    std::fs::remove_dir_all(client_root).expect("client cleanup");
}

async fn complete(host: &ApiContext, client: &ApiContext) {
    let sharing = sync_share_start(host).await.expect("share");
    sync_connect(
        client,
        dto::SyncConnectRequest {
            address: "127.0.0.1".into(),
            port: sharing.port,
            pin: sharing.pin,
        },
    )
    .await
    .expect("connect");
    wait_status(host, |status| {
        matches!(status, dto::SyncStatus::PendingApproval { .. })
    })
    .await;
    sync_share_approve(host, dto::SyncShareApproveRequest { approve: true })
        .await
        .expect("approve");
    sync_share_begin(host).await.expect("begin");
    wait_status(host, |status| {
        matches!(status, dto::SyncStatus::Completed { .. })
    })
    .await;
    wait_status(client, |status| {
        matches!(status, dto::SyncStatus::Completed { .. })
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_conflict_api_replays_propagates_and_rejects_changed_or_stale_requests() {
    let (host, host_root) = device();
    let (client, client_root) = device();
    complete(&host.context, &client.context).await;
    for (context, name) in [
        (&host.context, "Host edit"),
        (&client.context, "Client edit"),
    ] {
        let view = character_get(
            context,
            dto::CharacterGetRequest {
                character_id: host.character_id.to_string(),
            },
        )
        .await
        .expect("character");
        let mut draft = view.draft;
        draft.profile.name = name.into();
        character_save(
            context,
            dto::CharacterSaveRequest {
                client_operation_id: "edit-before-sync".into(),
                character_id: Some(view.id),
                expected_revision: Some(view.revision),
                draft,
            },
        )
        .await
        .expect("edit");
    }
    complete(&host.context, &client.context).await;
    let conflict = sync_conflicts_list(&host.context)
        .await
        .expect("inbox")
        .conflicts
        .into_iter()
        .find(|conflict| conflict.entity_id == host.character_id.to_string())
        .expect("character conflict");
    complete(&host.context, &client.context).await;
    assert!(
        matches!(
            sync_status(&host.context)
                .await
                .expect("repeated completion"),
            dto::SyncStatus::Completed { conflicts: 0, .. }
        ),
        "old inbox conflicts are not new session conflicts"
    );
    let request = dto::SyncConflictResolveRequest {
        client_operation_id: "choose".into(),
        conflict_id: conflict.conflict_id,
        expected_current_change: conflict.current.change_id,
        choice: dto::SyncConflictChoice::Other,
    };
    let result = sync_conflict_resolve(&host.context, request.clone())
        .await
        .expect("resolve");
    assert_eq!(
        sync_conflict_resolve(&host.context, request.clone())
            .await
            .expect("replay"),
        result
    );
    let mut changed = request.clone();
    changed.choice = dto::SyncConflictChoice::Current;
    assert_eq!(
        sync_conflict_resolve(&host.context, changed)
            .await
            .expect_err("changed digest")
            .code,
        ApiErrorCode::Conflict
    );
    let mut stale = request;
    stale.client_operation_id = "stale".into();
    assert_eq!(
        sync_conflict_resolve(&host.context, stale)
            .await
            .expect_err("stale inbox")
            .code,
        ApiErrorCode::Conflict
    );
    complete(&host.context, &client.context).await;
    assert!(
        sync_conflicts_list(&client.context)
            .await
            .expect("peer resolved")
            .conflicts
            .into_iter()
            .all(|conflict| conflict.entity_id != host.character_id.to_string())
    );
    sync_stop(&host.context).await.expect("stop host");
    sync_stop(&client.context).await.expect("stop client");
    std::fs::remove_dir_all(host_root).expect("host cleanup");
    std::fs::remove_dir_all(client_root).expect("client cleanup");
}

async fn peer_held_in_phase(
    host: &ApiContext,
    peer: &ApiContext,
    phase: dto::SyncPhase,
) -> (
    tokio::task::JoinHandle<()>,
    tokio::sync::oneshot::Receiver<()>,
    tokio::sync::oneshot::Sender<()>,
) {
    use crate::AuthenticatedSyncTransport;
    let sharing = sync_share_start(host).await.expect("share");
    let (reached, reached_rx) = tokio::sync::oneshot::channel();
    let (release, release_rx) = tokio::sync::oneshot::channel();
    let peer = peer.clone();
    let task = tokio::spawn(async move {
        peer.blocking(move |context| {
            tokio::runtime::Handle::current().block_on(async {
                let cancellation = lettuce_jobs::handle::CancellationToken::new();
                let database = context.backend().database();
                let media = lettuce_media::LocalSyncMediaStore::open(
                    context
                        .app_folder()
                        .expect("folder")
                        .join("platform-v2/media-blobs"),
                    database,
                    database,
                )
                .expect("media");
                let hello = context
                    .backend()
                    .sync_hello()
                    .build(
                        env!("CARGO_PKG_VERSION"),
                        "Controlled peer",
                        lettuce_sync::SyncSessionId::new(),
                        Default::default(),
                        context.now(),
                    )
                    .expect("hello");
                let mut transport = crate::connect_authenticated_sync(
                    ([127, 0, 0, 1], sharing.port).into(),
                    hello.device_id(),
                    &crate::PairingPin::new(sharing.pin).expect("pin"),
                    Some(&media),
                    &cancellation,
                )
                .await
                .expect("pair");
                if phase == dto::SyncPhase::Changes {
                    transport
                        .exchange_hello(hello, &cancellation)
                        .await
                        .expect("hello");
                    transport
                        .exchange_frontier(
                            lettuce_sync::LocalChangeJournal::local_frontier(database)
                                .expect("frontier"),
                            &cancellation,
                        )
                        .await
                        .expect("frontier");
                } else {
                    assert!(matches!(
                        context
                            .backend()
                            .sync_exchange()
                            .run(hello, &mut transport, &cancellation, context.now())
                            .await
                            .expect("changes"),
                        crate::SyncExchangeOutcome::Complete(_)
                    ));
                    if phase == dto::SyncPhase::Secrets {
                        context
                            .backend()
                            .sync_media(&media)
                            .run(&mut transport, &cancellation, context.now())
                            .await
                            .expect("media");
                    }
                }
                reached.send(()).expect("phase observer");
                let _ = release_rx.await;
                Ok(())
            })
        })
        .await
        .expect("controlled peer");
    });
    wait_status(host, |status| {
        matches!(status, dto::SyncStatus::PendingApproval { .. })
    })
    .await;
    sync_share_approve(host, dto::SyncShareApproveRequest { approve: true })
        .await
        .expect("approve");
    sync_share_begin(host).await.expect("begin");
    (task, reached_rx, release)
}

#[tokio::test(flavor = "multi_thread")]
async fn stop_disconnect_and_stall_in_every_transfer_phase_keep_data_and_allow_retry() {
    for phase in [
        dto::SyncPhase::Changes,
        dto::SyncPhase::Media,
        dto::SyncPhase::Secrets,
    ] {
        for action in ["stop", "disconnect", "stall"] {
            let (host, host_root) = device();
            let (peer, peer_root) = device();
            host.context
                .sync_state()
                .set_test_read_deadline(std::time::Duration::from_millis(200));
            let (task, reached, release) =
                peer_held_in_phase(&host.context, &peer.context, phase.clone()).await;
            tokio::time::timeout(std::time::Duration::from_secs(10), reached)
                .await
                .expect("phase deadline")
                .expect("phase");
            let mut release = Some(release);
            if action == "stop" {
                sync_stop(&host.context).await.expect("local stop");
                assert_eq!(
                    sync_status(&host.context).await.expect("local idle"),
                    dto::SyncStatus::Idle
                );
                release
                    .take()
                    .expect("sender")
                    .send(())
                    .expect("release peer");
            } else {
                if action == "disconnect" {
                    release.take().expect("sender").send(()).expect("drop peer");
                }
                let status = wait_status(&host.context, |status| {
                    matches!(status, dto::SyncStatus::Failed { .. })
                })
                .await;
                let dto::SyncStatus::Failed { reason } = status else {
                    panic!("failed");
                };
                let expected = if action == "disconnect" {
                    dto::SyncFailure::Disconnected
                } else {
                    dto::SyncFailure::PeerStalled
                };
                assert_eq!(
                    reason.details,
                    Some(dto::ApiErrorDetails::Sync { reason: expected })
                );
                if action == "stall" {
                    release
                        .take()
                        .expect("sender")
                        .send(())
                        .expect("release stalled peer");
                }
            }
            task.await.expect("peer task");
            assert!(
                lettuce_characters::CharacterRepository::get(
                    host.context.backend().database(),
                    host.character_id
                )
                .expect("host data")
                .is_some()
            );
            assert!(
                lettuce_characters::CharacterRepository::get(
                    peer.context.backend().database(),
                    peer.character_id
                )
                .expect("peer data")
                .is_some()
            );
            host.context
                .sync_state()
                .set_test_read_deadline(std::time::Duration::from_secs(60));
            complete(&host.context, &peer.context).await;
            sync_stop(&host.context).await.expect("host stop");
            sync_stop(&peer.context).await.expect("peer stop");
            std::fs::remove_dir_all(host_root).expect("host cleanup");
            std::fs::remove_dir_all(peer_root).expect("peer cleanup");
        }
    }
}

#[tokio::test]
async fn shutdown_rejects_every_persistent_sync_command_without_a_receipt() {
    let (host, root) = device();
    host.context.begin_shutdown();
    let error = sync_conflict_resolve(
        &host.context,
        dto::SyncConflictResolveRequest {
            client_operation_id: "cancel-resolve".into(),
            conflict_id: lettuce_types::OperationId::new().to_string(),
            expected_current_change: None,
            choice: dto::SyncConflictChoice::Current,
        },
    )
    .await
    .expect_err("canceled resolve");
    assert_eq!(error.code, ApiErrorCode::Cancelled);
    let error = sync_fork_resolve(
        &host.context,
        dto::SyncForkResolveRequest {
            client_operation_id: "cancel-fork".into(),
            conversation_id: lettuce_types::ConversationId::new().to_string(),
            branch_id: lettuce_types::ConversationBranchId::new().to_string(),
        },
    )
    .await
    .expect_err("canceled fork");
    assert_eq!(error.code, ApiErrorCode::Cancelled);
    let error = legacy_conflict_dismiss(
        &host.context,
        dto::LegacyConflictDismissRequest {
            client_operation_id: "cancel-dismiss".into(),
            run_id: lettuce_types::LegacyImportRunId::new().to_string(),
            source_key: "legacy".into(),
        },
    )
    .await
    .expect_err("canceled dismiss");
    assert_eq!(error.code, ApiErrorCode::Cancelled);
    for (command, key) in [
        ("sync_conflict_resolve", "cancel-resolve"),
        ("sync_fork_resolve", "cancel-fork"),
        ("legacy_conflict_dismiss", "cancel-dismiss"),
    ] {
        assert!(
            host.context
                .backend()
                .database()
                .lookup_api_operation(command, key)
                .expect("receipt")
                .is_none()
        );
    }
    assert!(
        lettuce_characters::CharacterRepository::get(
            host.context.backend().database(),
            host.character_id
        )
        .expect("data kept")
        .is_some()
    );
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_staged_unsupported_batch_survives_shutdown_and_reopen_without_losing_entities() {
    use lettuce_sync::{IncomingChangeRepository, LocalChangeJournal};
    let (host, host_root) = device();
    let (peer, peer_root) = device();
    complete(&host.context, &peer.context).await;
    let operation = lettuce_types::OperationId::new();
    let change = peer
        .context
        .backend()
        .database()
        .record_local_change(
            operation,
            lettuce_sync::NewCanonicalChange::new(
                lettuce_sync::SyncEntity::new("future", "future-1").expect("entity"),
                lettuce_sync::ChangeOperation::Insert,
                None,
                Some(
                    lettuce_sync::CanonicalPayload::new("future.snapshot", 1, b"future".to_vec())
                        .expect("payload"),
                ),
            )
            .expect("request"),
            peer.context.now(),
        )
        .expect("future journal");
    let sharing = sync_share_start(&host.context).await.expect("share");
    sync_connect(
        &peer.context,
        dto::SyncConnectRequest {
            address: "127.0.0.1".into(),
            port: sharing.port,
            pin: sharing.pin,
        },
    )
    .await
    .expect("connect");
    wait_status(&host.context, |status| {
        matches!(status, dto::SyncStatus::PendingApproval { .. })
    })
    .await;
    sync_share_approve(
        &host.context,
        dto::SyncShareApproveRequest { approve: true },
    )
    .await
    .expect("approve");
    sync_share_begin(&host.context).await.expect("begin");
    let status = wait_status(&host.context, |status| {
        matches!(status, dto::SyncStatus::Failed { .. })
    })
    .await;
    let dto::SyncStatus::Failed { reason } = status else {
        panic!("failed");
    };
    let Some(dto::ApiErrorDetails::Sync {
        reason: dto::SyncFailure::PendingBatch { batch_id },
    }) = reason.details
    else {
        panic!("typed pending batch");
    };
    host.context.begin_shutdown();
    host.context.stop_sync_session().await;
    let reopened = crate::AppBackend::open(host_root.join("state.sqlite3"), host.context.now())
        .expect("reopen");
    assert_eq!(
        reopened
            .database()
            .apply_incoming_batch(batch_id.parse().expect("batch id"), host.context.now())
            .expect("retained batch")
            .state,
        lettuce_sync::IncomingBatchState::Pending
    );
    assert!(
        lettuce_characters::CharacterRepository::get(reopened.database(), host.character_id)
            .expect("host data")
            .is_some()
    );
    assert_eq!(
        peer.context
            .backend()
            .database()
            .local_change_for_operation(operation)
            .expect("peer journal"),
        Some(change.change)
    );
    sync_stop(&peer.context).await.expect("stop peer");
    std::fs::remove_dir_all(host_root).expect("host cleanup");
    std::fs::remove_dir_all(peer_root).expect("peer cleanup");
}

async fn seed_media(
    context: &ApiContext,
    character_id: lettuce_types::CharacterId,
    size: usize,
) -> lettuce_sync::CanonicalMediaAsset {
    let root = context.app_folder().expect("root");
    let authority = lettuce_platform::FilesystemAuthority::new(
        &lettuce_platform::DirectorySnapshot::new(root).expect("snapshot"),
    )
    .expect("authority");
    let database = context.backend().database();
    let store = lettuce_media::LocalMediaBlobStore::new(
        authority.managed_files(),
        authority
            .read_capability(lettuce_platform::ManagedRoot::MediaBlobs)
            .expect("read"),
        authority
            .write_capability(lettuce_platform::ManagedRoot::MediaBlobs)
            .expect("write"),
        database,
        database,
    );
    let mut bytes = super::tests::png_bytes();
    bytes.resize(bytes.len().max(size), 0x5a);
    let uploaded = store
        .ingest(
            bytes.as_slice(),
            lettuce_media::IngestRequest::new(
                lettuce_media::AssetKind::AvatarOriginal,
                lettuce_media::AssetOrigin::Upload,
                lettuce_media::RetentionClass::Persistent,
                Default::default(),
            ),
        )
        .expect("upload");
    let before = character_get(
        context,
        dto::CharacterGetRequest {
            character_id: character_id.to_string(),
        },
    )
    .await
    .expect("character");
    let mut draft = before.draft;
    draft.media.push(dto::LibraryCharacterMediaLink {
        asset_id: uploaded.asset.id.to_string(),
        slot: dto::LibraryCharacterMediaSlot::AvatarOriginal,
        ordinal: 0,
    });
    character_save(
        context,
        dto::CharacterSaveRequest {
            client_operation_id: "attach-sync-media".into(),
            character_id: Some(before.id),
            expected_revision: Some(before.revision),
            draft,
        },
    )
    .await
    .expect("attach media");
    lettuce_sync::CanonicalMediaAsset {
        asset: uploaded.asset,
        blob: uploaded.blob,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn completed_reports_unavailable_assets_and_the_deferred_character_retries_next_session() {
    let (host, host_root) = device();
    let (peer, peer_root) = device();
    let asset = seed_media(&peer.context, peer.character_id, 128).await;
    let hash = asset.blob.content_hash.as_str();
    let file = peer_root
        .join("platform-v2/media-blobs/objects")
        .join(&hash[..2])
        .join(&hash[2..4])
        .join(hash);
    let held = peer_root.join("held-blob");
    std::fs::rename(&file, &held).expect("simulate unavailable blob");
    complete(&host.context, &peer.context).await;
    let status = sync_status(&host.context).await.expect("completion");
    assert!(
        matches!(status,dto::SyncStatus::Completed {counts,..} if counts.unavailable_assets == 1)
    );
    assert!(
        lettuce_characters::CharacterRepository::get(
            host.context.backend().database(),
            peer.character_id
        )
        .expect("deferred character")
        .is_none()
    );
    assert_eq!(
        lettuce_sync::MediaSyncRepository::pending_media(host.context.backend().database(), None)
            .expect("deferred asset")
            .len(),
        1
    );
    std::fs::rename(&held, &file).expect("restore peer blob");
    complete(&host.context, &peer.context).await;
    assert!(
        matches!(sync_status(&host.context).await.expect("retry completion"),dto::SyncStatus::Completed {counts,..} if counts.unavailable_assets == 0)
    );
    assert!(
        lettuce_characters::CharacterRepository::get(
            host.context.backend().database(),
            peer.character_id
        )
        .expect("retried character")
        .is_some()
    );
    assert!(
        lettuce_sync::MediaSyncRepository::pending_media(host.context.backend().database(), None)
            .expect("settled asset")
            .is_empty()
    );
    sync_stop(&host.context).await.expect("host stop");
    sync_stop(&peer.context).await.expect("peer stop");
    std::fs::remove_dir_all(host_root).expect("host cleanup");
    std::fs::remove_dir_all(peer_root).expect("peer cleanup");
}

struct BlockAfterFirstChunk<'a> {
    media: &'a lettuce_media::LocalSyncMediaStore<
        &'a lettuce_database::Database,
        &'a lettuce_database::Database,
    >,
    entered: std::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    release: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
}

impl crate::SyncBlobSource for BlockAfterFirstChunk<'_> {
    fn read_chunk(
        &self,
        hash: &lettuce_types::ContentHash,
        offset: u64,
        max_bytes: usize,
    ) -> Result<Vec<u8>, crate::SyncBlobSourceError> {
        if offset >= lettuce_sync::MAX_SYNC_BLOB_CHUNK_BYTES as u64 && max_bytes > 1 {
            if let Some(sender) = self.entered.lock().expect("observer").take() {
                let _ = sender.send(());
                let _ = self.release.lock().expect("release").recv();
            }
        }
        self.media
            .read_sync_chunk(hash, offset, max_bytes)
            .map_err(|_| crate::SyncBlobSourceError)
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn shutdown_mid_media_preserves_the_partial_and_restart_finishes_the_deferred_character() {
    let (host, host_root) = device();
    let (peer, peer_root) = device();
    let asset = seed_media(
        &peer.context,
        peer.character_id,
        3 * lettuce_sync::MAX_SYNC_BLOB_CHUNK_BYTES,
    )
    .await;
    let sharing = sync_share_start(&host.context).await.expect("share");
    let (entered, entered_rx) = tokio::sync::oneshot::channel();
    let (release, release_rx) = std::sync::mpsc::channel();
    let peer_context = peer.context.clone();
    let task = tokio::spawn(async move {
        peer_context
            .blocking(move |context| {
                tokio::runtime::Handle::current().block_on(async {
                    let cancellation = lettuce_jobs::handle::CancellationToken::new();
                    let database = context.backend().database();
                    let media = lettuce_media::LocalSyncMediaStore::open(
                        context
                            .app_folder()
                            .expect("root")
                            .join("platform-v2/media-blobs"),
                        database,
                        database,
                    )
                    .expect("media");
                    let source = BlockAfterFirstChunk {
                        media: &media,
                        entered: std::sync::Mutex::new(Some(entered)),
                        release: std::sync::Mutex::new(release_rx),
                    };
                    let hello = context
                        .backend()
                        .sync_hello()
                        .build(
                            env!("CARGO_PKG_VERSION"),
                            "Media peer",
                            lettuce_sync::SyncSessionId::new(),
                            Default::default(),
                            context.now(),
                        )
                        .expect("hello");
                    let mut transport = crate::connect_authenticated_sync(
                        ([127, 0, 0, 1], sharing.port).into(),
                        hello.device_id(),
                        &crate::PairingPin::new(sharing.pin).expect("pin"),
                        Some(&source),
                        &cancellation,
                    )
                    .await
                    .expect("pair");
                    context
                        .backend()
                        .sync_exchange()
                        .run(hello, &mut transport, &cancellation, context.now())
                        .await
                        .expect("changes");
                    assert!(matches!(
                        context
                            .backend()
                            .sync_media(&media)
                            .run(&mut transport, &cancellation, context.now())
                            .await,
                        Err(crate::SyncMediaExchangeError::Transport(
                            crate::MediaSyncTransportError::Disconnected
                        ))
                    ));
                    Ok(())
                })
            })
            .await
            .expect("controlled media worker");
    });
    wait_status(&host.context, |status| {
        matches!(status, dto::SyncStatus::PendingApproval { .. })
    })
    .await;
    sync_share_approve(
        &host.context,
        dto::SyncShareApproveRequest { approve: true },
    )
    .await
    .expect("approve");
    sync_share_begin(&host.context).await.expect("begin");
    tokio::time::timeout(std::time::Duration::from_secs(10), entered_rx)
        .await
        .expect("second chunk deadline")
        .expect("second chunk reached");
    wait_status(&host.context,|status| matches!(status,dto::SyncStatus::Syncing {phase:dto::SyncPhase::Media,progress} if progress.bytes_done >= lettuce_sync::MAX_SYNC_BLOB_CHUNK_BYTES as u64)).await;
    host.context.begin_shutdown();
    host.context.stop_sync_session().await;
    assert_eq!(
        sync_status(&host.context).await.expect("shutdown idle"),
        dto::SyncStatus::Idle
    );
    release.send(()).expect("release peer");
    task.await.expect("peer task");
    let deferred =
        lettuce_sync::MediaSyncRepository::pending_media(host.context.backend().database(), None)
            .expect("deferred asset");
    assert_eq!(deferred.len(), 1);
    let media = lettuce_media::LocalSyncMediaStore::open(
        host_root.join("platform-v2/media-blobs"),
        host.context.backend().database(),
        host.context.backend().database(),
    )
    .expect("reopen media");
    assert_eq!(
        media.receive_offset(&asset).expect("partial survived"),
        lettuce_sync::MAX_SYNC_BLOB_CHUNK_BYTES as u64
    );
    assert!(
        lettuce_characters::CharacterRepository::get(
            host.context.backend().database(),
            peer.character_id
        )
        .expect("kept deferred character")
        .is_none()
    );
    let restarted = host.context.restarted();
    assert_eq!(
        sync_status(&restarted).await.expect("restart idle"),
        dto::SyncStatus::Idle
    );
    complete(&restarted, &peer.context).await;
    assert!(
        lettuce_characters::CharacterRepository::get(
            restarted.backend().database(),
            peer.character_id
        )
        .expect("restored character")
        .is_some()
    );
    assert!(
        lettuce_sync::MediaSyncRepository::pending_media(restarted.backend().database(), None)
            .expect("settled")
            .is_empty()
    );
    sync_stop(&restarted).await.expect("host stop");
    sync_stop(&peer.context).await.expect("peer stop");
    std::fs::remove_dir_all(host_root).expect("host cleanup");
    std::fs::remove_dir_all(peer_root).expect("peer cleanup");
}

#[tokio::test]
async fn context_retirement_without_workers_joins_the_sync_session() {
    let (host, root) = device();
    sync_share_start(&host.context).await.expect("share");
    ApiContextSlot::retire(&host.context, None)
        .await
        .expect("retire");
    assert_eq!(
        sync_status(&host.context).await.expect("retired status"),
        dto::SyncStatus::Idle
    );
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test(flavor = "multi_thread")]
async fn canceled_pending_start_rejects_begin_without_granting_the_gate() {
    let (host, host_root) = device();
    let (peer, peer_root) = device();
    let sharing = sync_share_start(&host.context).await.expect("share");
    sync_connect(
        &peer.context,
        dto::SyncConnectRequest {
            address: "127.0.0.1".into(),
            port: sharing.port,
            pin: sharing.pin,
        },
    )
    .await
    .expect("connect");
    wait_status(&host.context, |status| {
        matches!(status, dto::SyncStatus::PendingApproval { .. })
    })
    .await;
    sync_share_approve(
        &host.context,
        dto::SyncShareApproveRequest { approve: true },
    )
    .await
    .expect("approve");
    host.context.begin_shutdown();
    assert_eq!(
        sync_share_begin(&host.context)
            .await
            .expect_err("shutdown rejects begin")
            .code,
        ApiErrorCode::Cancelled
    );
    host.context.stop_sync_session().await;
    sync_stop(&peer.context).await.expect("peer stop");
    std::fs::remove_dir_all(host_root).expect("host cleanup");
    std::fs::remove_dir_all(peer_root).expect("peer cleanup");
}
