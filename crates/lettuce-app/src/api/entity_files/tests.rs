use std::sync::Arc;

use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_jobs::SystemClock;
use lettuce_transfer::ProviderBackupSource;
use serde_json::{Value, json};

use crate::api::tests::{Harness, Reply, harness_over};

fn setup() -> (std::path::PathBuf, Harness) {
    let root = std::env::temp_dir()
        .join(format!(
            "lettuce-entity-files-{}",
            lettuce_types::JobId::new()
        ));
    std::fs::create_dir_all(&root).expect("folder");
    let backend = Arc::new(
        crate::AppBackend::open(
            root.join("state.sqlite3"),
            lettuce_types::TimestampMillis::new(1),
        )
        .expect("backend"),
    );
    let harness = harness_over(
        backend,
        Reply::Text("reply"),
        Arc::new(SystemClock),
        Some(entity_media_store(&root)),
        Some(root.clone()),
        Arc::new(crate::api::NoModels),
        Arc::new(crate::api::tests::NoImages),
    );
    (root, harness)
}

fn entity_media_store(root: &std::path::Path) -> Arc<crate::api::ApiMediaStore> {
    let path = root.join("state.sqlite3");
    let authority = lettuce_platform::FilesystemAuthority::new(
        &lettuce_platform::DirectorySnapshot::new(root).expect("snapshot"),
    )
    .expect("authority");
    Arc::new(lettuce_media::LocalMediaBlobStore::new(
        authority.managed_files(),
        authority
            .read_capability(lettuce_platform::ManagedRoot::MediaBlobs)
            .expect("read"),
        authority
            .write_capability(lettuce_platform::ManagedRoot::MediaBlobs)
            .expect("write"),
        lettuce_database::Database::open(&path).expect("blobs"),
        lettuce_database::Database::open(&path).expect("assets"),
    ))
}

fn source(root: &std::path::Path, name: &str, content: &Value) -> dto::FileSource {
    let path = root.join(name);
    std::fs::write(&path, content.to_string()).expect("source");
    dto::FileSource {
        uri: path.display().to_string(),
    }
}

async fn preview(context: &crate::api::ApiContext, source: dto::FileSource) -> dto::ImportPreview {
    crate::api::import_preview(context, dto::ImportPreviewRequest { source })
        .await
        .expect("preview")
}

fn commit_request(
    source: dto::FileSource,
    digest: String,
    options: Value,
    key: &str,
) -> dto::ImportCommitRequest {
    serde_json::from_value(
        json!({"source": source, "digest": digest, "options": options, "client_operation_id": key}),
    )
    .expect("request")
}

#[tokio::test]
async fn previews_agree_with_inspection_and_never_carry_image_bytes() {
    let (root, harness) = setup();
    for (name, value) in [
        (
            "character.json",
            json!({"spec": "chara_card_v2", "spec_version": "2.0", "data": {"name": "Ada", "description": "Keeper", "personality": "Precise", "scenario": "Harbour", "first_mes": "Hello", "mes_example": "", "creator_notes": "", "system_prompt": "", "post_history_instructions": "", "alternate_greetings": [], "tags": [], "creator": "", "character_version": "", "extensions": {}}}),
        ),
        (
            "persona.json",
            json!({"version": 1, "persona": {"title": "Writer", "description": "Authored", "nickname": "W", "isDefault": true}, "avatarData": "data:image/png;base64,broken"}),
        ),
        (
            "world-info.json",
            json!({"name": "Harbour", "entries": [{"keys": ["ship"], "content": "A ship"}]}),
        ),
        (
            "usc.usc",
            json!({"schema": {"name": "USC", "version": "1.0"}, "kind": "lorebook", "payload": {"id": "old", "name": "Harbour", "entries": [], "createdAt": 1, "updatedAt": 2}}),
        ),
        (
            "template.json",
            json!({"version": 1, "kind": "chat_template", "template": {"name": "Starter", "messages": [{"role": "assistant", "content": "Hello"}]}}),
        ),
        (
            "model.json",
            json!({"name": "model", "providerId": "llamacpp", "displayName": "Local"}),
        ),
    ] {
        let source = source(&root, name, &value);
        let inspected = crate::api::files_inspect(
            &harness.context,
            dto::FilesInspectRequest {
                source: source.clone(),
            },
        )
        .await
        .expect("inspect");
        let preview = preview(&harness.context, source).await;
        assert_eq!(preview.kind, inspected.kind);
        assert!(!preview.digest.is_empty());
        let serialized = serde_json::to_string(&preview).expect("preview JSON");
        assert!(!serialized.contains("base64") && !serialized.contains("data:image"));
    }
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn changed_source_conflicts_before_import_and_operation_retries_replay() {
    let (root, harness) = setup();
    let content = json!({"name": "Harbour", "entries": [{"keys": ["ship"], "content": "A ship"}]});
    let source = source(&root, "book.json", &content);
    let before = preview(&harness.context, source.clone()).await;
    std::fs::write(
        &source.uri,
        json!({"name": "Changed", "entries": []}).to_string(),
    )
    .expect("change");
    let request = commit_request(source.clone(), before.digest, json!({}), "import-book");
    assert_eq!(
        crate::api::import_commit(&harness.context, request)
            .await
            .expect_err("changed file")
            .code,
        ApiErrorCode::Conflict
    );
    std::fs::write(&source.uri, content.to_string()).expect("restore fixture");
    let before = preview(&harness.context, source.clone()).await;
    let request = commit_request(source, before.digest, json!({}), "import-book");
    let result = crate::api::import_commit(&harness.context, request.clone())
        .await
        .expect("import");
    assert_eq!(
        crate::api::import_commit(&harness.context, request.clone())
            .await
            .expect("replay"),
        result
    );
    let mut changed = request;
    changed.options.character_id = Some(harness.character_id.to_string());
    assert_eq!(
        crate::api::import_commit(&harness.context, changed)
            .await
            .expect_err("changed request")
            .code,
        ApiErrorCode::Conflict
    );
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn an_unreadable_persona_avatar_is_a_named_warning_and_the_persona_imports() {
    let (root, harness) = setup();
    let source = source(
        &root,
        "persona.json",
        &json!({"version": 1, "persona": {"title": "Writer", "description": "Authored", "isDefault": true}, "avatarData": "data:image/png;base64,broken"}),
    );
    let preview = preview(&harness.context, source.clone()).await;
    assert!(
        preview
            .warnings
            .iter()
            .any(|warning| warning.field == "avatar")
    );
    let result = crate::api::import_commit(
        &harness.context,
        commit_request(source, preview.digest, json!({}), "persona"),
    )
    .await
    .expect("import without image");
    let dto::ImportCommitResult::Imported { id, warnings, .. } = result else {
        panic!("synchronous persona")
    };
    assert!(warnings.iter().any(|warning| warning.field == "avatar"));
    let persona = crate::api::persona_get(&harness.context, dto::PersonaGetRequest { persona_id: id })
        .await
        .expect("persona");
    assert_eq!(persona.draft.title, "Writer");
    assert!(persona.assets.is_empty());
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn a_template_requires_an_active_character_before_writing() {
    let (root, harness) = setup();
    let source = source(
        &root,
        "template.json",
        &json!({"version": 1, "kind": "chat_template", "template": {"name": "Starter", "messages": []}}),
    );
    let preview = preview(&harness.context, source.clone()).await;
    assert_eq!(
        crate::api::import_commit(
            &harness.context,
            commit_request(source, preview.digest, json!({}), "template")
        )
        .await
        .expect_err("target required")
        .code,
        ApiErrorCode::InvalidInput
    );
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn shutdown_refuses_synchronous_import_without_a_receipt() {
    let (root, harness) = setup();
    let source = source(
        &root,
        "book.json",
        &json!({"name": "Harbour", "entries": []}),
    );
    let preview = preview(&harness.context, source.clone()).await;
    harness.context.begin_shutdown();
    assert_eq!(
        crate::api::import_commit(
            &harness.context,
            commit_request(source, preview.digest, json!({}), "shutdown-import")
        )
        .await
        .expect_err("shutdown")
        .code,
        ApiErrorCode::Cancelled
    );
    assert!(
        harness
            .context
            .backend()
            .database()
            .lookup_api_operation("import_commit", "shutdown-import")
            .expect("lookup")
            .is_none()
    );
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn character_bad_avatar_is_named_and_bundled_name_reuse_creates_a_copy() {
    let (root, harness) = setup();
    let book = source(&root, "book.json", &json!({"name":"Harbour","entries":[]}));
    let p = preview(&harness.context, book.clone()).await;
    crate::api::import_commit(
        &harness.context,
        commit_request(book, p.digest, json!({}), "book-first"),
    )
    .await
    .expect("book");
    let card = source(
        &root,
        "card.json",
        &json!({"version":1,"character":{"name":"Ada","disableAvatarGradient":false,"rules":[],"scenes":[],"lorebooks":[{"lorebook":{"id":"bundled","name":"Harbour","createdAt":1,"updatedAt":1},"entries":[]}]},"avatarData":"data:image/png;base64,@@"}),
    );
    let p = preview(&harness.context, card.clone()).await;
    assert!(p.warnings.iter().any(|w| w.field == "avatar"));
    assert!(
        p.warnings
            .iter()
            .any(|w| w.code == dto::FileWarningCode::BundledLorebookNewCopy)
    );
    let result = crate::api::import_commit(
        &harness.context,
        commit_request(card, p.digest, json!({}), "card-copy"),
    )
    .await
    .expect("card");
    assert!(
        matches!(result,dto::ImportCommitResult::Imported{warnings,..} if warnings.iter().any(|w|w.field=="avatar"))
    );
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn group_import_with_an_unmapped_speaker_fails_typed_before_writing() {
    let (root, harness) = setup();
    let path = root.join("group.jsonl");
    std::fs::write(&path,"{\"chat_metadata\":{\"group\":true},\"character_name\":\"Group\"}\n{\"name\":\"Ada\",\"is_user\":false,\"mes\":\"One\"}\n{\"name\":\"Ben\",\"is_user\":false,\"mes\":\"Two\"}").expect("chat");
    let source = dto::FileSource {
        uri: path.display().to_string(),
    };
    let p = preview(&harness.context, source.clone()).await;
    let dto::ImportCommitResult::Job { job_id } = crate::api::import_commit(
        &harness.context,
        commit_request(
            source,
            p.digest,
            json!({"speakers":{"Ada":harness.character_id.to_string()}}),
            "unmapped",
        ),
    )
    .await
    .expect("admit") else {
        panic!("job")
    };
    let runner = crate::api::JobRunner::new(harness.context.clone(), crate::api::JobHandlers::standard());
    assert!(runner.run_once().await.expect("run"));
    runner.wait_idle().await;
    let job = crate::api::job_get(&harness.context, dto::JobGetRequest { job_id })
        .await
        .expect("job");
    assert_eq!(job.state, dto::JobStateDto::Failed);
    let graph = harness
        .context
        .backend()
        .database()
        .read_provider_backup_graph()
        .expect("graph");
    assert!(graph.conversation_history.conversations.is_empty());
    assert!(graph.authored.groups.is_empty());
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn chat_commit_needs_no_preview_in_this_process() {
    let (root, harness) = setup();
    let path = root.join("chat.jsonl");
    let raw = "{\"character_name\":\"Ada\"}\n{\"name\":\"Ada\",\"is_user\":false,\"mes\":\"Hello\"}";
    std::fs::write(&path, raw).expect("chat");
    let source = dto::FileSource {
        uri: path.display().to_string(),
    };
    let digest = blake3::hash(raw.as_bytes()).to_hex().to_string();
    let result = crate::api::import_commit(
        &harness.context,
        commit_request(
            source,
            digest,
            json!({"character_id":harness.character_id.to_string()}),
            "no-preview",
        ),
    )
    .await
    .expect("admit without a preview");
    let runner = crate::api::JobRunner::new(harness.context.clone(), crate::api::JobHandlers::standard());
    assert!(runner.run_once().await.expect("run"));
    runner.wait_idle().await;
    let dto::ImportCommitResult::Job { job_id } = result else {
        panic!("job")
    };
    let job = crate::api::job_get(&harness.context, dto::JobGetRequest { job_id })
        .await
        .expect("job");
    assert_eq!(job.state, dto::JobStateDto::Succeeded);
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn lorebook_export_replaces_a_target_only_with_a_complete_current_file() {
    let (root, harness) = setup();
    let source = source(
        &root,
        "book.json",
        &json!({"name":"Harbour","entries":[{"keys":["ship"],"content":"A ship"}]}),
    );
    let p = preview(&harness.context, source.clone()).await;
    let dto::ImportCommitResult::Imported { id, .. } = crate::api::import_commit(
        &harness.context,
        commit_request(source, p.digest, json!({}), "export-book"),
    )
    .await
    .expect("import") else {
        panic!("book")
    };
    let target = root.join("out.usc");
    std::fs::write(&target, "previous").expect("target");
    let request = dto::LorebookExportRequest {
        id,
        format: dto::LorebookExportFormat::Uec,
        target: Some(dto::FileTarget {
            uri: target.display().to_string(),
        }),
    };
    crate::api::lorebook_export(&harness.context, request.clone())
        .await
        .expect("export");
    let content: Value =
        serde_json::from_slice(&std::fs::read(&target).expect("target")).expect("complete JSON");
    assert_eq!(content["entity"]["name"], "Harbour");
    crate::api::lorebook_export(&harness.context, request)
        .await
        .expect("repeat current export");
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn cancelled_chat_import_job_writes_no_conversation() {
    let (root, harness) = setup();
    let path = root.join("chat.jsonl");
    std::fs::write(&path,"{\"character_name\":\"Ada\"}\n{\"name\":\"Ada\",\"is_user\":false,\"mes\":\"Whole message\"}").expect("chat");
    let source = dto::FileSource {
        uri: path.display().to_string(),
    };
    let p = preview(&harness.context, source.clone()).await;
    let request = commit_request(
        source,
        p.digest,
        json!({"character_id":harness.character_id.to_string()}),
        "cancel-chat",
    );
    let dto::ImportCommitResult::Job { job_id } =
        crate::api::import_commit(&harness.context, request.clone())
            .await
            .expect("admit")
    else {
        panic!("job")
    };
    crate::api::job_cancel(
        &harness.context,
        dto::JobCancelRequest {
            job_id: job_id.clone(),
        },
    )
    .await
    .expect("cancel");
    let job = crate::api::job_get(
        &harness.context,
        dto::JobGetRequest {
            job_id: job_id.clone(),
        },
    )
    .await
    .expect("job");
    assert_eq!(job.state, dto::JobStateDto::Cancelled);
    assert_eq!(
        crate::api::import_commit(&harness.context, request)
            .await
            .expect("replay admission"),
        dto::ImportCommitResult::Job { job_id }
    );
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn chat_apply_rolls_back_all_rows_and_two_workers_import_once() {
    use lettuce_database::ApiOperationError;
    let (root, harness) = setup();
    let raw = "{\"character_name\":\"Ada\"}\n{\"name\":\"Ada\",\"is_user\":false,\"mes\":\"Whole message\"}";
    let database = harness.context.backend().database();
    let before = database.read_provider_backup_graph().expect("before");
    let prepared = harness
        .context
        .backend()
        .chat_files()
        .prepare_direct(
            lettuce_transfer::parse_chat_jsonl(raw, 1).expect("parse transcript"),
            Some("chat"),
            Some(harness.character_id),
            harness.context.now(),
        )
        .expect("prepare");
    let result: Result<(), ApiOperationError> = database.commit_api_operation("import_chat_apply", "rollback", "rollback", harness.context.now(), |scope| {
            scope
                .import_chat(
                    prepared.group,
                    prepared.record,
                    prepared.companion,
                    harness.context.now(),
                )
                .expect("chat write");
            Err(ApiOperationError::Storage { reason: "simulated failure" })
        },
    );
    assert_eq!(result, Err(ApiOperationError::Storage { reason: "simulated failure" }));
    assert_eq!(
        database.read_provider_backup_graph().expect("after"),
        before
    );
    let path = root.join("chat.jsonl");
    std::fs::write(&path, raw).expect("chat");
    let source = dto::FileSource {
        uri: path.display().to_string(),
    };
    let p = preview(&harness.context, source.clone()).await;
    let request = commit_request(
        source,
        p.digest,
        json!({"character_id":harness.character_id.to_string()}),
        "once-chat",
    );
    let dto::ImportCommitResult::Job { job_id } =
        crate::api::import_commit(&harness.context, request.clone())
            .await
            .expect("admit")
    else {
        panic!("job")
    };
    let first = crate::api::JobRunner::new(harness.context.clone(), crate::api::JobHandlers::standard());
    let second = crate::api::JobRunner::new(harness.context.clone(), crate::api::JobHandlers::standard());
    let (a, b) = tokio::join!(first.run_once(), second.run_once());
    assert!(a.expect("first") || b.expect("second"));
    first.wait_idle().await;
    second.wait_idle().await;
    let job = crate::api::job_get(
        &harness.context,
        dto::JobGetRequest {
            job_id: job_id.clone(),
        },
    )
    .await
    .expect("job");
    assert_eq!(job.state, dto::JobStateDto::Succeeded);
    assert!(database.lookup_api_operation("import_chat_apply", "once-chat").expect("receipt lookup").is_some());
    assert!(matches!(
        job.result,
        Some(dto::JobResultDto::Conversation { .. })
    ));
    assert_eq!(
        crate::api::import_commit(&harness.context, request)
            .await
            .expect("replay"),
        dto::ImportCommitResult::Job { job_id }
    );
    assert_eq!(
        database
            .read_provider_backup_graph()
            .expect("graph")
            .conversation_history
            .conversations
            .len(),
        1
    );
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn shutdown_after_a_chat_claim_leaves_nothing_and_restart_retries_once() {
    use crate::api::JobHandler;
    use lettuce_jobs::{JobStore, WorkerId};
    let root = std::env::temp_dir()
        .join(format!("chat-restart-{}", lettuce_types::JobId::new()));
    std::fs::create_dir_all(&root).expect("folder");
    let path = root.join("state.sqlite3");
    let backend = Arc::new(
        crate::AppBackend::open(&path, lettuce_types::TimestampMillis::new(1)).expect("backend"),
    );
    let harness = crate::api::tests::harness_over(
        backend,
        Reply::Text("reply"),
        Arc::new(SystemClock),
        None,
        Some(root.clone()),
        Arc::new(crate::api::NoModels),
        Arc::new(crate::api::tests::NoImages),
    );
    let raw = "{\"character_name\":\"Ada\"}\n{\"name\":\"Ada\",\"is_user\":false,\"mes\":\"Whole message\"}";
    let input = root.join("chat.jsonl");
    std::fs::write(&input, raw).expect("chat");
    let source = dto::FileSource {
        uri: input.display().to_string(),
    };
    let p = preview(&harness.context, source.clone()).await;
    let request = commit_request(
        source,
        p.digest,
        json!({"character_id":harness.character_id.to_string()}),
        "restart-chat",
    );
    let dto::ImportCommitResult::Job { job_id } =
        crate::api::import_commit(&harness.context, request.clone())
            .await
            .expect("admit")
    else {
        panic!("job")
    };
    let id = job_id.parse().expect("id");
    let snapshot = harness
        .context
        .backend()
        .database()
        .get(id)
        .expect("job")
        .expect("snapshot");
    let claimed = super::import_jobs::ImportHandler
        .claim(&harness.context, &snapshot, WorkerId::new())
        .await
        .expect("claim")
        .expect("claimed");
    harness.context.begin_shutdown();
    claimed
        .run(
            harness.context.clone(),
            Arc::new(crate::api::backup::tests::Progress),
        )
        .await
        .expect("shutdown");
    assert!(
        harness
            .context
            .backend()
            .database()
            .read_provider_backup_graph()
            .expect("before recovery")
            .conversation_history
            .conversations
            .is_empty()
    );
    drop(harness);
    let backend = Arc::new(
        crate::AppBackend::open(&path, lettuce_types::TimestampMillis::new(2))
            .expect("restart backend"),
    );
    let harness = crate::api::tests::harness_over(
        backend,
        Reply::Text("reply"),
        Arc::new(SystemClock),
        None,
        Some(root.clone()),
        Arc::new(crate::api::NoModels),
        Arc::new(crate::api::tests::NoImages),
    );
    harness
        .context
        .recover_after_restart()
        .expect("recover orphan");
    let runner = crate::api::JobRunner::new(harness.context.clone(), crate::api::JobHandlers::standard());
    assert!(runner.run_once().await.expect("restart"));
    runner.wait_idle().await;
    assert_eq!(
        crate::api::job_get(
            &harness.context,
            dto::JobGetRequest {
                job_id: job_id.clone()
            }
        )
        .await
        .expect("job")
        .state,
        dto::JobStateDto::Succeeded
    );
    assert_eq!(
        crate::api::import_commit(&harness.context, request)
            .await
            .expect("replay"),
        dto::ImportCommitResult::Job { job_id }
    );
    assert_eq!(
        harness
            .context
            .backend()
            .database()
            .read_provider_backup_graph()
            .expect("after recovery")
            .conversation_history
            .conversations
            .len(),
        1
    );
    std::fs::remove_dir_all(root).expect("folder");
}

async fn avatar_server(
    stall: bool,
) -> (
    String,
    Arc<tokio::sync::Notify>,
    tokio::task::JoinHandle<()>,
) {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    use tokio_rustls::{
        TlsAcceptor,
        rustls::{
            ServerConfig,
            pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
        },
    };
    let cert = CertificateDer::from_pem_slice(include_bytes!(
        "../../../tests/fixtures/provider-test-server.pem"
    ))
    .expect("cert");
    let key =
        PrivateKeyDer::from_pem_slice(include_bytes!("../../../tests/fixtures/provider-test-key.pem"))
            .expect("key");
    let acceptor = TlsAcceptor::from(Arc::new(
        ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)
            .expect("tls"),
    ));
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let url = format!(
        "https://localhost:{}/avatar.png",
        listener.local_addr().expect("address").port()
    );
    let started = Arc::new(tokio::sync::Notify::new());
    let signal = started.clone();
    let task = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.expect("accept");
        let mut socket = acceptor.accept(socket).await.expect("tls");
        let mut request = [0; 4096];
        let amount = socket.read(&mut request).await.expect("request");
        assert!(amount > 0);
        let bytes = crate::api::tests::png_bytes();
        socket
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    bytes.len()
                )
                .as_bytes(),
            )
            .await
            .expect("head");
        if stall {
            socket.write_all(&bytes[..8]).await.expect("prefix");
            signal.notify_one();
            std::future::pending::<()>().await;
        }
        socket.write_all(&bytes).await.expect("image");
    });
    (url, started, task)
}

fn trust_avatar_server(context: &crate::api::ApiContext) {
    use lettuce_settings::DeviceSettingsStore;
    context
        .backend()
        .database()
        .update_device_settings(&|settings| {
            settings
                .trusted_certificates
                .push(lettuce_settings::TrustedCertificate {
                    id: uuid::Uuid::new_v4(),
                    name: "test.pem".into(),
                    imported_at: 1,
                    pem: include_str!("../../../tests/fixtures/provider-test-cert.pem").into(),
                })
        })
        .expect("trust");
}

#[tokio::test]
async fn remote_avatar_downloads_when_enabled_and_warns_when_disabled_or_failing() {
    use lettuce_settings::GlobalSettingsStore;
    let (root, harness) = setup();
    trust_avatar_server(&harness.context);
    let (url, _, server) = avatar_server(false).await;
    for (index, enabled, expected) in [
        (0, true, None),
        (
            1,
            false,
            Some(dto::FileWarningCode::RemoteAvatarNotDownloaded),
        ),
        (2, true, Some(dto::FileWarningCode::ImageDownloadFailed)),
    ] {
        let database = harness.context.backend().database();
        let settings = database.load().expect("settings");
        let mut draft = settings.settings;
        draft.auto_download_character_card_avatars = enabled;
        database
            .save(draft, settings.default_model_profile_id, settings.revision)
            .expect("setting");
        let avatar = if index == 2 {
            "https://localhost:1/unavailable"
        } else {
            &url
        };
        let source = source(
            &root,
            &format!("persona-{index}.json"),
            &json!({"version":1,"persona":{"title":"Writer","description":"Authored"},"avatarData":avatar}),
        );
        let p = preview(&harness.context, source.clone()).await;
        let dto::ImportCommitResult::Imported { id, warnings, .. } = crate::api::import_commit(
            &harness.context,
            commit_request(source, p.digest, json!({}), &format!("remote-{index}")),
        )
        .await
        .expect("import") else {
            panic!("persona")
        };
        if let Some(code) = expected {
            assert!(
                warnings
                    .iter()
                    .any(|warning| warning.code == code && warning.field == "avatar")
            );
        } else {
            assert!(warnings.is_empty(), "{warnings:?}");
            assert!(
                !crate::api::persona_get(&harness.context, dto::PersonaGetRequest { persona_id: id })
                    .await
                    .expect("persona")
                    .assets
                    .is_empty()
            );
        }
    }
    server.abort();
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn cancelling_a_remote_avatar_mid_stream_leaves_no_media_object() {
    let (root, harness) = setup();
    trust_avatar_server(&harness.context);
    let (url, started, server) = avatar_server(true).await;
    let source = source(
        &root,
        "persona.json",
        &json!({"version":1,"persona":{"title":"Writer","description":"Authored"},"avatarData":url}),
    );
    let p = preview(&harness.context, source.clone()).await;
    let request = commit_request(source, p.digest, json!({}), "cancel-avatar");
    let context = harness.context.clone();
    let task = tokio::spawn(async move { crate::api::import_commit(&context, request).await });
    tokio::time::timeout(std::time::Duration::from_secs(10), started.notified())
        .await
        .expect("download started");
    harness.context.begin_shutdown();
    assert_eq!(
        task.await.expect("task").expect_err("cancel").code,
        ApiErrorCode::Cancelled
    );
    let objects = root.join("platform-v2/media-blobs/objects");
    assert!(
        !objects.exists()
            || std::fs::read_dir(objects)
                .expect("objects")
                .next()
                .is_none()
    );
    server.abort();
    std::fs::remove_dir_all(root).expect("folder");
}

struct RefuseExport;
impl crate::api::FileAccess for RefuseExport {
    fn describe(&self, uri: &str) -> Result<crate::api::FileDescription, crate::api::FileAccessError> {
        crate::api::tests::StdFiles.describe(uri)
    }
    fn open(&self, uri: &str) -> Result<Box<dyn crate::api::FileReader>, crate::api::FileAccessError> {
        crate::api::tests::StdFiles.open(uri)
    }
    fn create(&self, uri: &str) -> Result<Box<dyn std::io::Write + Send>, crate::api::FileAccessError> {
        crate::api::tests::StdFiles.create(uri)
    }
    fn remove(&self, uri: &str) -> Result<(), crate::api::FileAccessError> {
        crate::api::tests::StdFiles.remove(uri)
    }
    fn backup_temporary_location(
        &self,
        target: Option<&str>,
        name: &str,
    ) -> Result<String, crate::api::FileAccessError> {
        crate::api::tests::StdFiles.backup_temporary_location(target, name)
    }
    fn create_backup_temporary(
        &self,
        intent: &str,
        protected: crate::api::ExportProtection<'_>,
    ) -> Result<(String, Box<dyn std::io::Write + Send>), crate::api::FileAccessError> {
        let (path, _) = crate::api::tests::StdFiles.create_backup_temporary(intent, protected)?;
        Ok((path, Box::new(RefuseWrite)))
    }
}
struct RefuseWrite;
impl std::io::Write for RefuseWrite {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("injected write failure"))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn failing_exports(harness: &Harness, root: &std::path::Path) -> crate::api::ApiContext {
    export_context(harness, root, Arc::new(RefuseExport))
}
fn export_context(
    harness: &Harness,
    root: &std::path::Path,
    files: Arc<dyn crate::api::FileAccess>,
) -> crate::api::ApiContext {
    crate::api::ApiContext::new(crate::api::ApiContextParts {
        backend: harness.context.shared_backend(),
        secret_store: harness.context.secret_store().clone(),
        inference: harness.provider.clone(),
        image_provider: Arc::new(crate::api::tests::NoImages),
        models: Arc::new(crate::api::NoModels),
        speech: Arc::new(crate::api::NoSpeech),
        media: Some(entity_media_store(root)),
        events: harness.events.clone(),
        clock: Arc::new(lettuce_jobs::FakeClock::new(
            lettuce_types::TimestampMillis::new(1),
        )),
        files,
        app_folder: Some(root.into()),
        resource_dir: None,
        database_files: None,
        asset_url_base: "test-asset://host".into(),
    })
}

#[tokio::test]
async fn every_export_is_complete_and_failed_writes_preserve_the_old_target() {
    use lettuce_models::ModelCatalog;
    let (root, harness) = setup();
    let character = harness.character_id.to_string();
    let import = async |name: &str, value: Value, options: Value, key: &str| {
        let source = source(&root, name, &value);
        let p = preview(&harness.context, source.clone()).await;
        let dto::ImportCommitResult::Imported { id, .. } = crate::api::import_commit(
            &harness.context,
            commit_request(source, p.digest, options, key),
        )
        .await
        .expect("import") else {
            panic!("entity")
        };
        id
    };
    let persona = import(
        "persona.json",
        json!({"version":1,"persona":{"title":"Writer","description":"Authored"}}),
        json!({}),
        "export-persona",
    )
    .await;
    let book = import(
        "book.json",
        json!({"name":"Harbour","entries":[]}),
        json!({}),
        "export-lorebook",
    )
    .await;
    let prompt=import("prompt.usc",json!({"schema":{"name":"USC","version":"1.0"},"kind":"system_prompt_template","payload":{"name":"Rules","promptType":"directChat","entries":[{"name":"Rules","role":"system","content":"Authored","enabled":true}],"condensePromptEntries":false}}),json!({}),"export-prompt").await;
    let template=import("template.json",json!({"version":1,"kind":"chat_template","template":{"name":"Greeting","messages":[{"role":"assistant","content":"Hello"}]}}),json!({"character_id":character}),"export-template").await;
    let model = harness
        .context
        .backend()
        .database()
        .model_profiles()
        .expect("models")[0]
        .id
        .to_string();
    let prepared = harness.context.backend().chat_files().prepare_direct(
        lettuce_transfer::parse_chat_jsonl("{\"character_name\":\"Ada\"}\n{\"name\":\"Ada\",\"mes\":\"Hello\"}", 1).expect("parse transcript"),
        Some("chat"), Some(harness.character_id), harness.context.now(),
    ).expect("chat plan");
    let chat = prepared.record.history.aggregate.conversation.id.to_string();
    harness.context.backend().database().commit_api_operation::<(), lettuce_database::ApiOperationError>("import_chat_apply", &chat, &chat, harness.context.now(), |scope| {
        scope.import_chat(prepared.group, prepared.record, prepared.companion, harness.context.now())
            .map_err(|_| lettuce_database::ApiOperationError::Storage { reason: "simulated failure" })
    }).expect("chat");
    let fail = failing_exports(&harness, &root);
    let android = export_context(&harness, &root, Arc::new(AndroidDownloads(root.clone())));
    let success = export_context(&harness, &root, Arc::new(crate::api::tests::StdFiles));
    for kind in 0..7 {
        let target = root.join(format!("export-{kind}"));
        let export = async |context: &crate::api::ApiContext, downloads: bool| {
            let target = if downloads {
                None
            } else {
                Some(dto::FileTarget {
                    uri: target.display().to_string(),
                })
            };
            match kind {
                0 => {
                    crate::api::character_export(
                        context,
                        dto::CharacterExportRequest {
                            include_learned: false,
                            id: character.clone(),
                            format: dto::CharacterExportFormat::Uec,
                            target,
                        },
                    )
                    .await
                }
                1 => {
                    crate::api::persona_export(
                        context,
                        dto::EntityExportRequest {
                            id: persona.clone(),
                            target,
                        },
                    )
                    .await
                }
                2 => {
                    crate::api::lorebook_export(
                        context,
                        dto::LorebookExportRequest {
                            id: book.clone(),
                            format: dto::LorebookExportFormat::Uec,
                            target,
                        },
                    )
                    .await
                }
                3 => {
                    crate::api::prompt_export(
                        context,
                        dto::EntityExportRequest {
                            id: prompt.clone(),
                            target,
                        },
                    )
                    .await
                }
                4 => {
                    crate::api::chat_template_export(
                        context,
                        dto::ChatTemplateExportRequest {
                            character_id: character.clone(),
                            starter_id: template.clone(),
                            target,
                        },
                    )
                    .await
                }
                5 => {
                    crate::api::model_export(
                        context,
                        dto::ModelExportRequest {
                            id: model.clone(),
                            target,
                        },
                    )
                    .await
                }
                _ => {
                    crate::api::chat_export(
                        context,
                        dto::ChatExportRequest {
                            conversation_id: chat.clone(),
                            target,
                        },
                    )
                    .await
                }
            }
        };
        std::fs::write(&target, "old content").expect("old");
        assert!(export(&fail, false).await.is_err());
        assert_eq!(
            std::fs::read_to_string(&target).expect("target"),
            "old content"
        );
        std::fs::remove_file(&target).expect("remove old");
        assert!(export(&fail, false).await.is_err());
        assert!(!target.exists());
        export(&success, false).await.expect("complete export");
        let content = std::fs::read_to_string(&target).expect("export");
        if kind == 6 {
            assert!(lettuce_transfer::parse_chat_jsonl(&content, 1).is_ok());
        } else {
            assert!(serde_json::from_str::<Value>(&content).is_ok());
        }
        let published = export(&android, true).await.expect("Downloads export");
        assert_eq!(
            std::fs::read_to_string(&published.uri).expect("published"),
            content
        );
        assert_eq!(
            std::fs::read_to_string(root.join(&published.suggested_name)).expect("existing"),
            "existing download"
        );
        assert!(!std::fs::read_dir(&root).expect("folder").any(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .ends_with(".partial")
        }));
    }
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn every_synchronous_import_replays_concurrent_retries_without_the_source() {
    use lettuce_settings::GlobalSettingsStore;
    let (root, harness) = setup();
    let database = harness.context.backend().database();
    let stored = database.load().expect("settings");
    database
        .save(stored.settings, None, stored.revision)
        .expect("clear default");
    for (index, (name, value, options)) in [
        ("card.json", json!({"spec":"chara_card_v2","spec_version":"2.0","data":{"name":"Imported Ada","description":"Keeper","personality":"","scenario":"","first_mes":"Hello","mes_example":"","creator_notes":"","system_prompt":"","post_history_instructions":"","alternate_greetings":[],"tags":[],"creator":"","character_version":"","extensions":{}}}), json!({})),
        ("persona.json", json!({"version":1,"persona":{"title":"Writer","description":"Authored","isDefault":true}}), json!({})),
        ("book.json", json!({"name":"Harbour","entries":[{"keys":["ship"],"content":"A ship"}]}), json!({})),
        ("prompt.usc", json!({"schema":{"name":"USC","version":"1.0"},"kind":"system_prompt_template","payload":{"name":"Rules","promptType":"directChat","entries":[{"name":"Rules","role":"system","content":"Authored","enabled":true}],"condensePromptEntries":false}}), json!({})),
        ("template.json", json!({"version":1,"kind":"chat_template","template":{"name":"Greeting","messages":[{"role":"assistant","content":"Hello"}]}}), json!({"character_id":harness.character_id.to_string()})),
        ("model.json", json!({"name":"imported-model","providerId":"ollama","displayName":"Imported model"}), json!({})),
    ].into_iter().enumerate() {
        let source = source(&root, name, &value);
        let p = preview(&harness.context, source.clone()).await;
        let request = commit_request(source.clone(), p.digest, options, &format!("concurrent-{index}"));
        let (a,b) = tokio::join!(crate::api::import_commit(&harness.context,request.clone()), crate::api::import_commit(&harness.context,request.clone()));
        let result = a.expect("first import");
        assert_eq!(b.expect("concurrent replay"),result);
        let graph = database.read_provider_backup_graph().expect("graph");
        std::fs::remove_file(&source.uri).expect("remove source");
        assert_eq!(crate::api::import_commit(&harness.context,request.clone()).await.expect("replay without source"),result);
        assert_eq!(database.read_provider_backup_graph().expect("after"),graph);
        let mut changed = request;
        changed.digest.push('0');
        assert_eq!(crate::api::import_commit(&harness.context,changed).await.expect_err("conflict").code,ApiErrorCode::Conflict);
        if index == 5 {
            let dto::ImportCommitResult::Imported{id,..} = result else {panic!("model")};
            assert_eq!(database.load().expect("settings").default_model_profile_id.expect("default").to_string(),id);
        }
    }
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn group_and_whole_transcript_roll_back_together_then_import_once() {
    use lettuce_database::ApiOperationError;
    let (root, harness) = setup();
    let database = harness.context.backend().database();
    let ben = crate::api::tests::create_character(database, "Ben", Default::default());
    let raw = "{\"character_name\":\"Ada and Ben\"}\n{\"name\":\"Ada\",\"is_user\":false,\"mes\":\"Ada message\"}\n{\"name\":\"Ben\",\"is_user\":false,\"mes\":\"Ben message\"}";
    let mapping = std::collections::BTreeMap::from([
        ("Ada".into(), harness.character_id),
        ("Ben".into(), ben),
    ]);
    let prepared = harness
        .context
        .backend()
        .chat_files()
        .prepare_group(lettuce_transfer::parse_chat_jsonl(raw, 1).expect("parse transcript"), &mapping, harness.context.now())
        .expect("prepare group");
    assert!(prepared.group.is_some());
    let before = database.read_provider_backup_graph().expect("before");
    let result: Result<String, ApiOperationError> = database.commit_api_operation(
        "import_chat_apply",
        "group-crash",
        "digest",
        harness.context.now(),
        |scope| {
            scope
                .import_chat(
                    prepared.group,
                    prepared.record,
                    prepared.companion,
                    harness.context.now(),
                )
                .expect("group and transcript");
            Err(ApiOperationError::Storage { reason: "simulated failure" })
        },
    );
    assert_eq!(result, Err(ApiOperationError::Storage { reason: "simulated failure" }));
    assert_eq!(
        database.read_provider_backup_graph().expect("after"),
        before
    );
    let path = root.join("group.jsonl");
    std::fs::write(&path, raw).expect("source");
    let source = dto::FileSource {
        uri: path.display().to_string(),
    };
    let p = preview(&harness.context, source.clone()).await;
    let request = commit_request(
        source,
        p.digest,
        json!({"speakers":{"Ada":harness.character_id.to_string(),"Ben":ben.to_string()}}),
        "group-once",
    );
    let result = crate::api::import_commit(&harness.context, request.clone())
        .await
        .expect("admit");
    let runner = crate::api::JobRunner::new(harness.context.clone(), crate::api::JobHandlers::standard());
    assert!(runner.run_once().await.expect("run"));
    runner.wait_idle().await;
    let dto::ImportCommitResult::Job { job_id } = result.clone() else {
        panic!("job")
    };
    let job = crate::api::job_get(&harness.context, dto::JobGetRequest { job_id })
        .await
        .expect("job");
    assert_eq!(job.state, dto::JobStateDto::Succeeded);
    assert_eq!(
        crate::api::import_commit(&harness.context, request)
            .await
            .expect("replay"),
        result
    );
    let graph = database.read_provider_backup_graph().expect("graph");
    assert_eq!(graph.conversation_history.conversations.len(), 1);
    assert_eq!(graph.authored.groups.len(), 1);
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn late_failures_roll_back_media_templates_documents_and_model_defaults() {
    use lettuce_database::ApiOperationError;
    use lettuce_settings::GlobalSettingsStore;
    let (root, harness) = setup();
    let database = harness.context.backend().database();
    let stored = database.load().expect("settings");
    database
        .save(stored.settings, None, stored.revision)
        .expect("clear default");
    let before = database.read_provider_backup_graph().expect("before");
    let media = harness.context.media().expect("media");
    let upload = media
        .ingest_with_commit(
            crate::api::tests::png_bytes().as_slice(),
            lettuce_media::IngestRequest::new(
                lettuce_media::AssetKind::AvatarOriginal,
                lettuce_media::AssetOrigin::Import,
                lettuce_media::RetentionClass::Persistent,
                Default::default(),
            ),
            |upload| upload,
        )
        .expect("stage object");
    let avatar = upload.asset.id;
    let package = harness.context.backend().persona_files().read("{\"version\":1,\"persona\":{\"title\":\"Writer\",\"description\":\"Authored\",\"isDefault\":true}}",harness.context.now()).expect("persona");
    let (mut persona, _) = harness
        .context
        .backend()
        .persona_files()
        .prepare(&package, harness.context.now())
        .expect("prepare");
    persona
        .persona
        .media
        .links
        .push(lettuce_characters::PersonaMediaLink {
            asset_id: avatar,
            slot: lettuce_characters::PersonaMediaSlot::Avatar,
            ordinal: 0,
        });
    let (revision,starter) = harness.context.backend().chat_template_files().prepare(harness.character_id,"{\"version\":1,\"kind\":\"chat_template\",\"template\":{\"name\":\"Greeting\",\"messages\":[{\"role\":\"assistant\",\"content\":\"Hello\"}]}}",harness.context.now()).expect("starter");
    let model = harness
        .context
        .backend()
        .model_files()
        .prepare(
            lettuce_transfer::parse_model_import(
                "{\"name\":\"imported\",\"providerId\":\"ollama\",\"displayName\":\"Imported\"}",
            )
            .expect("model"),
            harness.context.now(),
        )
        .expect("profile");
    for case in 0..4 {
        let result: Result<String,ApiOperationError> = database.commit_api_operation("import_commit",&format!("late-{case}"),"digest",harness.context.now(),|scope| {
            match case {
                0 => {scope.install_library_upload(lettuce_media::IngestedMedia{blob:upload.blob.clone(),asset:upload.asset.clone()}).expect("catalog");scope.import_persona_file(&persona).expect("persona");},
                1 => {
                    scope.import_starter(harness.character_id,revision,starter.clone(),harness.context.now()).expect("append");
                },
                2 => {
                    let (metadata,entries) = crate::files::lorebook_files::prepare_import("{\"name\":\"Harbour\",\"entries\":[{\"keys\":[\"ship\"],\"content\":\"A ship\"}]}").expect("book");
                    scope.create_lorebook(metadata,entries,harness.context.now()).expect("book write");
                    scope.create_prompt(lettuce_context::PromptMetadataDraft{name:"Rules".into(),purpose:lettuce_context::PromptPurpose::DirectChat,condense:false,behavior_version:lettuce_context::PromptBehaviorVersion::LegacyV1},vec![],harness.context.now()).expect("prompt");
                },
                _ => {
                    scope.save_model_profile(model.profile.clone(),None,true).expect("model and default");
                },
            }
            Err(ApiOperationError::Storage { reason: "simulated failure" })
        });
        assert_eq!(result, Err(ApiOperationError::Storage { reason: "simulated failure" }));
        assert_eq!(
            database.read_provider_backup_graph().expect("after"),
            before
        );
    }
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn a_missing_required_export_avatar_fails_typed_and_preserves_the_target() {
    use base64::Engine;
    let (root, harness) = setup();
    let image = base64::engine::general_purpose::STANDARD.encode(crate::api::tests::png_bytes());
    let source = source(
        &root,
        "persona.json",
        &json!({"version":1,"persona":{"title":"Writer","description":"Authored"},"avatarData":image}),
    );
    let p = preview(&harness.context, source.clone()).await;
    let dto::ImportCommitResult::Imported { id, warnings, .. } = crate::api::import_commit(
        &harness.context,
        commit_request(source, p.digest, json!({}), "missing-avatar"),
    )
    .await
    .expect("import") else {
        panic!("persona")
    };
    assert!(warnings.is_empty());
    let objects = root.join("platform-v2/media-blobs/objects");
    fn remove_objects(path: &std::path::Path) {
        for entry in std::fs::read_dir(path).expect("objects") {
            let path = entry.expect("object").path();
            if path.is_dir() {
                remove_objects(&path);
            } else {
                std::fs::remove_file(path).expect("remove object");
            }
        }
    }
    remove_objects(&objects);
    let target = root.join("persona.uec");
    std::fs::write(&target, b"existing complete file").expect("target");
    let result = crate::api::persona_export(
        &harness.context,
        dto::EntityExportRequest {
            id,
            target: Some(dto::FileTarget {
                uri: target.display().to_string(),
            }),
        },
    )
    .await
    .expect_err("required avatar");
    assert_eq!(result.code, ApiErrorCode::Unavailable);
    assert_eq!(
        std::fs::read(target).expect("target"),
        b"existing complete file"
    );
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn active_database_maintenance_refuses_import_and_export_before_writing() {
    let (root, harness) = setup();
    let source = source(&root, "book.json", &json!({"name":"Harbour","entries":[]}));
    let p = preview(&harness.context, source.clone()).await;
    let dto::ImportCommitResult::Imported { id, .. } = crate::api::import_commit(
        &harness.context,
        commit_request(
            source.clone(),
            p.digest.clone(),
            json!({}),
            "maintenance-book",
        ),
    )
    .await
    .expect("book") else {
        panic!("book")
    };
    let cancellation = lettuce_jobs::handle::CancellationToken::new();
    let _lease = harness
        .context
        .maintenance()
        .maintenance(&cancellation)
        .await
        .expect("maintenance");
    let import = crate::api::import_commit(
        &harness.context,
        commit_request(source, p.digest, json!({}), "during-maintenance"),
    )
    .await;
    let target = root.join("during-maintenance.usc");
    let export = crate::api::lorebook_export(
        &harness.context,
        dto::LorebookExportRequest {
            id,
            format: dto::LorebookExportFormat::Uec,
            target: Some(dto::FileTarget {
                uri: target.display().to_string(),
            }),
        },
    )
    .await;
    assert_eq!(import.expect_err("import refused").code, ApiErrorCode::Busy);
    assert_eq!(export.expect_err("export refused").code, ApiErrorCode::Busy);
    assert!(!target.exists());
    assert!(
        harness
            .context
            .backend()
            .database()
            .lookup_api_operation("import_commit", "during-maintenance")
            .expect("receipt")
            .is_none()
    );
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn claimed_chat_cancellation_and_changed_source_leave_no_transcript() {
    use crate::api::JobHandler;
    use lettuce_jobs::{JobStore, WorkerId};
    let (root, harness) = setup();
    for changed in [false, true] {
        let path = root.join(format!("claimed-{changed}.jsonl"));
        std::fs::write(&path,"{\"character_name\":\"Ada\"}\n{\"name\":\"Ada\",\"is_user\":false,\"mes\":\"Whole message\"}").expect("source");
        let source = dto::FileSource {
            uri: path.display().to_string(),
        };
        let p = preview(&harness.context, source.clone()).await;
        let request = commit_request(
            source,
            p.digest,
            json!({"character_id":harness.character_id.to_string()}),
            &format!("claimed-{changed}"),
        );
        let dto::ImportCommitResult::Job { job_id } =
            crate::api::import_commit(&harness.context, request.clone())
                .await
                .expect("admit")
        else {
            panic!("job")
        };
        let snapshot = harness
            .context
            .backend()
            .database()
            .get(job_id.parse().expect("id"))
            .expect("job")
            .expect("snapshot");
        let claimed = super::import_jobs::ImportHandler
            .claim(&harness.context, &snapshot, WorkerId::new())
            .await
            .expect("claim")
            .expect("claimed");
        if changed {
            std::fs::write(&path, "{\"name\":\"Ada\",\"mes\":\"Changed\"}").expect("change");
        } else {
            crate::api::job_cancel(
                &harness.context,
                dto::JobCancelRequest {
                    job_id: job_id.clone(),
                },
            )
            .await
            .expect("cancel");
        }
        claimed
            .run(
                harness.context.clone(),
                Arc::new(crate::api::backup::tests::Progress),
            )
            .await
            .expect("settle");
        let job = crate::api::job_get(
            &harness.context,
            dto::JobGetRequest {
                job_id: job_id.clone(),
            },
        )
        .await
        .expect("job");
        if changed {
            assert_eq!(job.state, dto::JobStateDto::Failed);
            assert_eq!(
                job.failure
                    .expect("failure")
                    .entity_import
                    .expect("typed import failure")
                    .code,
                ApiErrorCode::Conflict
            );
        } else {
            assert_eq!(job.state, dto::JobStateDto::Cancelled);
        }
        assert!(
            harness
                .context
                .backend()
                .database()
                .read_provider_backup_graph()
                .expect("graph")
                .conversation_history
                .conversations
                .is_empty()
        );
        assert_eq!(
            crate::api::import_commit(&harness.context, request)
                .await
                .expect("replay"),
            dto::ImportCommitResult::Job { job_id }
        );
    }
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn model_import_ignores_inert_legacy_prompt_fields_without_warnings() {
    let (root, harness) = setup();
    for (index,value) in [json!({"name":"imported","providerId":"ollama","displayName":"Imported","promptTemplateId":"authored-prompt","systemPrompt":"Authored instructions"}),json!({"schema":{"name":"USC","version":"1.0"},"kind":"model_profile","payload":{"name":"imported","providerId":"ollama","displayName":"Imported","systemPromptTemplate":{"id":"authored-prompt"},"systemPrompt":"Authored instructions"}})].into_iter().enumerate() {
        let source = source(&root,&format!("model-prompt-{index}.json"),&value);
        let p = preview(&harness.context,source.clone()).await;
        assert!(p.warnings.is_empty(), "{:?}", p.warnings);
        let dto::ImportCommitResult::Imported{file_kind,warnings,..} = crate::api::import_commit(&harness.context,commit_request(source,p.digest,json!({}),&format!("model-prompt-{index}"))).await.expect("inert fields accepted") else {panic!("model")};
        assert_eq!(file_kind,dto::FileKind::ModelFile);
        assert!(warnings.is_empty(),"{warnings:?}");
    }
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn an_http_avatar_without_a_declared_size_downloads_and_is_checked_as_an_image() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let (root, harness) = setup();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let address = listener.local_addr().expect("address");
    let server = tokio::spawn(async move {
        for bytes in [crate::api::tests::png_bytes(), b"this is not an image".to_vec()] {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut request = [0; 4096];
            let size = stream.read(&mut request).await.expect("request");
            assert!(size > 0);
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
                )
                .await
                .expect("head");
            stream
                .write_all(format!("{:x}\r\n", bytes.len()).as_bytes())
                .await
                .expect("chunk head");
            stream.write_all(&bytes).await.expect("body");
            stream.write_all(b"\r\n0\r\n\r\n").await.expect("end");
        }
    });
    for index in 0..2 {
        let source = source(
            &root,
            &format!("http-{index}.json"),
            &json!({"version":1,"persona":{"title":"Writer","description":"Authored"},"avatarData":format!("http://{address}/avatar")}),
        );
        let p = preview(&harness.context, source.clone()).await;
        let dto::ImportCommitResult::Imported { id, warnings, .. } = crate::api::import_commit(
            &harness.context,
            commit_request(source, p.digest, json!({}), &format!("http-{index}")),
        )
        .await
        .expect("import") else {
            panic!("persona")
        };
        if index == 0 {
            assert!(warnings.is_empty(), "{warnings:?}");
            assert!(
                !crate::api::persona_get(&harness.context, dto::PersonaGetRequest { persona_id: id })
                    .await
                    .expect("persona")
                    .assets
                    .is_empty()
            );
        } else {
            assert!(warnings.iter().any(|warning| warning.field == "avatar"
                && matches!(
                    warning.code,
                    dto::FileWarningCode::ImageDecodeFailed
                        | dto::FileWarningCode::ImageStoreFailed
                )));
        }
    }
    server.await.expect("server");
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn android_exports_accept_no_target_and_return_the_published_uri() {
    let (root, harness) = setup();
    let request: dto::CharacterExportRequest = serde_json::from_value(json!({
        "id": harness.character_id.to_string(), "format": "uec"
    }))
    .expect("Android target is optional");
    let desktop_error = crate::api::character_export(&harness.context, request.clone())
        .await
        .expect_err("desktop requires a target");
    assert_eq!(desktop_error.code, ApiErrorCode::Unsupported);
    let context = export_context(&harness, &root, Arc::new(AndroidDownloads(root.clone())));
    let result = crate::api::character_export(&context, request)
        .await
        .expect("Android export");
    let value = serde_json::to_value(&result).expect("result");
    let uri = value["uri"].as_str().expect("published URI");
    assert!(uri.ends_with(" (1).uec"));
    assert!(
        serde_json::from_str::<Value>(&std::fs::read_to_string(uri).expect("published")).is_ok()
    );
    assert_eq!(
        std::fs::read_to_string(root.join(&result.suggested_name)).expect("existing"),
        "existing download"
    );
    assert!(!std::fs::read_dir(&root).expect("folder").any(|entry| {
        entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .ends_with(".partial")
    }));
    std::fs::remove_dir_all(root).expect("folder");
}

struct AndroidDownloads(std::path::PathBuf);
impl crate::api::FileAccess for AndroidDownloads {
    fn describe(&self, uri: &str) -> Result<crate::api::FileDescription, crate::api::FileAccessError> {
        crate::api::tests::StdFiles.describe(uri)
    }
    fn open(&self, uri: &str) -> Result<Box<dyn crate::api::FileReader>, crate::api::FileAccessError> {
        crate::api::tests::StdFiles.open(uri)
    }
    fn create(&self, uri: &str) -> Result<Box<dyn std::io::Write + Send>, crate::api::FileAccessError> {
        crate::api::tests::StdFiles.create(uri)
    }
    fn remove(&self, uri: &str) -> Result<(), crate::api::FileAccessError> {
        crate::api::tests::StdFiles.remove(uri)
    }
    fn backup_temporary_location(
        &self,
        target: Option<&str>,
        name: &str,
    ) -> Result<String, crate::api::FileAccessError> {
        assert!(target.is_none());
        assert!(name.ends_with(".partial"));
        Ok(self.0.join(name).display().to_string())
    }
    fn create_backup_temporary(
        &self,
        intent: &str,
        protected: crate::api::ExportProtection<'_>,
    ) -> Result<(String, Box<dyn std::io::Write + Send>), crate::api::FileAccessError> {
        crate::api::tests::StdFiles.create_backup_temporary(intent, protected)
    }
    fn sync_backup(&self, uri: &str) -> Result<(), crate::api::FileAccessError> {
        crate::api::tests::StdFiles.sync_backup(uri)
    }
    fn publish_backup(
        &self,
        temporary: &str,
        target: Option<&str>,
        name: &str,
        protected: crate::api::ExportProtection<'_>,
    ) -> Result<String, crate::api::FileAccessError> {
        assert!(target.is_none());
        std::fs::write(self.0.join(name), "existing download").expect("existing download");
        let path = std::path::Path::new(name);
        let published = self.0.join(format!(
            "{} (1).{}",
            path.file_stem().expect("stem").to_string_lossy(),
            path.extension().expect("extension").to_string_lossy()
        ));
        crate::api::tests::StdFiles.publish_backup(
            temporary,
            Some(&published.display().to_string()),
            name,
            protected,
        )
    }
}

struct HeaderOnlyFiles(Vec<u8>);
struct HeaderOnlyReader(std::io::Cursor<Vec<u8>>);
impl std::io::Read for HeaderOnlyReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        assert!(self.0.position() < crate::api::files::file_kind::HEADER_BYTES as u64, "read past the header");
        std::io::Read::read(&mut self.0, buffer)
    }
}
impl std::io::Seek for HeaderOnlyReader {
    fn seek(&mut self, position: std::io::SeekFrom) -> std::io::Result<u64> {
        assert!(!matches!(position, std::io::SeekFrom::Start(0)), "reopened body after detection");
        std::io::Seek::seek(&mut self.0, position)
    }
}
impl crate::api::FileAccess for HeaderOnlyFiles {
    fn describe(&self, _: &str) -> Result<crate::api::FileDescription, crate::api::FileAccessError> {
        Ok(crate::api::FileDescription { name: "large".into(), size: 12 * 1024 * 1024 * 1024 })
    }
    fn open(&self, _: &str) -> Result<Box<dyn crate::api::FileReader>, crate::api::FileAccessError> {
        let mut header = self.0.clone();
        header.resize(crate::api::files::file_kind::HEADER_BYTES, 0);
        Ok(Box::new(HeaderOnlyReader(std::io::Cursor::new(header))))
    }
    fn create(&self, _: &str) -> Result<Box<dyn std::io::Write + Send>, crate::api::FileAccessError> {
        panic!("no writes")
    }
}

#[tokio::test]
async fn unsupported_entity_imports_stop_at_the_header() {
    let (root, harness) = setup();
    for magic in [b"GGUF".as_slice(), b"LETTUCE-BACKUP2\0".as_slice()] {
        let context = export_context(&harness, &root, Arc::new(HeaderOnlyFiles(magic.to_vec())));
        let source = dto::FileSource { uri: "large".into() };
        assert_eq!(crate::api::import_preview(&context, dto::ImportPreviewRequest { source: source.clone() }).await.expect_err("unsupported").code, ApiErrorCode::Unsupported);
        assert_eq!(crate::api::import_commit(&context, commit_request(source, "digest".into(), json!({}), "unsupported")).await.expect_err("unsupported").code, ApiErrorCode::Unsupported);
    }
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn character_preview_accepts_a_package_without_rules() {
    let (root, harness) = setup();
    let source = source(&root, "character.json", &json!({"version":1,"character":{"name":"Ada","scenes":[],"disableAvatarGradient":false}}));
    let p = preview(&harness.context, source).await;
    let dto::ImportPreviewContent::Character { content } = p.content else { panic!("character") };
    assert_eq!(content.name, "Ada");
    assert!(serde_json::to_value(content).expect("content").get("rules").is_none());
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn previews_omit_inert_prompts_and_fabricated_entry_metadata() {
    let (root, harness) = setup();
    let card = source(&root, "character.json", &json!({"version":1,"character":{"name":"Ada","scenes":[],"disableAvatarGradient":false}}));
    let p = preview(&harness.context, card).await;
    let serialized = serde_json::to_value(p.content).expect("content");
    assert!(serialized["content"].get("systemPrompt").is_none());
    let book = source(&root, "book.json", &json!({"name":"Harbour","entries":[{"keys":["ship"],"content":"A ship"}]}));
    let p = preview(&harness.context, book).await;
    let serialized = serde_json::to_value(p.content).expect("content");
    let entry = &serialized["entries"][0];
    for field in ["id", "lorebookId", "createdAt", "updatedAt"] {
        assert!(entry.get(field).is_none(), "{field}");
    }
    std::fs::remove_dir_all(root).expect("folder");
}

struct BoundedReads {
    bounded: Arc<std::sync::atomic::AtomicBool>,
}
struct BoundedReader {
    reader: Box<dyn crate::api::FileReader>,
    bounded: Arc<std::sync::atomic::AtomicBool>,
}
impl std::io::Read for BoundedReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.bounded.load(std::sync::atomic::Ordering::SeqCst) {
            assert!(buffer.len() <= 64 * 1024, "request attempted a whole-file buffer");
        }
        self.reader.read(buffer)
    }
}
impl std::io::Seek for BoundedReader {
    fn seek(&mut self, position: std::io::SeekFrom) -> std::io::Result<u64> {
        self.reader.seek(position)
    }
}
impl crate::api::FileAccess for BoundedReads {
    fn describe(&self, uri: &str) -> Result<crate::api::FileDescription, crate::api::FileAccessError> {
        crate::api::tests::StdFiles.describe(uri)
    }
    fn open(&self, uri: &str) -> Result<Box<dyn crate::api::FileReader>, crate::api::FileAccessError> {
        Ok(Box::new(BoundedReader { reader: crate::api::tests::StdFiles.open(uri)?, bounded: self.bounded.clone() }))
    }
    fn create(&self, _: &str) -> Result<Box<dyn std::io::Write + Send>, crate::api::FileAccessError> {
        panic!("no writes")
    }
}

#[tokio::test]
async fn large_chat_admission_streams_a_digest_in_bounded_reads() {
    let (root, harness) = setup();
    let path = root.join("large.jsonl");
    let raw = format!("{{\"character_name\":\"Ada\"}}\n{}", json!({"name":"Ada","is_user":false,"mes":"x".repeat(8 * 1024 * 1024)}));
    std::fs::write(&path, &raw).expect("large transcript");
    let bounded = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let context = export_context(&harness, &root, Arc::new(BoundedReads { bounded: bounded.clone() }));
    let source = dto::FileSource { uri: path.display().to_string() };
    let p = preview(&context, source.clone()).await;
    bounded.store(true, std::sync::atomic::Ordering::SeqCst);
    let streamed = super::read_for_commit(&context, &source).expect("streamed digest");
    assert!(streamed.bytes.is_empty());
    assert_eq!(streamed.digest, p.digest);
    let result = crate::api::import_commit(&context, commit_request(source, p.digest, json!({"character_id":harness.character_id.to_string()}), "large-chat")).await.expect("streamed admission");
    assert!(matches!(result, dto::ImportCommitResult::Job { .. }));
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn disabled_avatar_downloads_make_no_network_requests() {
    use lettuce_settings::GlobalSettingsStore;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let (root, harness) = setup();
    let database = harness.context.backend().database();
    let settings = database.load().expect("settings");
    let mut draft = settings.settings;
    draft.auto_download_character_card_avatars = false;
    database.save(draft, settings.default_model_profile_id, settings.revision).expect("disable");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("address");
    let requests = Arc::new(AtomicUsize::new(0));
    let count = requests.clone();
    let server = tokio::spawn(async move {
        loop {
            let _ = listener.accept().await.expect("accept");
            count.fetch_add(1, Ordering::SeqCst);
        }
    });
    for (index, value) in [
        json!({"version":1,"character":{"name":"Ada","scenes":[],"disableAvatarGradient":false},"avatarData":format!("http://{address}/avatar")}),
        json!({"version":1,"persona":{"title":"Writer","description":"Authored"},"avatarData":format!("http://{address}/avatar")}),
    ].into_iter().enumerate() {
        let source = source(&root, &format!("disabled-{index}.json"), &value);
        let p = preview(&harness.context, source.clone()).await;
        let dto::ImportCommitResult::Imported { warnings, .. } = crate::api::import_commit(&harness.context, commit_request(source, p.digest, json!({}), &format!("disabled-{index}"))).await.expect("import") else { panic!("entity") };
        assert!(warnings.iter().any(|w| w.code == dto::FileWarningCode::RemoteAvatarNotDownloaded));
    }
    tokio::task::yield_now().await;
    assert_eq!(requests.load(Ordering::SeqCst), 0);
    server.abort();
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn png_cards_and_charx_agree_with_file_inspection() {
    use base64::Engine;
    let (root, harness) = setup();
    let card = json!({"spec":"chara_card_v2","spec_version":"2.0","data":{"name":"Ada","description":"Keeper","first_mes":"Hello"}});
    let mut png = crate::api::tests::png_bytes();
    let mut text = b"chara\0".to_vec();
    text.extend_from_slice(base64::engine::general_purpose::STANDARD.encode(card.to_string()).as_bytes());
    let mut chunk = (text.len() as u32).to_be_bytes().to_vec();
    chunk.extend_from_slice(b"tEXt");
    chunk.extend_from_slice(&text);
    chunk.extend_from_slice(&[0; 4]);
    png.truncate(33);
    png.extend_from_slice(&chunk);
    png.extend_from_slice(&[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0, 0, 0, 0]);
    for (name, bytes, expected) in [
        ("card.png", png.as_slice(), dto::FileKind::CharacterCard),
        ("card.charx", include_bytes!("../fixtures/card.charx").as_slice(), dto::FileKind::Other),
    ] {
        let path = root.join(name);
        std::fs::write(&path, bytes).expect("file");
        let source = dto::FileSource { uri: path.display().to_string() };
        let inspected = crate::api::files_inspect(&harness.context, dto::FilesInspectRequest { source: source.clone() }).await.expect("inspect");
        assert_eq!(inspected.kind, expected);
        let result = crate::api::import_preview(&harness.context, dto::ImportPreviewRequest { source }).await;
        if expected == dto::FileKind::CharacterCard {
            assert_eq!(result.expect("PNG preview").kind, inspected.kind);
        } else {
            assert_eq!(result.expect_err("unsupported CharX").code, ApiErrorCode::Unsupported);
        }
    }
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn uec3_lorebook_import_preserves_behavior_and_replays() {
    let (root, harness) = setup();
    let value = json!({
        "schema": {"name": "UEC", "version": "3.0"}, "kind": "lorebook",
        "meta": {"exportedAt": 3, "app": "LettuceAI 3.x", "createdAt": 1,
            "updatedAt": 2, "authors": [], "license": null, "sources": []},
        "entity": {"name": "UEC Harbour", "icon": null, "detection": "latest_user_message",
            "behaviorVersion": "deterministic_v2", "entries": [{"title": "Ships", "enabled": false,
                "alwaysActive": true, "keywords": ["ship.*"], "caseSensitive": true,
                "matchMode": "regex", "content": "The harbour", "priority": -7}]},
        "dependencies": [], "state": null, "assets": {}
    });
    let source = source(&root, "harbour.uec", &value);
    let inspected = crate::api::files_inspect(
        &harness.context,
        dto::FilesInspectRequest {
            source: source.clone(),
        },
    )
    .await
    .expect("inspect");
    assert_eq!(inspected.kind, dto::FileKind::Lorebook);
    let preview = preview(&harness.context, source.clone()).await;
    assert_eq!(preview.kind, inspected.kind);
    let request = commit_request(source, preview.digest, json!({}), "uec3-lorebook");
    let result = crate::api::import_commit(&harness.context, request.clone())
        .await
        .expect("import");
    assert_eq!(
        crate::api::import_commit(&harness.context, request.clone())
            .await
            .expect("replay"),
        result
    );
    let mut changed = request;
    changed.digest.push_str("changed");
    assert_eq!(
        crate::api::import_commit(&harness.context, changed)
            .await
            .expect_err("changed retry")
            .code,
        ApiErrorCode::Conflict
    );
    let graph = harness
        .context
        .backend()
        .database()
        .read_provider_backup_graph()
        .expect("graph");
    let details = graph
        .authored
        .lorebooks
        .iter()
        .find(|details| details.book.name == "UEC Harbour")
        .expect("book");
    assert_eq!(
        details.book.behavior_version,
        lettuce_context::LorebookBehaviorVersion::DeterministicV2
    );
    assert_eq!(
        details.book.detection_policy,
        lettuce_context::DetectionPolicy::LatestUserMessage
    );
    assert_eq!(
        details.entries[0].match_mode,
        lettuce_context::KeywordMatchMode::Regex
    );
    assert_eq!(details.entries[0].priority, -7);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn invalid_uec3_assets_and_versions_leave_no_lorebook_or_receipt() {
    let (root, harness) = setup();
    let database = harness.context.backend().database();
    let before = database.read_provider_backup_graph().expect("before");
    for (name, version, icon, assets, expected) in [
        (
            "hash",
            "3.0",
            "asset:a1",
            json!({"a1": {"mime": "image/png", "blake3": "0".repeat(64), "data": "aW1hZ2U="}}),
            ApiErrorCode::InvalidInput,
        ),
        (
            "major",
            "4.0",
            "asset:a1",
            json!({}),
            ApiErrorCode::Unsupported,
        ),
        (
            "missing",
            "3.0",
            "asset:a1",
            json!({}),
            ApiErrorCode::InvalidInput,
        ),
        (
            "image",
            "3.0",
            "asset:a1",
            json!({"a1": {"mime": "image/png", "blake3": blake3::hash(b"image").to_hex().as_str(), "data": "aW1hZ2U="}}),
            ApiErrorCode::InvalidInput,
        ),
    ] {
        let value = json!({
            "schema": {"name": "UEC", "version": version}, "kind": "lorebook",
            "meta": {"exportedAt": 3, "app": "LettuceAI 3.x", "createdAt": 1,
                "updatedAt": 2, "authors": [], "license": null, "sources": []},
            "entity": {"name": "Invalid", "icon": icon, "detection": "recent_message_window",
                "behaviorVersion": "legacy_v1", "entries": []},
            "dependencies": [], "state": null, "assets": assets
        });
        let source = source(&root, &format!("{name}.uec"), &value);
        if name == "major" {
            assert_eq!(
                crate::api::files_inspect(
                    &harness.context,
                    dto::FilesInspectRequest {
                        source: source.clone()
                    }
                )
                .await
                .expect_err("unsupported inspect")
                .code,
                ApiErrorCode::Unsupported
            );
        }
        let digest = blake3::hash(value.to_string().as_bytes())
            .to_hex()
            .to_string();
        let request = commit_request(source.clone(), digest, json!({}), name);
        assert_eq!(
            crate::api::import_commit(&harness.context, request)
                .await
                .expect_err(name)
                .code,
            expected
        );
        assert_eq!(
            database.read_provider_backup_graph().expect("after"),
            before
        );
        assert!(
            database
                .lookup_api_operation("import_commit", name)
                .expect("receipt")
                .is_none()
        );
        if name != "image" {
            assert_eq!(
                crate::api::import_preview(&harness.context, dto::ImportPreviewRequest { source })
                    .await
                    .expect_err(name)
                    .code,
                expected
            );
        }
    }
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn uec3_model_import_preserves_kind_configuration_and_replay() {
    use lettuce_models::{ModelCatalog, ModelProfileRepository, ProviderAccountRepository};
    let (root, harness) = setup();
    let database = harness.context.backend().database();
    let original = ModelCatalog::model_profiles(database)
        .expect("models")
        .into_iter()
        .next()
        .expect("seeded");
    let account = ProviderAccountRepository::get(database, original.provider_account_id)
        .expect("account")
        .expect("exists");
    let config = &original.config;
    let mut wire = lettuce_transfer::uec3::ModelProfile::from_stored(&original, &account);
    wire.display_name = "UEC model".into();
    wire.kind = lettuce_models::ModelKind::Embedding;
    wire.external_id = "embedding-name".into();
    wire.llama_cpp.gpu_layers = Some(17);
    wire.stable_diffusion.steps = Some(27);
    let value = json!({
        "schema": {"name":"UEC","version":"3.0"},"kind":"model_profile",
        "meta":{"exportedAt":3,"app":"LettuceAI 3.x","createdAt":1,"updatedAt":2,"authors":[],"license":null,"sources":[]},
        "entity":wire,
        "dependencies":[],"state":null,"assets":{}
    });
    let source = source(&root, "model.uec", &value);
    let p = preview(&harness.context, source.clone()).await;
    assert_eq!(
        p.kind,
        crate::api::files_inspect(
            &harness.context,
            dto::FilesInspectRequest {
                source: source.clone()
            }
        )
        .await
        .expect("inspect")
        .kind
    );
    let request = commit_request(source, p.digest, json!({}), "uec3-model");
    let result = crate::api::import_commit(&harness.context, request.clone())
        .await
        .expect("import");
    assert_eq!(
        crate::api::import_commit(&harness.context, request.clone())
            .await
            .expect("replay"),
        result
    );
    let mut changed = request;
    changed.digest.push('x');
    assert_eq!(
        crate::api::import_commit(&harness.context, changed)
            .await
            .expect_err("conflict")
            .code,
        ApiErrorCode::Conflict
    );
    let dto::ImportCommitResult::Imported { id, .. } = result else {
        panic!("imported")
    };
    let stored = ModelProfileRepository::get(database, id.parse().expect("id"))
        .expect("model")
        .expect("exists");
    assert_eq!(stored.kind, lettuce_models::ModelKind::Embedding);
    assert_eq!(stored.config.chat_parameters, config.chat_parameters);
    assert_eq!(stored.config.feature_parameters, config.feature_parameters);
    assert_eq!(stored.config.capabilities, config.capabilities);
    assert_eq!(stored.config.llama_cpp.gpu_layers, Some(17));
    assert_eq!(stored.config.stable_diffusion.steps, Some(27));
    assert_eq!(stored.provider_account_id, original.provider_account_id);
    let mut absent = value.clone();
    absent["entity"]["provider"] = json!({"kind":"missing-provider","label":"Missing account"});
    let absent_path = root.join("missing-account.uec");
    std::fs::write(&absent_path, absent.to_string()).expect("file");
    let absent_source = dto::FileSource {
        uri: absent_path.display().to_string(),
    };
    assert_eq!(
        crate::api::import_preview(
            &harness.context,
            dto::ImportPreviewRequest {
                source: absent_source.clone()
            }
        )
        .await
        .expect_err("account required")
        .code,
        ApiErrorCode::NotFound
    );
    let digest = blake3::hash(absent.to_string().as_bytes())
        .to_hex()
        .to_string();
    assert_eq!(
        crate::api::import_commit(
            &harness.context,
            commit_request(absent_source, digest, json!({}), "missing-account")
        )
        .await
        .expect_err("account required")
        .code,
        ApiErrorCode::NotFound
    );
    assert!(
        database
            .lookup_api_operation("import_commit", "missing-account")
            .expect("receipt")
            .is_none()
    );
    let target = root.join("export-model.uec");
    crate::api::model_export(
        &harness.context,
        dto::ModelExportRequest {
            id: stored.id.to_string(),
            target: Some(dto::FileTarget {
                uri: target.display().to_string(),
            }),
        },
    )
    .await
    .expect("export");
    let exported: Value =
        serde_json::from_str(&std::fs::read_to_string(target).expect("file")).expect("JSON");
    assert_eq!(exported["schema"]["version"], "3.0");
    assert_eq!(exported["entity"], value["entity"]);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn uec3_model_every_portable_setting_survives_native_conversion() {
    use lettuce_models::{ModelCatalog, ProviderAccountRepository};
    let (root, harness) = setup();
    let database = harness.context.backend().database();
    let original = ModelCatalog::model_profiles(database)
        .expect("models")
        .into_iter()
        .next()
        .expect("model");
    let account = ProviderAccountRepository::get(database, original.provider_account_id)
        .expect("account")
        .expect("exists");
    let mut value = serde_json::to_value(lettuce_transfer::uec3::ModelProfile::from_stored(
        &original, &account,
    ))
    .expect("wire");
    let chat = json!({"temperature":0.7,"topP":0.8,"topK":32,"maxOutputTokens":256,"contextLength":4096,
        "frequencyPenalty":0.2,"presencePenalty":0.3,"repetitionPenalty":1.1,"reasoningMode":"enabled","reasoningEffort":"high",
        "reasoningBudgetTokens":128,"promptCaching":{"enabled":{"retention":"one_hour"}},"sendThinkingState":true,
        "ollama":{"numCtx":4096,"numPredict":256,"numKeep":10,"numBatch":32,"numGpu":1,"numThread":8,"tfsZ":1.0,
            "typicalP":0.9,"minP":0.1,"mirostat":2,"mirostatTau":5.0,"mirostatEta":0.1,"seed":42,"stop":["STOP"]},
        "openrouter":{"pinnedProvider":"provider"}});
    let sampler = json!({"profile":"balanced","order":["penalties","top_k","top_p","min_p","typical","xtc","temp"],
        "minP":0.1,"typicalP":0.9,"repeatPenalty":1.1,"nPenRange":64,"dryMultiplier":0.8,"dryBase":1.75,"dryAllowedLength":2,
        "dryPenaltyLastN":128,"drySequenceBreakers":["\n"],"xtcProbability":0.1,"xtcThreshold":0.2,"seed":42,"adaptiveTarget":0.8,"adaptiveDecay":0.9});
    value["parameters"]["chat"] = chat.clone();
    let mut overrides = chat.as_object().expect("chat").clone();
    overrides.remove("openrouter");
    for (key, item) in &mut overrides {
        if key == "ollama" {
            for field in item.as_object_mut().expect("ollama").values_mut() {
                *field = json!({"kind":"set","value":field.clone()});
            }
        } else {
            *item = json!({"kind":"set","value":item.clone()});
        }
    }
    for feature in value["parameters"]["features"]
        .as_object_mut()
        .expect("features")
        .values_mut()
    {
        *feature = json!({"parameters":overrides,"llamaSampler":sampler});
    }
    value["llamaCpp"] = json!({"gpuLayers":12,"multiGpuEnabled":true,"gpuDeviceIds":[0,1],"gpuDistributionMode":"manual",
        "gpuManualLayers":[{"deviceId":0,"layers":8},{"deviceId":1,"layers":4}],"cpuLayers":2,"kvPlacement":"split","mainGpu":0,
        "singleGpuDeviceId":1,"priorityVramLimitBytes":1073741824u64,"threads":8,"threadsBatch":4,"ropeFreqBase":10000.0,"ropeFreqScale":1.0,
        "offloadKqv":true,"batchSize":256,"ubatchSize":128,"kvType":null,"kvTypeK":"q8_0","kvTypeV":"f16","flashAttention":"enabled",
        "swaFull":true,"chatTemplateOverride":"Authored template","chatTemplatePreset":"chatml","rawCompletionFallback":false,"strictMode":true,
        "mtpEnabled":true,"mtpPlacement":"cpu","mtpDraftTokens":4,"dflashEnabled":true,"dflashDraftTokens":4,"dflashMinProbability":0.5,
        "streamingEnabled":true,"forceGemma4Reasoning":true,"sampler":sampler});
    let cpp = json!({"profileId":"flux-2","variantId":"q4","maxReferenceImages":4,"supportsLora":true,"supportsTextToImage":true,"supportsImageEdit":true,"recommendedForScenes":true,"requiresReferenceImage":false});
    value["stableDiffusion"] = json!({"steps":27,"cfgScale":7.0,"sampler":"euler","scheduler":"normal","seed":42,"negativePrompt":"Authored negative",
        "denoisingStrength":0.7,"imageCfgScale":1.5,"distilledGuidance":3.5,"eta":0.1,"flowShift":1.2,"size":"512x512","vaeTilingEnabled":true,
        "vaeTileSizeX":256,"vaeTileSizeY":256,"vaeTileOverlap":0.25,"autoResizeReferenceImages":true,"increaseReferenceIndex":true,
        "hiresEnabled":true,"hiresUpscaler":"nearest","hiresScale":2.0,"hiresWidth":1024,"hiresHeight":1024,"hiresSteps":10,"hiresDenoisingStrength":0.4,
        "slgScale":2.5,"slgLayers":"7,8,9","slgLayerStart":0.1,"slgLayerEnd":0.8,"cacheMode":"easycache","cacheOption":"option","offloadMode":"mixed",
        "extraPrompt":"Authored extra","promptWriterInstructions":"Authored instructions","cpp":cpp});
    let caps = value["capabilities"].as_object_mut().expect("caps");
    caps.insert(
        "evidence".into(),
        json!({"source":"user_override","sourceVersion":1,"observedAt":5}),
    );
    for key in ["inputModalities", "outputModalities", "parameterSupport"] {
        for status in caps[key].as_object_mut().expect("statuses").values_mut() {
            *status = json!("supported");
        }
    }
    for key in [
        "streaming",
        "tools",
        "structuredOutput",
        "reasoning",
        "promptCache",
    ] {
        caps.insert(key.into(), json!("supported"));
    }
    caps.insert("contextLength".into(), json!(4096));
    caps.insert("maxVisibleOutputTokens".into(), json!(256));
    caps.insert("maxTotalCompletionTokens".into(), json!(512));
    for split in [true, false] {
        if !split {
            value["llamaCpp"]["kvType"] = json!("f16");
            value["llamaCpp"]["kvTypeK"] = Value::Null;
            value["llamaCpp"]["kvTypeV"] = Value::Null;
        }
        let wire: lettuce_transfer::uec3::ModelProfile =
            serde_json::from_value(value.clone()).expect("full model");
        assert_eq!(serde_json::to_value(&wire).expect("wire"), value);
        let native = wire
            .into_stored(account.id, harness.context.now())
            .expect("native");
        assert_eq!(
            serde_json::to_value(lettuce_transfer::uec3::ModelProfile::from_stored(
                &native, &account
            ))
            .expect("round trip"),
            value
        );
    }
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn uec3_local_model_files_and_installed_runtime_never_leave_the_device() {
    use lettuce_models::{ModelCatalog, ProviderAccountRepository};
    let (root, harness) = setup();
    let database = harness.context.backend().database();
    let mut original = ModelCatalog::model_profiles(database)
        .expect("models")
        .into_iter()
        .next()
        .expect("seeded");
    let mut account = ProviderAccountRepository::get(database, original.provider_account_id)
        .expect("account")
        .expect("exists");
    original.external_model_id = "/private/model.gguf".into();
    original.config.llama_cpp.mmproj_path = Some("/private/projector.gguf".into());
    original.config.llama_cpp.mtp_model_path = Some("/private/mtp.gguf".into());
    original.config.llama_cpp.dflash_model_path = Some("/private/dflash.gguf".into());
    original.config.stable_diffusion.cpp.text_encoder_path = Some("/private/encoder.gguf".into());
    original.config.stable_diffusion.cpp.vae_path = Some("/private/vae.gguf".into());
    original.config.stable_diffusion.cpp.vision_encoder_path = Some("/private/vision.gguf".into());
    original.config.stable_diffusion.cpp.runtime_release = Some("installed-release".into());
    original.config.stable_diffusion.cpp.runtime_asset = Some("installed-asset".into());
    original.config.stable_diffusion.cpp.runtime_backend = Some("installed-backend".into());
    original.config.stable_diffusion.cpp.profile_id = Some("flux-2".into());
    original.config.stable_diffusion.cpp.variant_id = Some("q4".into());
    original.config.stable_diffusion.base_loras = Some(vec![lettuce_models::StableDiffusionLora {
        path: "/private/style.safetensors".into(),
        multiplier: 0.7,
        is_high_noise: false,
        keywords: vec![],
    }]);
    for protocol in [
        lettuce_models::ProviderProtocol::LlamaCpp,
        lettuce_models::ProviderProtocol::StableDiffusion,
    ] {
        account.protocol = protocol;
        let wire = lettuce_transfer::uec3::ModelProfile::from_stored(&original, &account);
        assert_eq!(wire.external_id, lettuce_models::UNPICKED_LOCAL_MODEL_FILE);
        let json = serde_json::to_string(&wire).expect("wire");
        assert!(!json.contains("/private/") && !json.contains("installed-"));
        let imported = wire
            .into_stored(account.id, lettuce_types::TimestampMillis::new(5))
            .expect("import");
        assert_eq!(
            imported.config.stable_diffusion.cpp.profile_id,
            original.config.stable_diffusion.cpp.profile_id
        );
        assert_eq!(
            imported.config.stable_diffusion.cpp.variant_id,
            original.config.stable_diffusion.cpp.variant_id
        );
    }
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn uec3_prompt_preserves_entries_and_resolves_known_and_unknown_built_in_sources() {
    use lettuce_context::{PromptProvenance, PromptRepository};
    let (root, harness) = setup();
    let database = harness.context.backend().database();
    let built_in_id = harness.context.backend().built_in_prompt_ids().app_default;
    let built_in = PromptRepository::get(database, built_in_id)
        .expect("source")
        .expect("exists");
    let PromptProvenance::BuiltIn { key: known, .. } = &built_in.provenance else {
        panic!("built in")
    };
    for (index, key) in [Some(known.as_str()), Some("future.catalog"), None]
        .into_iter()
        .enumerate()
    {
        let value = json!({
            "schema":{"name":"UEC","version":"3.0"},"kind":"prompt",
            "meta":{"exportedAt":3,"app":"LettuceAI 3.x","createdAt":1,"updatedAt":2,"authors":[],"license":null,"sources":[]},
            "entity":{"name":format!("UEC prompt {index}"),"purpose":"direct_chat","condense":true,
                "behaviorVersion":"deterministic_v2","builtIn":key,"entries":[{
                    "name":"Context","role":"system","content":"{{character}}","enabled":false,
                    "position":"in_chat","depth":7,"conditionalMinMessages":13,"intervalTurns":5,"systemPrompt":true,
                    "conditions":{"type":"all","conditions":[{"type":"chatMode","value":"direct"}]},
                    "payload":{"type":"imageSlot","slot":"chatBackground"}}]},
            "dependencies":[],"state":null,"assets":{}
        });
        let source = source(&root, &format!("prompt-{index}.uec"), &value);
        let p = preview(&harness.context, source.clone()).await;
        assert_eq!(
            p.kind,
            crate::api::files_inspect(
                &harness.context,
                dto::FilesInspectRequest {
                    source: source.clone()
                }
            )
            .await
            .expect("inspect")
            .kind
        );
        let request = commit_request(source, p.digest, json!({}), &format!("uec3-prompt-{index}"));
        let result = crate::api::import_commit(&harness.context, request.clone())
            .await
            .expect("import");
        assert_eq!(
            crate::api::import_commit(&harness.context, request)
                .await
                .expect("replay"),
            result
        );
        let dto::ImportCommitResult::Imported { id, warnings, .. } = result else {
            panic!("imported")
        };
        let stored = PromptRepository::get(database, id.parse().expect("id"))
            .expect("prompt")
            .expect("exists");
        assert_eq!(
            stored.behavior_version,
            lettuce_context::PromptBehaviorVersion::DeterministicV2
        );
        assert!(stored.condense);
        assert_eq!(stored.entries[0].content, "{{character}}");
        assert_eq!(
            stored.entries[0].injection_position,
            lettuce_context::PromptEntryPosition::InChat
        );
        assert_eq!(stored.entries[0].depth, 7);
        assert_eq!(stored.entries[0].conditional_min_messages, Some(13));
        assert_eq!(stored.entries[0].interval_turns, Some(5));
        assert!(stored.entries[0].system_prompt && !stored.entries[0].enabled);
        assert!(stored.entries[0].conditions.is_some() && stored.entries[0].payload.is_some());
        assert!(
            stored
                .entries
                .iter()
                .all(|entry| entry.built_in_entry_key.is_none())
        );
        if index == 0 {
            assert_eq!(
                stored.provenance,
                PromptProvenance::Derived {
                    source: built_in.id,
                    source_name: built_in.name.clone()
                }
            );
        } else {
            assert_eq!(stored.provenance, PromptProvenance::User);
        }
        if index == 1 {
            assert!(
                warnings
                    .iter()
                    .any(|warning| warning.field == "builtIn:future.catalog")
            );
        }
        let target = root.join(format!("export-prompt-{index}.uec"));
        crate::api::prompt_export(
            &harness.context,
            dto::EntityExportRequest {
                id,
                target: Some(dto::FileTarget {
                    uri: target.display().to_string(),
                }),
            },
        )
        .await
        .expect("export");
        let exported: Value =
            serde_json::from_str(&std::fs::read_to_string(target).expect("file")).expect("JSON");
        assert_eq!(exported["schema"]["version"], "3.0");
        let mut expected = value["entity"].clone();
        if index == 1 {
            expected["builtIn"] = Value::Null;
        }
        assert_eq!(exported["entity"], expected);
    }
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn uec3_prompt_document_and_receipt_roll_back_together_on_late_failure() {
    let (root, harness) = setup();
    let database = harness.context.backend().database();
    let before = database.read_provider_backup_graph().expect("before");
    let document = lettuce_transfer::uec3::Prompt {
        name: "Imported program".into(),
        purpose: lettuce_transfer::uec3::PromptPurpose::DirectChat,
        condense: true,
        behavior_version: lettuce_transfer::uec3::BehaviorVersion::DeterministicV2,
        built_in: None,
        entries: vec![],
    }
    .into_document(None, harness.context.now())
    .expect("document");
    let result: Result<(), lettuce_database::ApiOperationError> = database.commit_api_operation(
        "import_commit",
        "uec3-prompt-rollback",
        "digest",
        harness.context.now(),
        |scope| {
            scope.import_prompt_file(&document).expect("write");
            Err(lettuce_database::ApiOperationError::Storage { reason: "simulated failure" })
        },
    );
    assert_eq!(result, Err(lettuce_database::ApiOperationError::Storage { reason: "simulated failure" }));
    assert_eq!(
        database.read_provider_backup_graph().expect("after"),
        before
    );
    assert!(
        database
            .lookup_api_operation("import_commit", "uec3-prompt-rollback")
            .expect("receipt")
            .is_none()
    );
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn uec3_persona_import_preserves_disabled_bundled_book_and_authored_fields() {
    use lettuce_characters::PersonaRepository;
    use lettuce_context::PersonaLorebookBindingRepository;
    let (root, harness) = setup();
    let value = json!({"schema":{"name":"UEC","version":"3.0"},"kind":"persona",
        "meta":{"exportedAt":3,"app":"LettuceAI 3.x","createdAt":1,"updatedAt":2,"authors":[],"license":null,"sources":[]},
        "entity":{"name":"Writer","nickname":"W","description":"Authored","design":{"description":"Coat","references":[]},
        "media":{"avatar":null,"avatarCrop":{"x":0.2f32,"y":0.3f32,"scale":1.2f32}},
        "behavior":{"image":null,"lorebooks":[{"book":"dep:l1","enabled":false}]}},
        "dependencies":[{"key":"l1","kind":"lorebook","entity":{"name":"Harbour","icon":null,"detection":"latest_user_message","behaviorVersion":"deterministic_v2","entries":[]}}],
        "state":null,"assets":{}});
    let source = source(&root, "persona3.uec", &value);
    let p = preview(&harness.context, source.clone()).await;
    assert_eq!(
        p.kind,
        crate::api::files_inspect(
            &harness.context,
            dto::FilesInspectRequest {
                source: source.clone()
            }
        )
        .await
        .expect("inspect")
        .kind
    );
    let request = commit_request(source, p.digest, json!({}), "persona3");
    let result = crate::api::import_commit(&harness.context, request.clone())
        .await
        .expect("import");
    assert_eq!(
        crate::api::import_commit(&harness.context, request)
            .await
            .expect("replay"),
        result
    );
    let dto::ImportCommitResult::Imported { id, .. } = result else {
        panic!("imported")
    };
    let id = id.parse().expect("id");
    let database = harness.context.backend().database();
    let persona = PersonaRepository::get(database, id)
        .expect("read")
        .expect("persona");
    assert_eq!(persona.design_description.as_deref(), Some("Coat"));
    assert_eq!(persona.nickname.as_deref(), Some("W"));
    assert_eq!(persona.avatar_crop.expect("crop").scale, 1.2);
    let bindings = database.list_persona_bindings(id).expect("bindings");
    assert_eq!(bindings.len(), 1);
    assert!(!bindings[0].enabled);
    assert_eq!(
        lettuce_context::LorebookRepository::get(database, bindings[0].lorebook_id)
            .expect("book")
            .expect("exists")
            .book
            .name,
        "Harbour"
    );
    let exported = crate::api::persona_export(
        &harness.context,
        dto::EntityExportRequest {
            id: id.to_string(),
            target: Some(dto::FileTarget {
                uri: root.join("persona-export.uec").display().to_string(),
            }),
        },
    )
    .await
    .expect("export");
    let exported: Value =
        serde_json::from_slice(&std::fs::read(exported.uri).expect("file")).expect("json");
    assert_eq!(exported["schema"]["version"], "3.0");
    assert_eq!(exported["entity"], value["entity"]);
    assert_eq!(exported["dependencies"], value["dependencies"]);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn uec3_lora_filename_lookup_is_exact_and_ambiguous_names_never_guess() {
    use lettuce_image_generation::sd_runtime::lora_library::LoraLibraryRepository;
    let backend =
        crate::AppBackend::open_in_memory(lettuce_types::TimestampMillis::new(1)).expect("backend");
    let database = backend.database();
    let now = lettuce_types::TimestampMillis::new(2);
    database
        .record_lora_file("folder/a.safetensors", "a.safetensors", 1, 1, now)
        .expect("record");
    assert_eq!(
        database
            .lora_by_filename("a.safetensors")
            .expect("lookup")
            .expect("found")
            .path,
        "folder/a.safetensors"
    );
    assert!(
        database
            .lora_by_filename("A.safetensors")
            .expect("case")
            .is_none()
    );
    database
        .record_lora_file("other/a.safetensors", "a.safetensors", 2, 2, now)
        .expect("record");
    assert!(
        database
            .lora_by_filename("a.safetensors")
            .expect("ambiguous")
            .is_none()
    );
}

#[tokio::test]
async fn uec3_template_import_preserves_roles_and_bundled_overrides() {
    use lettuce_characters::CharacterRepository;
    let (root, harness) = setup();
    let database = harness.context.backend().database();
    let character = crate::api::tests::create_character(database, "Ada", Default::default());
    let value = json!({"schema":{"name":"UEC","version":"3.0"},"kind":"chat_template",
        "meta":{"exportedAt":3,"app":"LettuceAI 3.x","createdAt":1,"updatedAt":2,"authors":[],"license":null,"sources":[]},
        "entity":{"key":"t1","name":"Opening","messages":[{"role":"user","text":"Hello"},{"role":"assistant","text":"Welcome"}],
        "prompt":"builtin:prompt_app_default","lorebooks":["dep:l1"]},
        "dependencies":[{"key":"l1","kind":"lorebook","entity":{"name":"Harbour","icon":null,"detection":"latest_user_message","behaviorVersion":"deterministic_v2","entries":[]}}],
        "state":null,"assets":{}});
    let source = source(&root, "template3.uec", &value);
    let p = preview(&harness.context, source.clone()).await;
    assert_eq!(
        p.kind,
        crate::api::files_inspect(
            &harness.context,
            dto::FilesInspectRequest {
                source: source.clone()
            }
        )
        .await
        .expect("inspect")
        .kind
    );
    let request = commit_request(
        source,
        p.digest,
        json!({"character_id":character.to_string()}),
        "template3",
    );
    let result = crate::api::import_commit(&harness.context, request.clone())
        .await
        .expect("import");
    assert_eq!(
        crate::api::import_commit(&harness.context, request)
            .await
            .expect("replay"),
        result
    );
    let dto::ImportCommitResult::Imported { id, .. } = result else {
        panic!("imported")
    };
    let details = CharacterRepository::get(database, character)
        .expect("read")
        .expect("exists");
    let starter = details
        .starters
        .iter()
        .find(|starter| starter.id.to_string() == id)
        .expect("starter");
    assert_eq!(
        starter.messages[0].role,
        lettuce_characters::StarterRole::User
    );
    assert_eq!(
        starter.messages[1].role,
        lettuce_characters::StarterRole::Assistant
    );
    assert!(starter.scene_id.is_none());
    assert!(starter.prompt_id.is_some());
    let lettuce_characters::Selection::Explicit(books) = &starter.lorebooks else {
        panic!("explicit")
    };
    assert_eq!(books.len(), 1);
    let exported = crate::api::chat_template_export(
        &harness.context,
        dto::ChatTemplateExportRequest {
            character_id: character.to_string(),
            starter_id: id,
            target: Some(dto::FileTarget {
                uri: root.join("template-export.uec").display().to_string(),
            }),
        },
    )
    .await
    .expect("export");
    let exported: Value =
        serde_json::from_slice(&std::fs::read(exported.uri).expect("file")).expect("JSON");
    assert_eq!(exported["schema"]["version"], "3.0");
    assert_eq!(exported["entity"], value["entity"]);
    assert_eq!(exported["dependencies"], value["dependencies"]);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn uec3_lora_hints_resolve_hash_then_name_and_missing_hints_warn() {
    use lettuce_characters::PersonaRepository;
    use lettuce_image_generation::sd_runtime::lora_library::LoraLibraryRepository;
    let (root, harness) = setup();
    let database = harness.context.backend().database();
    let now = harness.context.now();
    let mut record = database
        .record_lora_file("folder/ink.safetensors", "ink.safetensors", 1, 1, now)
        .expect("record");
    record.sha256 = Some("a".repeat(64));
    database.save_lora(&record, now).expect("save");
    assert!(
        database
            .lora_by_hash(&"a".repeat(64))
            .expect("metadata cache")
            .is_none()
    );
    assert_eq!(
        database
            .lora_file_by_sha256(&"a".repeat(64))
            .expect("file lookup")
            .expect("file")
            .path,
        record.path
    );
    for (index, file_name, hash, matched) in [
        (0, "renamed.safetensors", Some("a".repeat(64)), true),
        (1, "ink.safetensors", Some("b".repeat(64)), true),
        (2, "missing.safetensors", None, false),
    ] {
        let value = json!({"schema":{"name":"UEC","version":"3.0"},"kind":"persona",
            "meta":{"exportedAt":3,"app":"LettuceAI 3.x","createdAt":1,"updatedAt":2,"authors":[],"license":null,"sources":[]},
            "entity":{"name":"Writer","nickname":null,"description":"Authored","design":{"description":null,"references":[]},
                "media":{"avatar":null,"avatarCrop":null},"behavior":{"image":{"fileName":file_name,"sha256":hash,"strength":0.8f32},"lorebooks":[]}},
            "dependencies":[],"state":null,"assets":{}});
        let source = source(&root, &format!("lora-{index}.uec"), &value);
        let p = preview(&harness.context, source.clone()).await;
        assert_eq!(
            p.warnings
                .iter()
                .any(|warning| warning.field == "image.lora:missing.safetensors"),
            !matched
        );
        let result = crate::api::import_commit(
            &harness.context,
            commit_request(source, p.digest, json!({}), &format!("lora-{index}")),
        )
        .await
        .expect("import");
        let dto::ImportCommitResult::Imported { id, .. } = result else {
            panic!("imported")
        };
        let persona = PersonaRepository::get(database, id.parse().expect("id"))
            .expect("read")
            .expect("persona");
        if matched {
            let image = persona.image_recommendation.expect("recommendation");
            assert_eq!(
                image.unresolved_legacy_name.as_deref(),
                Some("folder/ink.safetensors")
            );
            assert_eq!(image.strength, 0.8);
            let target = dto::FileTarget {
                uri: root
                    .join(format!("lora-export-{index}.uec"))
                    .display()
                    .to_string(),
            };
            let exported = crate::api::persona_export(
                &harness.context,
                dto::EntityExportRequest {
                    id,
                    target: Some(target),
                },
            )
            .await
            .expect("export");
            let exported: Value =
                serde_json::from_slice(&std::fs::read(exported.uri).expect("file")).expect("JSON");
            assert_eq!(
                exported["entity"]["behavior"]["image"],
                json!({"fileName":"ink.safetensors","sha256":"a".repeat(64),"strength":0.8f32})
            );
            assert!(!exported.to_string().contains("folder/"));
        } else {
            assert!(persona.image_recommendation.is_none());
        }
    }
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn uec3_character_commands_round_trip_authored_graph_and_replay() {
    use lettuce_characters::*;
    use lettuce_context::{
        BindingInsertionTarget, CharacterLorebookBindingRepository, LorebookBindingCreate,
    };
    use lettuce_types::*;
    let (root, harness) = setup();
    let context = &harness.context;
    let database = context.backend().database();
    let now = TimestampMillis::new(10);
    let id = CharacterId::new();
    let mut uploads = Vec::new();
    for _ in 0..6 {
        uploads.push(
            context
                .media()
                .expect("media")
                .ingest_with_commit(
                    crate::api::tests::png_bytes().as_slice(),
                    lettuce_media::IngestRequest::new(
                        lettuce_media::AssetKind::OtherImage,
                        lettuce_media::AssetOrigin::Import,
                        lettuce_media::RetentionClass::Persistent,
                        Default::default(),
                    ),
                    |upload| upload,
                )
                .expect("stage"),
        );
    }
    let asset_ids = uploads
        .iter()
        .map(|upload| upload.asset.id)
        .collect::<Vec<_>>();
    database
        .commit_api_operation("fixture", "uec3-character-assets", "digest", now, |scope| {
            for upload in uploads {
                scope.install_library_upload(upload).expect("install");
            }
            Ok::<_, lettuce_database::ApiOperationError>(())
        })
        .expect("assets");
    let model_id = lettuce_models::ModelCatalog::model_profiles(database).expect("models")[0].id;
    let provider = lettuce_speech::TtsConfigurationRepository::upsert_audio_provider(
        database,
        lettuce_speech::AudioProvider {
            id: AudioProviderId::new(),
            secret_owner_id: lettuce_settings::SecretOwnerId::new(),
            label: "Card voice account".into(),
            api_key_ref: None,
            config: lettuce_speech::AudioProviderConfig::OpenAiCompatible {
                base_url: Some("https://example.org".into()),
                request_path: None,
            },
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        },
        None,
    )
    .expect("voice provider");
    let voice_profile = lettuce_speech::TtsConfigurationRepository::upsert_user_voice(
        database,
        lettuce_speech::UserVoice {
            id: VoiceProfileId::new(),
            provider_id: provider.id,
            name: "Card profile voice".into(),
            model_id: "tts".into(),
            voice_id: "voice".into(),
            prompt: Some("Authored voice prompt".into()),
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        },
        None,
    )
    .expect("voice profile");
    let lora = lettuce_image_generation::sd_runtime::lora_library::LoraLibraryRepository::record_lora_file(database,"folder/full.safetensors","full.safetensors",1,1,now).expect("LoRA");
    let first = SceneId::new();
    let second = SceneId::new();
    let variant_id = SceneVariantId::new();
    let link = SceneAssetLinkId::new();
    let starter_ids = [
        ConversationStarterId::new(),
        ConversationStarterId::new(),
        ConversationStarterId::new(),
        ConversationStarterId::new(),
    ];
    let book = lettuce_context::LorebookRepository::create(
        database,
        lettuce_context::LorebookMetadataDraft {
            name: "Harbour".into(),
            detection_policy: lettuce_context::DetectionPolicy::LatestUserMessage,
            icon_asset_id: Some(asset_ids[4]),
            behavior_version: lettuce_context::LorebookBehaviorVersion::DeterministicV2,
        },
        vec![],
        now,
    )
    .expect("book");
    let mut presentation = CharacterPresentationV1 {
        card_style: CardStyle::Banner,
        avatar_crop: Some(Crop {
            x: 0.2,
            y: 0.3,
            scale: 1.2,
        }),
        banner_crop: Some(Crop {
            x: 0.1,
            y: 0.4,
            scale: 1.4,
        }),
        disable_gradient: true,
        gradient_source: GradientSource::Round,
        custom_gradient_enabled: true,
        custom_gradient_colors: vec!["#112233".into(), "#445566".into()],
        primary_text_color: Some("#ffffff".into()),
        secondary_text_color: Some("#dddddd".into()),
        ..Default::default()
    };
    presentation.chat_appearance.chat_widget_area_enabled = true;
    presentation.chat_appearance.chat_widget_slots.left = vec![
        WidgetNode::CharacterInfo {
            id: "self-info".into(),
            design: Some(WidgetDesign::Minimal),
            character_id: Some(id),
        },
        WidgetNode::Image {
            id: "image".into(),
            design: Some(WidgetDesign::Default),
            title: Some("Picture".into()),
            description: Some("Authored".into()),
            source: WidgetImageSource::LogicalAsset {
                asset_id: asset_ids[4],
            },
            shape: Some(WidgetImageShape::Wide),
        },
        WidgetNode::CharacterInfo {
            id: "current-info".into(),
            design: None,
            character_id: None,
        },
        WidgetNode::CharacterInfo {
            id: "other-info".into(),
            design: None,
            character_id: Some(harness.character_id),
        },
    ];
    let direct = context
        .backend()
        .built_in_prompt_ids()
        .get(crate::generation::built_in_prompts::BuiltInPromptId::AppDefault);
    let base = lettuce_context::PromptRepository::get(database, direct)
        .expect("prompt")
        .expect("built in");
    let mut edited =
        lettuce_transfer::uec3::Prompt::from_document(&base, Some("prompt_app_default".into()))
            .expect("wire prompt");
    edited.name = "Edited direct prompt".into();
    edited.entries[0].content.push_str(" Authored addition");
    let edited = edited.into_document(Some(&base), now).expect("derived");
    database
        .commit_api_operation("fixture", "uec3-edited-prompt", "digest", now, |scope| {
            scope.import_prompt_file(&edited).expect("write");
            Ok::<_, lettuce_database::ApiOperationError>(())
        })
        .expect("prompt");
    let direct = edited.id;
    let group = context
        .backend()
        .built_in_prompt_ids()
        .get(crate::generation::built_in_prompts::BuiltInPromptId::GroupChat);
    let group_roleplay = context
        .backend()
        .built_in_prompt_ids()
        .get(crate::generation::built_in_prompts::BuiltInPromptId::GroupChatRoleplay);
    let character = Character::new(
        id,
        CharacterProfile {
            name: "UEC Ada".into(),
            nickname: Some("A".into()),
            description: Some("Keeper".into()),
            definition: Some("Precise".into()),
            design_description: Some("Coat".into()),
            scenario: Some("Harbour".into()),
        },
        CharacterProvenance {
            creator: Some("Author".into()),
            creator_notes: Some("Notes".into()),
            localized_creator_notes: std::collections::BTreeMap::from([(
                "tr".into(),
                "Notlar".into(),
            )]),
            sources: vec!["https://example.org".into()],
            tags: vec!["keeper".into()],
        },
        CharacterDefaults {
            default_scene_id: Some(first),
            default_starter_id: Some(starter_ids[0]),
            direct_prompt_id: Some(direct),
            group_conversation_prompt_id: Some(group),
            group_roleplay_prompt_id: Some(group_roleplay),
            voice_autoplay: true,
            memory_policy: MemoryPolicy::Dynamic,
            model_profile_id: Some(model_id),
            voice: Some(VoicePreference::Provider {
                provider_id: provider.id,
                voice_id: "voice".into(),
                model_id: Some("tts".into()),
                voice_name: Some("Authored voice".into()),
            }),
            ..Default::default()
        },
        presentation,
        Some(ImageRecommendation {
            artifact_id: None,
            unresolved_legacy_name: Some(lora.path),
            strength: 0.8,
        }),
        CharacterMedia {
            links: vec![
                CharacterMediaLink {
                    asset_id: asset_ids[0],
                    slot: CharacterMediaSlot::AvatarOriginal,
                    ordinal: 0,
                },
                CharacterMediaLink {
                    asset_id: asset_ids[1],
                    slot: CharacterMediaSlot::Banner,
                    ordinal: 0,
                },
                CharacterMediaLink {
                    asset_id: asset_ids[2],
                    slot: CharacterMediaSlot::Background,
                    ordinal: 0,
                },
                CharacterMediaLink {
                    asset_id: asset_ids[3],
                    slot: CharacterMediaSlot::DesignReference,
                    ordinal: 0,
                },
            ],
        },
        now,
    )
    .expect("character");
    let scenes = vec![
        Scene {
            id: first,
            owner: SceneOwner::Character(id),
            status: LifecycleStatus::Active,
            ordinal: 0,
            content: SceneDocumentV1::new(vec![
                ScenePart::Text {
                    text: "Opening".into(),
                },
                ScenePart::InlineAsset { link_id: link },
            ])
            .expect("document"),
            direction: Some("North".into()),
            selected_variant_id: Some(variant_id),
            assets: vec![
                SceneAssetLink {
                    id: link,
                    asset_id: asset_ids[4],
                    slot: SceneAssetSlot::Inline,
                    ordinal: 0,
                },
                SceneAssetLink {
                    id: SceneAssetLinkId::new(),
                    asset_id: asset_ids[5],
                    slot: SceneAssetSlot::Background,
                    ordinal: 0,
                },
            ],
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        },
        Scene::new(
            second,
            SceneOwner::Character(id),
            1,
            SceneDocumentV1::new(vec![ScenePart::Text {
                text: "Second".into(),
            }])
            .expect("document"),
            now,
        )
        .expect("scene"),
    ];
    let variants = vec![SceneVariant {
        id: variant_id,
        scene_id: first,
        ordinal: 0,
        content: SceneDocumentV1::new(vec![
            ScenePart::Text {
                text: "Alternate".into(),
            },
            ScenePart::InlineAsset { link_id: link },
        ])
        .expect("document"),
        direction: Some("South".into()),
        revision: Revision::INITIAL,
        created_at: now,
        updated_at: now,
    }];
    let starters = starter_ids
        .into_iter()
        .enumerate()
        .map(|(index, starter_id)| ConversationStarter {
            id: starter_id,
            character_id: id,
            name: format!("Opening {index}"),
            ordinal: u32::try_from(index).expect("ordinal"),
            messages: vec![StarterMessage {
                id: StarterMessageId::new(),
                role: if index == 1 {
                    StarterRole::User
                } else {
                    StarterRole::Assistant
                },
                content: format!("Greeting {index}"),
            }],
            scene_id: if index == 0 { Some(first) } else { None },
            prompt_id: if index == 0 { Some(direct) } else { None },
            lorebooks: match index {
                0 => Selection::Inherit,
                1 => Selection::Explicit(vec![]),
                2 => Selection::Explicit(vec![book.book.id]),
                _ => Selection::Disabled,
            },
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        })
        .collect();
    let created = CharacterRepository::create(
        database,
        CreateCharacterPlan {
            character,
            scenes,
            variants,
            starters,
        },
    )
    .expect("create graph");
    let bound = database
        .bind_character_lorebook(
            id,
            created.character.revision,
            LorebookBindingCreate {
                lorebook_id: book.book.id,
                target: BindingInsertionTarget::Append,
            },
            now,
        )
        .expect("bind");
    database
        .set_character_lorebook_enabled(id, bound.owner_revision, book.book.id, false, now)
        .expect("disable");
    let exported = crate::api::character_export(
        context,
        dto::CharacterExportRequest {
            include_learned: false,
            id: id.to_string(),
            format: dto::CharacterExportFormat::Uec,
            target: Some(dto::FileTarget {
                uri: root.join("character3.uec").display().to_string(),
            }),
        },
    )
    .await
    .expect("export");
    let value: Value =
        serde_json::from_slice(&std::fs::read(&exported.uri).expect("file")).expect("JSON");
    assert_eq!(value["schema"]["version"], "3.0");
    assert!(value["state"].is_null());
    assert_eq!(value["entity"]["starters"][0]["lorebooks"], "inherit");
    assert_eq!(value["entity"]["starters"][1]["lorebooks"], json!([]));
    assert_eq!(
        value["entity"]["starters"][2]["lorebooks"]
            .as_array()
            .expect("books")
            .len(),
        1
    );
    assert_eq!(value["entity"]["starters"][3]["lorebooks"], "disabled");
    assert_eq!(value["assets"].as_object().expect("assets").len(), 1);
    assert!(!value.to_string().contains(&id.to_string()));
    let mut repeated = value.clone();
    let image = repeated["entity"]["scenes"][0]["parts"][1].clone();
    repeated["entity"]["scenes"][0]["parts"]
        .as_array_mut()
        .expect("parts")
        .push(image);
    let repeated_source = source(&root, "repeated-inline.uec", &repeated);
    let repeated_preview = preview(context, repeated_source.clone()).await;
    let repeated_result = crate::api::import_commit(
        context,
        commit_request(
            repeated_source,
            repeated_preview.digest,
            json!({}),
            "repeated-inline",
        ),
    )
    .await
    .expect("repeated image import");
    let dto::ImportCommitResult::Imported {
        id: repeated_id, ..
    } = repeated_result
    else {
        panic!("imported")
    };
    let repeated_export = crate::api::character_export(
        context,
        dto::CharacterExportRequest {
            include_learned: false,
            id: repeated_id,
            format: dto::CharacterExportFormat::Uec,
            target: Some(dto::FileTarget {
                uri: root.join("repeated-again.uec").display().to_string(),
            }),
        },
    )
    .await
    .expect("repeat export");
    let repeated_again: Value =
        serde_json::from_slice(&std::fs::read(repeated_export.uri).expect("file")).expect("JSON");
    assert_eq!(repeated_again["entity"], repeated["entity"]);
    let mut profiled = value.clone();
    profiled["entity"]["behavior"]["voice"]["provider"] = json!({"voiceProfile":{"name":voice_profile.name,"provider":"open_ai_tts","externalId":voice_profile.voice_id}});
    let profile_source = source(&root, "profile-voice.uec", &profiled);
    let profile_preview = preview(context, profile_source.clone()).await;
    let profile_result = crate::api::import_commit(
        context,
        commit_request(
            profile_source,
            profile_preview.digest,
            json!({}),
            "profile-voice",
        ),
    )
    .await
    .expect("profile import");
    let dto::ImportCommitResult::Imported { id: profile_id, .. } = profile_result else {
        panic!("imported")
    };
    let profile_stored = CharacterRepository::get(database, profile_id.parse().expect("id"))
        .expect("read")
        .expect("character");
    assert_eq!(
        profile_stored.character.defaults.voice,
        Some(VoicePreference::VoiceProfile(voice_profile.id))
    );
    let mut unresolved = value.clone();
    unresolved["entity"]["behavior"]["model"] =
        json!({"name":"Missing model","provider":"missing-provider","externalId":"missing-model"});
    unresolved["entity"]["behavior"]["voice"]["provider"] = json!({"kind":"openai","account":{"provider":"openai","label":"Missing account"},"voiceId":"voice","modelId":"tts","voiceName":"Missing voice"});
    unresolved["entity"]["behavior"]["image"] =
        json!({"fileName":"missing-lora.safetensors","sha256":null,"strength":0.8f32});
    let unresolved_source = source(&root, "unresolved-hints.uec", &unresolved);
    let unresolved_preview = preview(context, unresolved_source.clone()).await;
    for field in [
        "behavior.model:Missing model",
        "behavior.voice.account:openai:Missing account",
        "image.lora:missing-lora.safetensors",
    ] {
        assert!(
            unresolved_preview
                .warnings
                .iter()
                .any(|warning| warning.field == field),
            "{field}"
        );
    }
    let unresolved_result = crate::api::import_commit(
        context,
        commit_request(
            unresolved_source,
            unresolved_preview.digest,
            json!({}),
            "unresolved-hints",
        ),
    )
    .await
    .expect("import with warnings");
    let dto::ImportCommitResult::Imported {
        id: unresolved_id,
        warnings,
        ..
    } = unresolved_result
    else {
        panic!("imported")
    };
    assert_eq!(warnings.len(), 3);
    let unresolved_stored = CharacterRepository::get(database, unresolved_id.parse().expect("id"))
        .expect("read")
        .expect("character");
    assert!(
        unresolved_stored
            .character
            .defaults
            .model_profile_id
            .is_none()
    );
    assert!(unresolved_stored.character.defaults.voice.is_none());
    assert!(unresolved_stored.character.image_recommendation.is_none());
    let source = dto::FileSource { uri: exported.uri };
    let p = preview(context, source.clone()).await;
    assert_eq!(
        p.uec3
            .as_ref()
            .expect("UEC preview")
            .character
            .as_ref()
            .expect("draft")
            .scenes
            .len(),
        2
    );
    let before = database.read_provider_backup_graph().expect("before crash");
    let file = super::read(context, &source).expect("read");
    let mut staged = Vec::new();
    let mut pins = Vec::new();
    let mut warnings = Vec::new();
    let (card, assets) = super::read_uec3_staged(
        context,
        &file,
        &mut staged,
        &mut pins,
        &mut warnings,
    )
    .expect("card");
    let card = card.expect("UEC document");
    let books = super::uec3_books(context, &card.document, &assets).expect("books");
    let prompts =
        super::uec3_prompts(context, &card.document, &mut warnings).expect("prompts");
    let plan = super::character3::prepare(
        context,
        &card.document,
        super::character3::ImportAssets {
            ids: &assets,
            uploads: &mut staged,
            preview: false,
        },
        &mut warnings,
        context.now(),
        &books,
        &prompts,
    )
    .expect("plan");
    let crashing_database =
        lettuce_database::Database::open(root.join("state.sqlite3")).expect("crash connection");
    let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), lettuce_database::ApiOperationError> = crashing_database
            .commit_api_operation(
                "import_commit",
                "uec3-character-crash",
                "digest",
                context.now(),
                |scope| {
                    for upload in staged {
                        scope.install_library_upload(upload).expect("asset write");
                    }
                    for book in books.values() {
                        scope.import_lorebook_file(book).expect("book write");
                    }
                    for prompt in prompts.values() {
                        scope.import_prompt_file(prompt).expect("prompt write");
                    }
                    scope.import_character_file(&plan).expect("graph write");
                    panic!("simulated crash before receipt");
                },
            );
    }));
    assert!(crashed.is_err());
    assert_eq!(
        database.read_provider_backup_graph().expect("after crash"),
        before
    );
    assert!(
        database
            .lookup_api_operation("import_commit", "uec3-character-crash")
            .expect("receipt")
            .is_none()
    );
    drop(pins);
    let request = commit_request(source, p.digest, json!({}), "uec3-character");
    let (a, b) = tokio::join!(
        crate::api::import_commit(context, request.clone()),
        crate::api::import_commit(context, request.clone())
    );
    let imported = a.expect("import");
    assert_eq!(b.expect("concurrent replay"), imported);
    let mut changed = request;
    changed.digest.push('x');
    assert_eq!(
        crate::api::import_commit(context, changed)
            .await
            .expect_err("conflict")
            .code,
        ApiErrorCode::Conflict
    );
    let dto::ImportCommitResult::Imported {
        id: imported_id, ..
    } = imported
    else {
        panic!("imported")
    };
    assert_ne!(imported_id, id.to_string());
    let imported_starters = CharacterRepository::get(database, imported_id.parse().expect("id"))
        .expect("read")
        .expect("character")
        .starters;
    assert!(matches!(imported_starters[0].lorebooks, Selection::Inherit));
    assert!(
        matches!(&imported_starters[1].lorebooks,Selection::Explicit(books) if books.is_empty())
    );
    assert!(matches!(&imported_starters[2].lorebooks,Selection::Explicit(books) if books.len()==1));
    assert!(matches!(
        imported_starters[3].lorebooks,
        Selection::Disabled
    ));
    let exported = crate::api::character_export(
        context,
        dto::CharacterExportRequest {
            include_learned: false,
            id: imported_id,
            format: dto::CharacterExportFormat::Uec,
            target: Some(dto::FileTarget {
                uri: root.join("character3-again.uec").display().to_string(),
            }),
        },
    )
    .await
    .expect("re-export");
    let again: Value =
        serde_json::from_slice(&std::fs::read(exported.uri).expect("file")).expect("JSON");
    assert_eq!(again["entity"], value["entity"]);
    assert_eq!(again["dependencies"], value["dependencies"]);
    assert_eq!(again["assets"], value["assets"]);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn uec3_every_dependency_imports_once_and_shared_image_gets_distinct_logical_assets() {
    use base64::Engine;
    let (root, harness) = setup();
    let bytes = crate::api::tests::png_bytes();
    let value = json!({"schema":{"name":"UEC","version":"3.0"},"kind":"persona",
        "meta":{"exportedAt":3,"app":"LettuceAI 3.x","createdAt":1,"updatedAt":2,"authors":[],"license":null,"sources":[]},
        "entity":{"name":"Writer","nickname":null,"description":"Authored","design":{"description":"Coat","references":["asset:a1"]},
            "media":{"avatar":"asset:a1","avatarCrop":null},"behavior":{"image":null,"lorebooks":[{"book":"dep:l1","enabled":false}]}},
        "dependencies":[
            {"key":"l1","kind":"lorebook","entity":{"name":"Bundle book","icon":"asset:a1","detection":"latest_user_message","behaviorVersion":"deterministic_v2","entries":[]}},
            {"key":"p1","kind":"prompt","entity":{"name":"Bundle prompt","purpose":"direct_chat","condense":false,"behaviorVersion":"legacy_v1","builtIn":null,"entries":[]}}],
        "state":null,"assets":{"a1":{"mime":"image/png","blake3":blake3::hash(&bytes).to_hex().to_string(),"data":base64::engine::general_purpose::STANDARD.encode(&bytes)}}});
    let source = source(&root, "dependencies.uec", &value);
    let p = preview(&harness.context, source.clone()).await;
    let uec3 = p.uec3.as_ref().expect("UEC 3 details");
    assert_eq!(uec3.dependencies.len(), 2);
    assert!(!uec3.include_learned);
    assert_eq!(uec3.persona.as_ref().expect("persona draft").media.len(), 2);
    let result = crate::api::import_commit(
        &harness.context,
        commit_request(source, p.digest, json!({}), "all-dependencies"),
    )
    .await
    .expect("import");
    let dto::ImportCommitResult::Imported { id, .. } = result else {
        panic!("imported")
    };
    let database = harness.context.backend().database();
    let graph = database.read_provider_backup_graph().expect("graph");
    assert_eq!(
        graph
            .prompts
            .iter()
            .filter(|prompt| prompt.name == "Bundle prompt")
            .count(),
        1
    );
    assert_eq!(
        graph
            .authored
            .lorebooks
            .iter()
            .filter(|book| book.book.name == "Bundle book")
            .count(),
        1
    );
    let persona = lettuce_characters::PersonaRepository::get(database, id.parse().expect("id"))
        .expect("read")
        .expect("persona");
    assert_eq!(persona.media.links.len(), 2);
    assert_ne!(
        persona.media.links[0].asset_id,
        persona.media.links[1].asset_id
    );
    let mut with_persona = value.clone();
    with_persona["dependencies"].as_array_mut().expect("dependencies").push(json!({"key":"a1","kind":"persona","entity":{"name":"Bundle persona","nickname":null,"description":"Bundled","design":{"description":null,"references":[]},"media":{"avatar":null,"avatarCrop":null},"behavior":{"image":null,"lorebooks":[]}}}));
    let path = root.join("persona-dependency.uec");
    std::fs::write(&path, with_persona.to_string()).expect("write");
    let refused = crate::api::import_preview(
        &harness.context,
        dto::ImportPreviewRequest {
            source: dto::FileSource {
                uri: path.display().to_string(),
            },
        },
    )
    .await
    .expect_err("a persona is not a dependency");
    assert_eq!(refused.code, ApiErrorCode::InvalidInput);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn uec3_companion_authored_and_learned_state_round_trip_without_conversation_data() {
    use lettuce_characters::*;
    use lettuce_companions::*;
    use lettuce_memory::*;
    use lettuce_types::*;
    let (root, harness) = setup();
    let context = &harness.context;
    let database = context.backend().database();
    let now = TimestampMillis::new(10);
    let id = CharacterId::new();
    let mut config = CompanionSoulConfig::default();
    config.soul.essence = "Keeper".into();
    config.soul.traits = "Patient".into();
    config.soul.backstory = "Harbour".into();
    config.soul.appearance = "Coat".into();
    config.soul.goals = "Help".into();
    config.soul.likes = "Tea".into();
    config.soul.voice = "Quiet".into();
    config.soul.relational_style = "Warm".into();
    config.soul.vulnerabilities = "Loss".into();
    config.soul.fears = "Storm".into();
    config.soul.habits = "Walk".into();
    config.soul.boundaries = "Respect".into();
    config.soul.baseline_affect = EmotionVector {
        warmth: 0.1,
        trust: 0.2,
        calm: 0.3,
        vulnerability: 0.4,
        longing: 0.5,
        hurt: 0.6,
        tension: 0.7,
        irritation: 0.8,
        affection_intensity: 0.9,
        reassurance_need: 1.0,
    };
    config.soul.regulation_style = RegulationStyle {
        suppression: 0.1,
        volatility: 0.2,
        recovery_speed: 0.3,
        conflict_avoidance: 0.4,
        reassurance_seeking: 0.5,
        protest_behavior: 0.6,
        emotional_transparency: 0.7,
        attachment_activation: 0.8,
        pride: 0.9,
    };
    config.relationship_defaults = RelationshipDefaults {
        closeness: 0.2,
        trust: 0.3,
        affection: 0.4,
        tension: 0.5,
    };
    config.prompting.prompt_template_id = Some(
        context
            .backend()
            .built_in_prompt_ids()
            .get(crate::generation::built_in_prompts::BuiltInPromptId::Companion),
    );
    config.prompting.style_notes = "Stay curious".into();
    config.time_awareness = true;
    config.share_memory_across_chats = false;
    config.share_soul_growth_across_chats = false;
    let authored = SoulFact {
        id: "authored-local".into(),
        category: SoulCategory::Likes,
        value: "Tea".into(),
        kind: SoulFactKind::Authored,
        policy: SoulFactPolicy::Adaptive,
        slot: "drink".into(),
        confidence: 0.82,
        evidence_count: 3,
        weight: 0.8,
        valid_from: TimestampMillis::new(3),
        valid_until: Some(TimestampMillis::new(1000)),
        locked: false,
        source_memory_ids: vec![],
        created_at: now,
        supersedes: vec![],
        superseded_by: None,
        superseded_at: None,
    };
    config.authored_facts = vec![authored.clone()];
    let mut memory = MemoryItem::written(
        MemoryId::new(),
        MemoryShortId::new(17).expect("short"),
        "We sailed".into(),
        now,
    );
    memory.category = Some(MemoryCategory::PlotEvent);
    memory.origin = MemoryOrigin::Model;
    memory.source_message_id = Some(MessageId::new());
    memory.source_role = Some(lettuce_conversations::MessageRole::Assistant);
    memory.observed_at = Some(TimestampMillis::new(7));
    memory.observed_time_precision = Some("turn".into());
    memory.supersedes = vec![MemoryId::new()];
    memory.superseded_by = Some(MemoryId::new());
    memory.superseded_at = Some(TimestampMillis::new(12));
    memory.token_count = Some(42);
    memory.access_count = 6;
    memory.is_cold = false;
    memory.is_pinned = true;
    memory.importance = Score::from_basis_points(1234).expect("score");
    memory.persistence_importance = Score::from_basis_points(2345).expect("score");
    memory.prompt_importance = Score::from_basis_points(3456).expect("score");
    memory.volatility = Score::from_basis_points(4567).expect("score");
    let learned = SoulFact {
        id: "learned-local".into(),
        category: SoulCategory::Likes,
        value: "Ships".into(),
        kind: SoulFactKind::Add,
        policy: SoulFactPolicy::Historical,
        slot: "ships".into(),
        confidence: 0.7,
        evidence_count: 2,
        weight: 0.6,
        valid_from: TimestampMillis::new(4),
        valid_until: Some(TimestampMillis::new(1001)),
        locked: true,
        source_memory_ids: vec![memory.id.to_string(), "deleted-memory-local".into()],
        created_at: now,
        supersedes: vec!["deleted-soul-local".into()],
        superseded_by: Some("consolidation".into()),
        superseded_at: Some(TimestampMillis::new(12)),
    };
    let mut authored_state = authored.clone();
    authored_state.value = "Earlier tea".into();
    authored_state.superseded_by = Some(learned.id.clone());
    authored_state.superseded_at = Some(TimestampMillis::new(11));
    let mut adjusted = learned.clone();
    adjusted.id = "adjusted-local".into();
    adjusted.kind = SoulFactKind::Adjust;
    adjusted.value = "Adjusted ships".into();
    let mut consolidated = learned.clone();
    consolidated.id = "consolidated-local".into();
    consolidated.kind = SoulFactKind::Consolidated;
    consolidated.value = "Consolidated ships".into();
    let mut user_memory = memory.clone();
    user_memory.id = MemoryId::new();
    user_memory.short_id = MemoryShortId::new(18).expect("short");
    user_memory.origin = MemoryOrigin::User;
    user_memory.source_message_id = None;
    user_memory.source_role = None;
    user_memory.observed_time_precision = Some("user".into());
    let mut undated_memory = user_memory.clone();
    undated_memory.id = MemoryId::new();
    undated_memory.short_id = MemoryShortId::new(19).expect("short");
    undated_memory.origin = MemoryOrigin::Import;
    undated_memory.observed_at = None;
    undated_memory.observed_time_precision = None;
    undated_memory.is_pinned = false;
    undated_memory.is_cold = true;
    let character = Character::new(
        id,
        CharacterProfile {
            name: "Companion".into(),
            nickname: None,
            description: None,
            definition: None,
            design_description: None,
            scenario: None,
        },
        Default::default(),
        CharacterDefaults {
            interaction_mode: InteractionMode::Companion,
            companion_soul: Some(config),
            ..Default::default()
        },
        Default::default(),
        None,
        Default::default(),
        now,
    )
    .expect("character");
    let import = lettuce_transfer::CharacterFileImport {
        lorebooks: vec![],
        character: CreateCharacterPlan {
            character,
            scenes: vec![],
            variants: vec![],
            starters: vec![],
        },
        lorebook_bindings: vec![],
        scheduled_notes: vec![CompanionScheduledNote {
            id: uuid::Uuid::new_v4(),
            character_id: id,
            label: "Anniversary".into(),
            content: "Remember".into(),
            available_at: TimestampMillis::new(-2_520_000_000_000),
            expires_at: Some(TimestampMillis::new(-2_510_000_000_000)),
            recurrence: ScheduledNoteRecurrence::Yearly,
            recurrence_window_ms: Some(100),
            enabled: false,
            created_at: now,
            updated_at: now,
        }],
        companion_memory: Some(lettuce_transfer::CharacterFileCompanionMemory {
            pool: Some(MemorySpaceSnapshot {
                id: MemorySpaceId::new(),
                revision: Revision::INITIAL,
                items: vec![memory, user_memory, undated_memory],
            }),
            soul_facts: Some(vec![authored_state, learned, adjusted, consolidated]),
            relationships: vec![],
            created_at: now,
            updated_at: now,
        }),
        skipped: vec![],
        notices: vec![],
    };
    lettuce_transfer::CharacterFileRepository::import_character_file(database, &import)
        .expect("fixture");
    let export = |id: String, include_learned: bool, name: &str| dto::CharacterExportRequest {
        id,
        include_learned,
        format: dto::CharacterExportFormat::Uec,
        target: Some(dto::FileTarget {
            uri: root.join(name).display().to_string(),
        }),
    };
    let plain = crate::api::character_export(context, export(id.to_string(), false, "plain.uec"))
        .await
        .expect("authored export");
    let plain: Value =
        serde_json::from_slice(&std::fs::read(plain.uri).expect("file")).expect("JSON");
    assert!(plain["state"].is_null());
    assert_eq!(
        plain["entity"]["companion"]["scheduledNotes"]
            .as_array()
            .expect("notes")
            .len(),
        1
    );
    assert_eq!(
        plain["entity"]["companion"]["scheduledNotes"][0]["availableAt"],
        -2_520_000_000_000_i64
    );
    assert_eq!(
        plain["entity"]["companion"]["scheduledNotes"][0]["expiresAt"],
        -2_510_000_000_000_i64
    );
    assert_eq!(
        plain["entity"]["companion"]["authoredFacts"][0]["confidence"],
        0.82
    );
    assert_eq!(
        plain["entity"]["companion"]["authoredFacts"][0]["evidenceCount"],
        3
    );
    let learned = crate::api::character_export(context, export(id.to_string(), true, "learned.uec"))
        .await
        .expect("learned export");
    let value: Value =
        serde_json::from_slice(&std::fs::read(&learned.uri).expect("file")).expect("JSON");
    let item = &value["state"]["companion"]["memory"]["items"][0];
    assert_eq!(item["observedPrecision"], "user");
    assert_eq!(item["observedAt"], 7);
    for excluded in [
        "sourceMessageId",
        "sourceRole",
        "tokenCount",
        "accessCount",
        "lastAccessedAt",
        "shortId",
        "relationships",
        "summary",
    ] {
        assert!(!value["state"].to_string().contains(excluded), "{excluded}");
    }
    assert!(!value.to_string().contains("learned-local"));
    assert!(!value.to_string().contains("deleted-memory-local"));
    assert!(!value.to_string().contains("deleted-soul-local"));
    assert_eq!(
        value["state"]["companion"]["soul"][1]["supersededBy"],
        "consolidation"
    );
    assert_eq!(
        value["state"]["companion"]["soul"][1]["sourceMemories"]
            .as_array()
            .expect("sources")
            .len(),
        2
    );
    assert_eq!(value["state"]["companion"]["soul"][0]["kind"], "authored");
    assert_eq!(
        value["state"]["companion"]["soul"][0]["value"],
        "Earlier tea"
    );
    assert_eq!(
        value["state"]["companion"]["soul"][0]["key"],
        value["entity"]["companion"]["authoredFacts"][0]["key"]
    );
    assert_eq!(value["state"]["companion"]["soul"][0]["supersededAt"], 11);
    assert!(
        value["state"]["companion"]
            .get("authoredSupersessions")
            .is_none()
    );
    let source = dto::FileSource { uri: learned.uri };
    let p = preview(context, source.clone()).await;
    let result = crate::api::import_commit(
        context,
        commit_request(source, p.digest, json!({}), "companion-learned"),
    )
    .await
    .expect("import");
    let dto::ImportCommitResult::Imported { id, .. } = result else {
        panic!("imported")
    };
    let again = crate::api::character_export(context, export(id, true, "learned-again.uec"))
        .await
        .expect("re-export");
    let again: Value =
        serde_json::from_slice(&std::fs::read(again.uri).expect("file")).expect("JSON");
    assert_eq!(again["entity"], value["entity"]);
    assert_eq!(again["state"], value["state"]);
    assert_eq!(
        value["state"]["companion"]["memory"]["items"][1]["observedPrecision"],
        "user"
    );
    assert!(value["state"]["companion"]["memory"]["items"][2]["observedAt"].is_null());
    let plain_source = dto::FileSource {
        uri: root.join("plain.uec").display().to_string(),
    };
    let p = preview(context, plain_source.clone()).await;
    let result = crate::api::import_commit(
        context,
        commit_request(plain_source, p.digest, json!({}), "companion-plain"),
    )
    .await
    .expect("normal card");
    let dto::ImportCommitResult::Imported { id, .. } = result else {
        panic!("imported")
    };
    let seeded = crate::api::character_export(context, export(id, true, "seeded.uec"))
        .await
        .expect("seeded Soul");
    let seeded: Value =
        serde_json::from_slice(&std::fs::read(seeded.uri).expect("file")).expect("JSON");
    assert_eq!(seeded["state"]["companion"]["soul"][0]["value"], "Tea");
    assert_eq!(seeded["state"]["companion"]["soul"][0]["confidence"], 0.82);
    assert_eq!(seeded["state"]["companion"]["soul"][0]["evidenceCount"], 3);
    assert_eq!(
        seeded["state"]["companion"]["soul"]
            .as_array()
            .expect("Soul")
            .len(),
        1
    );
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn uec3_artifact_only_lora_exports_a_named_warning_without_failing() {
    use lettuce_characters::*;
    use lettuce_types::*;
    let (root, harness) = setup();
    let now = harness.context.now();
    let recommendation = ImageRecommendation {
        artifact_id: Some(ModelArtifactId::new()),
        unresolved_legacy_name: None,
        strength: 0.8,
    };
    let persona = PersonaRepository::create(
        harness.context.backend().database(),
        Persona {
            id: PersonaId::new(),
            status: LifecycleStatus::Active,
            title: "Artifact recommendation".into(),
            description: "Authored".into(),
            nickname: None,
            design_description: None,
            avatar_crop: None,
            image_recommendation: Some(recommendation),
            media: PersonaMedia::default(),
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        },
    )
    .expect("persona");
    let exported = crate::api::persona_export(
        &harness.context,
        dto::EntityExportRequest {
            id: persona.id.to_string(),
            target: Some(dto::FileTarget {
                uri: root.join("artifact.uec").display().to_string(),
            }),
        },
    )
    .await
    .expect("export");
    assert_eq!(exported.warnings.len(), 1);
    assert_eq!(exported.warnings[0].field, "image.lora.artifact");
    let value: Value =
        serde_json::from_slice(&std::fs::read(exported.uri).expect("file")).expect("JSON");
    assert!(value["entity"]["behavior"]["image"].is_null());
    let invalid_path = PersonaRepository::revise(
        harness.context.backend().database(),
        persona.id,
        persona.revision,
        PersonaDraftUpdate {
            title: persona.title,
            description: persona.description,
            nickname: persona.nickname,
            design_description: persona.design_description,
            avatar_crop: persona.avatar_crop,
            image_recommendation: Some(ImageRecommendation {
                artifact_id: None,
                unresolved_legacy_name: Some("/".into()),
                strength: 0.8,
            }),
        },
        now,
    )
    .expect("unresolvable legacy path");
    let exported = crate::api::persona_export(
        &harness.context,
        dto::EntityExportRequest {
            id: invalid_path.id.to_string(),
            target: Some(dto::FileTarget {
                uri: root.join("invalid-lora-path.uec").display().to_string(),
            }),
        },
    )
    .await
    .expect("LoRA never fails export");
    assert_eq!(exported.warnings[0].field, "image.lora.filename");
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn uec3_archived_scenes_are_omitted_and_named_references_warn() {
    use lettuce_characters::*;
    use lettuce_types::*;
    let (root, harness) = setup();
    let now = harness.context.now();
    let id = CharacterId::new();
    let scene_id = SceneId::new();
    let starter_id = ConversationStarterId::new();
    let character = Character::new(
        id,
        CharacterProfile {
            name: "Archived scenes".into(),
            nickname: None,
            description: None,
            definition: None,
            design_description: None,
            scenario: None,
        },
        Default::default(),
        CharacterDefaults {
            default_scene_id: Some(scene_id),
            default_starter_id: Some(starter_id),
            ..Default::default()
        },
        Default::default(),
        None,
        Default::default(),
        now,
    )
    .expect("character");
    let mut scene = Scene::new(
        scene_id,
        SceneOwner::Character(id),
        0,
        SceneDocumentV1::new(vec![ScenePart::Text {
            text: "Archived opening".into(),
        }])
        .expect("content"),
        now,
    )
    .expect("scene");
    scene.status = LifecycleStatus::Archived;
    let starter = ConversationStarter {
        id: starter_id,
        character_id: id,
        name: "Archived scene starter".into(),
        ordinal: 0,
        messages: vec![StarterMessage {
            id: StarterMessageId::new(),
            role: StarterRole::Assistant,
            content: "Hello".into(),
        }],
        scene_id: Some(scene_id),
        prompt_id: None,
        lorebooks: Selection::Inherit,
        revision: Revision::INITIAL,
        created_at: now,
        updated_at: now,
    };
    CharacterRepository::create(
        harness.context.backend().database(),
        CreateCharacterPlan {
            character,
            scenes: vec![scene],
            variants: vec![],
            starters: vec![starter],
        },
    )
    .expect("fixture");
    let exported = crate::api::character_export(
        &harness.context,
        dto::CharacterExportRequest {
            id: id.to_string(),
            include_learned: false,
            format: dto::CharacterExportFormat::Uec,
            target: Some(dto::FileTarget {
                uri: root.join("archived.uec").display().to_string(),
            }),
        },
    )
    .await
    .expect("export");
    let value: Value =
        serde_json::from_slice(&std::fs::read(exported.uri).expect("file")).expect("JSON");
    assert_eq!(value["entity"]["scenes"], json!([]));
    assert!(value["entity"]["defaultScene"].is_null());
    assert!(value["entity"]["starters"][0]["scene"].is_null());
    assert_eq!(
        exported
            .warnings
            .iter()
            .map(|warning| warning.field.as_str())
            .collect::<Vec<_>>(),
        ["starters.t1.scene", "defaultScene"]
    );
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn uec3_prompt_references_round_trip_as_stored_whatever_their_purpose() {
    use crate::generation::built_in_prompts::BuiltInPromptId;
    use lettuce_characters::*;
    use lettuce_companions::CompanionSoulConfig;
    use lettuce_types::*;
    let (root, harness) = setup();
    let context = &harness.context;
    let database = context.backend().database();
    let now = TimestampMillis::new(10);
    let prompt = |id| context.backend().built_in_prompt_ids().get(id);
    let profile = |name: &str| CharacterProfile {
        name: name.into(),
        nickname: None,
        description: None,
        definition: None,
        design_description: None,
        scenario: None,
    };
    let starter = |character_id, prompt_id| ConversationStarter {
        id: ConversationStarterId::new(),
        character_id,
        name: "Opening".into(),
        ordinal: 0,
        messages: vec![StarterMessage {
            id: StarterMessageId::new(),
            role: StarterRole::Assistant,
            content: "Hello".into(),
        }],
        scene_id: None,
        prompt_id: Some(prompt_id),
        lorebooks: Selection::Inherit,
        revision: Revision::INITIAL,
        created_at: now,
        updated_at: now,
    };
    let roleplay_id = CharacterId::new();
    let roleplay = Character::new(
        roleplay_id,
        profile("Roleplay"),
        Default::default(),
        CharacterDefaults {
            direct_prompt_id: Some(prompt(BuiltInPromptId::Companion)),
            ..Default::default()
        },
        Default::default(),
        None,
        Default::default(),
        now,
    )
    .expect("roleplay");
    CharacterRepository::create(
        database,
        CreateCharacterPlan {
            character: roleplay,
            scenes: vec![],
            variants: vec![],
            starters: vec![starter(
                roleplay_id,
                prompt(BuiltInPromptId::GroupChatRoleplay),
            )],
        },
    )
    .expect("roleplay graph");
    let companion_id = CharacterId::new();
    let companion = Character::new(
        companion_id,
        profile("Companion"),
        Default::default(),
        CharacterDefaults {
            interaction_mode: InteractionMode::Companion,
            companion_soul: Some(CompanionSoulConfig::default()),
            ..Default::default()
        },
        Default::default(),
        None,
        Default::default(),
        now,
    )
    .expect("companion");
    CharacterRepository::create(
        database,
        CreateCharacterPlan {
            character: companion,
            scenes: vec![],
            variants: vec![],
            starters: vec![starter(companion_id, prompt(BuiltInPromptId::AppDefault))],
        },
    )
    .expect("companion graph");
    for (id, expected) in [
        (roleplay_id, prompt(BuiltInPromptId::GroupChatRoleplay)),
        (companion_id, prompt(BuiltInPromptId::AppDefault)),
    ] {
        let name = format!("{id}.uec");
        let exported = crate::api::character_export(
            context,
            dto::CharacterExportRequest {
                id: id.to_string(),
                include_learned: false,
                format: dto::CharacterExportFormat::Uec,
                target: Some(dto::FileTarget {
                    uri: root.join(&name).display().to_string(),
                }),
            },
        )
        .await
        .expect("export");
        let source = dto::FileSource { uri: exported.uri };
        let p = preview(context, source.clone()).await;
        let result =
            crate::api::import_commit(context, commit_request(source, p.digest, json!({}), &name))
                .await
                .expect("a card with a prompt of any purpose imports");
        let dto::ImportCommitResult::Imported { id: imported, .. } = result else {
            panic!("imported")
        };
        let stored = CharacterRepository::get(database, imported.parse().expect("id"))
            .expect("read")
            .expect("character");
        assert_eq!(stored.starters[0].prompt_id, Some(expected));
    }
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn uec3_learned_export_of_a_former_companion_writes_no_state() {
    use lettuce_characters::*;
    use lettuce_memory::*;
    use lettuce_types::*;
    let (root, harness) = setup();
    let context = &harness.context;
    let now = TimestampMillis::new(10);
    let id = CharacterId::new();
    let character = Character::new(
        id,
        CharacterProfile {
            name: "Former companion".into(),
            nickname: None,
            description: None,
            definition: None,
            design_description: None,
            scenario: None,
        },
        Default::default(),
        CharacterDefaults::default(),
        Default::default(),
        None,
        Default::default(),
        now,
    )
    .expect("character");
    let memory = MemoryItem::written(
        MemoryId::new(),
        MemoryShortId::new(1).expect("short"),
        "Kept memory".into(),
        now,
    );
    let import = lettuce_transfer::CharacterFileImport {
        lorebooks: vec![],
        character: CreateCharacterPlan {
            character,
            scenes: vec![],
            variants: vec![],
            starters: vec![],
        },
        lorebook_bindings: vec![],
        scheduled_notes: vec![],
        companion_memory: Some(lettuce_transfer::CharacterFileCompanionMemory {
            pool: Some(MemorySpaceSnapshot {
                id: MemorySpaceId::new(),
                revision: Revision::INITIAL,
                items: vec![memory],
            }),
            soul_facts: Some(vec![]),
            relationships: vec![],
            created_at: now,
            updated_at: now,
        }),
        skipped: vec![],
        notices: vec![],
    };
    lettuce_transfer::CharacterFileRepository::import_character_file(
        context.backend().database(),
        &import,
    )
    .expect("fixture");
    let exported = crate::api::character_export(
        context,
        dto::CharacterExportRequest {
            id: id.to_string(),
            include_learned: true,
            format: dto::CharacterExportFormat::Uec,
            target: Some(dto::FileTarget {
                uri: root.join("former.uec").display().to_string(),
            }),
        },
    )
    .await
    .expect("a roleplay character exports with no learned state");
    let value: Value =
        serde_json::from_slice(&std::fs::read(exported.uri).expect("file")).expect("JSON");
    assert!(value["state"].is_null());
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn uec3_character_preview_names_an_unknown_built_in_once() {
    let (root, harness) = setup();
    let exported = crate::api::character_export(
        &harness.context,
        dto::CharacterExportRequest {
            id: harness.character_id.to_string(),
            include_learned: false,
            format: dto::CharacterExportFormat::Uec,
            target: Some(dto::FileTarget {
                uri: root.join("base.uec").display().to_string(),
            }),
        },
    )
    .await
    .expect("export");
    let mut value: Value =
        serde_json::from_slice(&std::fs::read(exported.uri).expect("file")).expect("JSON");
    value["dependencies"] = json!([{
        "key": "p1",
        "kind": "prompt",
        "entity": {
            "name": "Future prompt",
            "purpose": "direct_chat",
            "condense": false,
            "behaviorVersion": "legacy_v1",
            "builtIn": "future.catalog",
            "entries": []
        }
    }]);
    let p = preview(&harness.context, source(&root, "future.uec", &value)).await;
    let named = p
        .warnings
        .iter()
        .filter(|warning| warning.field == "builtIn:future.catalog")
        .count();
    assert_eq!(named, 1);
    std::fs::remove_dir_all(root).expect("cleanup");
}

struct RecordingWriter {
    largest: usize,
    total: usize,
}

impl std::io::Write for RecordingWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.largest = self.largest.max(bytes.len());
        self.total += bytes.len();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn uec3_export_encodes_assets_without_buffering_the_file() {
    use lettuce_characters::*;
    use lettuce_types::*;
    let (root, harness) = setup();
    let context = &harness.context;
    let database = context.backend().database();
    let now = TimestampMillis::new(10);
    let mut image = crate::api::tests::png_bytes();
    image.extend((0..3_000_000_u32).map(|index| (index.wrapping_mul(2_654_435_761) >> 24) as u8));
    let upload = context
        .media()
        .expect("media")
        .ingest_with_commit(
            image.as_slice(),
            lettuce_media::IngestRequest::new(
                lettuce_media::AssetKind::OtherImage,
                lettuce_media::AssetOrigin::Import,
                lettuce_media::RetentionClass::Persistent,
                Default::default(),
            ),
            |upload| upload,
        )
        .expect("stage");
    let asset_id = upload.asset.id;
    database
        .commit_api_operation("fixture", "uec3-large-asset", "digest", now, |scope| {
            scope.install_library_upload(upload).expect("install");
            Ok::<_, lettuce_database::ApiOperationError>(())
        })
        .expect("asset");
    let persona = PersonaRepository::create(
        database,
        Persona {
            id: PersonaId::new(),
            status: LifecycleStatus::Active,
            title: "Large avatar".into(),
            description: "Authored".into(),
            nickname: None,
            design_description: None,
            avatar_crop: None,
            image_recommendation: None,
            media: PersonaMedia {
                links: vec![PersonaMediaLink {
                    asset_id,
                    slot: PersonaMediaSlot::Avatar,
                    ordinal: 0,
                }],
            },
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        },
    )
    .expect("persona");
    let export = super::export_uec3_persona(context, &persona, &mut Vec::new())
        .expect("export");
    let mut recorder = RecordingWriter {
        largest: 0,
        total: 0,
    };
    export.write(context, &mut recorder).expect("write");
    assert!(recorder.total > image.len());
    assert!(
        recorder.largest <= 64 * 1024,
        "largest write {}",
        recorder.largest
    );
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn uec3_commit_reads_and_stages_each_asset_once() {
    use base64::Engine;
    let (root, harness) = setup();
    let first = crate::api::tests::png_bytes();
    let mut second = crate::api::tests::png_bytes();
    second.extend_from_slice(b"different");
    let asset = |bytes: &[u8]| {
        json!({
            "mime": "image/png",
            "blake3": blake3::hash(bytes).to_hex().to_string(),
            "data": base64::engine::general_purpose::STANDARD.encode(bytes)
        })
    };
    let value = json!({"schema":{"name":"UEC","version":"3.0"},"kind":"persona",
        "meta":{"exportedAt":3,"app":"LettuceAI 3.x","createdAt":1,"updatedAt":2,"authors":[],"license":null,"sources":[]},
        "entity":{"name":"Writer","nickname":null,"description":"Authored","design":{"description":null,"references":["asset:a2"]},
            "media":{"avatar":"asset:a1","avatarCrop":null},"behavior":{"image":null,"lorebooks":[]}},
        "dependencies":[],"state":null,"assets":{"a1":asset(&first),"a2":asset(&second)}});
    let source = source(&root, "two-assets.uec", &value);
    let file = super::read(&harness.context, &source).expect("read");
    let mut uploads = Vec::new();
    let mut pins = Vec::new();
    let mut warnings = Vec::new();
    let (card, assets) = super::read_uec3_staged(
        &harness.context,
        &file,
        &mut uploads,
        &mut pins,
        &mut warnings,
    )
    .expect("staged");
    assert!(card.is_some());
    assert_eq!(assets.len(), 2);
    assert_eq!(uploads.len(), 2);
    assert_eq!(pins.len(), 2);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn uec3_template_preview_tells_disabled_lorebooks_from_none() {
    let (root, harness) = setup();
    let template = |lorebooks: Value| {
        json!({"schema":{"name":"UEC","version":"3.0"},"kind":"chat_template",
            "meta":{"exportedAt":3,"app":"LettuceAI 3.x","createdAt":1,"updatedAt":2,"authors":[],"license":null,"sources":[]},
            "entity":{"key":"t1","name":"Opening","messages":[{"role":"assistant","text":"Welcome"}],
            "prompt":null,"lorebooks":lorebooks},
            "dependencies":[],"state":null,"assets":{}})
    };
    for (name, lorebooks, ids, disabled) in [
        ("disabled.uec", json!("disabled"), None, true),
        ("none.uec", json!([]), Some(Vec::new()), false),
        ("inherit.uec", json!("inherit"), None, false),
    ] {
        let p = preview(&harness.context, source(&root, name, &template(lorebooks))).await;
        let dto::ImportPreviewContent::ChatTemplate {
            lorebook_ids_override,
            lorebooks_disabled,
            ..
        } = p.content
        else {
            panic!("template preview")
        };
        assert_eq!(lorebook_ids_override, ids, "{name}");
        assert_eq!(lorebooks_disabled, disabled, "{name}");
    }
    std::fs::remove_dir_all(root).expect("folder");
}
