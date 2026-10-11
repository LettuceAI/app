use std::sync::Arc;

use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_jobs::SystemClock;
use lettuce_transfer::ProviderBackupSource;
use serde_json::{Value, json};

use super::tests::{Harness, Reply, harness_over};

fn setup() -> (std::path::PathBuf, Harness) {
    let root = std::env::var_os("TMPDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/test-files")
        })
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
        Arc::new(super::NoModels),
        Arc::new(super::tests::NoImages),
    );
    (root, harness)
}

fn entity_media_store(root: &std::path::Path) -> Arc<super::ApiMediaStore> {
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

async fn preview(context: &super::ApiContext, source: dto::FileSource) -> dto::ImportPreview {
    super::import_preview(context, dto::ImportPreviewRequest { source })
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
        let inspected = super::files_inspect(
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
        super::import_commit(&harness.context, request)
            .await
            .expect_err("changed file")
            .code,
        ApiErrorCode::Conflict
    );
    std::fs::write(&source.uri, content.to_string()).expect("restore fixture");
    let before = preview(&harness.context, source.clone()).await;
    let request = commit_request(source, before.digest, json!({}), "import-book");
    let result = super::import_commit(&harness.context, request.clone())
        .await
        .expect("import");
    assert_eq!(
        super::import_commit(&harness.context, request.clone())
            .await
            .expect("replay"),
        result
    );
    let mut changed = request;
    changed.options.character_id = Some(harness.character_id.to_string());
    assert_eq!(
        super::import_commit(&harness.context, changed)
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
    let result = super::import_commit(
        &harness.context,
        commit_request(source, preview.digest, json!({}), "persona"),
    )
    .await
    .expect("import without image");
    let dto::ImportCommitResult::Imported { id, warnings, .. } = result else {
        panic!("synchronous persona")
    };
    assert!(warnings.iter().any(|warning| warning.field == "avatar"));
    let persona = super::persona_get(&harness.context, dto::PersonaGetRequest { persona_id: id })
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
        super::import_commit(
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
        super::import_commit(
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
    super::import_commit(
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
    let result = super::import_commit(
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
async fn group_import_requires_every_speaker_before_admission() {
    let (root, harness) = setup();
    let path = root.join("group.jsonl");
    std::fs::write(&path,"{\"chat_metadata\":{\"group\":true},\"character_name\":\"Group\"}\n{\"name\":\"Ada\",\"is_user\":false,\"mes\":\"One\"}\n{\"name\":\"Ben\",\"is_user\":false,\"mes\":\"Two\"}").expect("chat");
    let source = dto::FileSource {
        uri: path.display().to_string(),
    };
    let p = preview(&harness.context, source.clone()).await;
    let result = super::import_commit(
        &harness.context,
        commit_request(
            source,
            p.digest,
            json!({"speakers":{"Ada":harness.character_id.to_string()}}),
            "unmapped",
        ),
    )
    .await
    .expect_err("every speaker");
    assert_eq!(result.code, ApiErrorCode::InvalidInput);
    assert!(
        harness
            .context
            .backend()
            .database()
            .lookup_api_operation("import_commit", "unmapped")
            .expect("receipt")
            .is_none()
    );
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
    let dto::ImportCommitResult::Imported { id, .. } = super::import_commit(
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
        format: dto::LorebookExportFormat::Usc,
        target: dto::FileTarget {
            uri: target.display().to_string(),
        },
    };
    super::lorebook_export(&harness.context, request.clone())
        .await
        .expect("export");
    let content: Value =
        serde_json::from_slice(&std::fs::read(&target).expect("target")).expect("complete JSON");
    assert_eq!(content["payload"]["name"], "Harbour");
    super::lorebook_export(&harness.context, request)
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
        super::import_commit(&harness.context, request.clone())
            .await
            .expect("admit")
    else {
        panic!("job")
    };
    super::job_cancel(
        &harness.context,
        dto::JobCancelRequest {
            job_id: job_id.clone(),
        },
    )
    .await
    .expect("cancel");
    let job = super::job_get(
        &harness.context,
        dto::JobGetRequest {
            job_id: job_id.clone(),
        },
    )
    .await
    .expect("job");
    assert_eq!(job.state, dto::JobStateDto::Cancelled);
    assert_eq!(
        super::import_commit(&harness.context, request)
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
            raw,
            Some("chat"),
            Some(harness.character_id),
            harness.context.now(),
        )
        .expect("prepare");
    let result: Result<String, ApiOperationError> = database.commit_api_operation(
        "import_chat_apply",
        "crash",
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
                .expect("chat write");
            Err(ApiOperationError::Storage)
        },
    );
    assert_eq!(result, Err(ApiOperationError::Storage));
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
        super::import_commit(&harness.context, request.clone())
            .await
            .expect("admit")
    else {
        panic!("job")
    };
    let first = super::JobRunner::new(harness.context.clone(), super::JobHandlers::standard());
    let second = super::JobRunner::new(harness.context.clone(), super::JobHandlers::standard());
    let (a, b) = tokio::join!(first.run_once(), second.run_once());
    assert!(a.expect("first") || b.expect("second"));
    first.wait_idle().await;
    second.wait_idle().await;
    let job = super::job_get(
        &harness.context,
        dto::JobGetRequest {
            job_id: job_id.clone(),
        },
    )
    .await
    .expect("job");
    assert_eq!(job.state, dto::JobStateDto::Succeeded);
    assert!(matches!(
        job.result,
        Some(dto::JobResultDto::Conversation { .. })
    ));
    assert_eq!(
        super::import_commit(&harness.context, request)
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
    use super::JobHandler;
    use lettuce_jobs::{JobStore, WorkerId};
    let root = std::env::var_os("TMPDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/test-files")
        })
        .join(format!("chat-restart-{}", lettuce_types::JobId::new()));
    std::fs::create_dir_all(&root).expect("folder");
    let path = root.join("state.sqlite3");
    let backend = Arc::new(
        crate::AppBackend::open(&path, lettuce_types::TimestampMillis::new(1)).expect("backend"),
    );
    let harness = super::tests::harness_over(
        backend,
        Reply::Text("reply"),
        Arc::new(SystemClock),
        None,
        Some(root.clone()),
        Arc::new(super::NoModels),
        Arc::new(super::tests::NoImages),
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
        super::import_commit(&harness.context, request.clone())
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
    let claimed = super::entity_import_jobs::ImportHandler
        .claim(&harness.context, &snapshot, WorkerId::new())
        .await
        .expect("claim")
        .expect("claimed");
    harness.context.begin_shutdown();
    claimed
        .run(
            harness.context.clone(),
            Arc::new(super::backup_tests::Progress),
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
    let harness = super::tests::harness_over(
        backend,
        Reply::Text("reply"),
        Arc::new(SystemClock),
        None,
        Some(root.clone()),
        Arc::new(super::NoModels),
        Arc::new(super::tests::NoImages),
    );
    harness
        .context
        .recover_after_restart()
        .expect("recover orphan");
    let runner = super::JobRunner::new(harness.context.clone(), super::JobHandlers::standard());
    assert!(runner.run_once().await.expect("restart"));
    runner.wait_idle().await;
    assert_eq!(
        super::job_get(
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
        super::import_commit(&harness.context, request)
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
        "../../tests/fixtures/provider-test-server.pem"
    ))
    .expect("cert");
    let key =
        PrivateKeyDer::from_pem_slice(include_bytes!("../../tests/fixtures/provider-test-key.pem"))
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
        let bytes = super::tests::png_bytes();
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

fn trust_avatar_server(context: &super::ApiContext) {
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
                    pem: include_str!("../../tests/fixtures/provider-test-cert.pem").into(),
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
        let dto::ImportCommitResult::Imported { id, warnings, .. } = super::import_commit(
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
                !super::persona_get(&harness.context, dto::PersonaGetRequest { persona_id: id })
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
    let task = tokio::spawn(async move { super::import_commit(&context, request).await });
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
impl super::FileAccess for RefuseExport {
    fn describe(&self, uri: &str) -> Result<super::FileDescription, super::FileAccessError> {
        super::tests::StdFiles.describe(uri)
    }
    fn open(&self, uri: &str) -> Result<Box<dyn super::FileReader>, super::FileAccessError> {
        super::tests::StdFiles.open(uri)
    }
    fn create(&self, uri: &str) -> Result<Box<dyn std::io::Write + Send>, super::FileAccessError> {
        super::tests::StdFiles.create(uri)
    }
    fn remove(&self, uri: &str) -> Result<(), super::FileAccessError> {
        super::tests::StdFiles.remove(uri)
    }
    fn backup_temporary_location(
        &self,
        target: Option<&str>,
        name: &str,
    ) -> Result<String, super::FileAccessError> {
        super::tests::StdFiles.backup_temporary_location(target, name)
    }
    fn create_backup_temporary(
        &self,
        intent: &str,
        protected: super::ExportProtection<'_>,
    ) -> Result<(String, Box<dyn std::io::Write + Send>), super::FileAccessError> {
        let (path, _) = super::tests::StdFiles.create_backup_temporary(intent, protected)?;
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
fn failing_exports(harness: &Harness, root: &std::path::Path) -> super::ApiContext {
    super::ApiContext::new(super::ApiContextParts {
        backend: harness.context.shared_backend(),
        secret_store: harness.context.secret_store().clone(),
        inference: harness.provider.clone(),
        image_provider: Arc::new(super::tests::NoImages),
        models: Arc::new(super::NoModels),
        speech: Arc::new(super::NoSpeech),
        media: Some(entity_media_store(root)),
        events: harness.events.clone(),
        clock: Arc::new(SystemClock),
        files: Arc::new(RefuseExport),
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
        let dto::ImportCommitResult::Imported { id, .. } = super::import_commit(
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
    let chat = harness
        .context
        .backend()
        .chat_files()
        .import_direct(
            "{\"character_name\":\"Ada\"}\n{\"name\":\"Ada\",\"mes\":\"Hello\"}",
            Some("chat"),
            Some(harness.character_id),
            harness.context.now(),
        )
        .expect("chat")
        .to_string();
    let fail = failing_exports(&harness, &root);
    for kind in 0..7 {
        let target = root.join(format!("export-{kind}"));
        let export = async |context: &super::ApiContext| {
            let target = dto::FileTarget {
                uri: target.display().to_string(),
            };
            match kind {
                0 => {
                    super::character_export(
                        context,
                        dto::CharacterExportRequest {
                            id: character.clone(),
                            format: dto::CharacterExportFormat::Uec,
                            target,
                        },
                    )
                    .await
                }
                1 => {
                    super::persona_export(
                        context,
                        dto::EntityExportRequest {
                            id: persona.clone(),
                            target,
                        },
                    )
                    .await
                }
                2 => {
                    super::lorebook_export(
                        context,
                        dto::LorebookExportRequest {
                            id: book.clone(),
                            format: dto::LorebookExportFormat::Usc,
                            target,
                        },
                    )
                    .await
                }
                3 => {
                    super::prompt_export(
                        context,
                        dto::EntityExportRequest {
                            id: prompt.clone(),
                            target,
                        },
                    )
                    .await
                }
                4 => {
                    super::chat_template_export(
                        context,
                        dto::ChatTemplateExportRequest {
                            character_id: character.clone(),
                            starter_id: template.clone(),
                            format: dto::EntityExportFormat::Json,
                            target,
                        },
                    )
                    .await
                }
                5 => {
                    super::model_export(
                        context,
                        dto::ModelExportRequest {
                            id: model.clone(),
                            format: dto::EntityExportFormat::Usc,
                            target,
                        },
                    )
                    .await
                }
                _ => {
                    super::chat_export(
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
        assert!(export(&fail).await.is_err());
        assert_eq!(
            std::fs::read_to_string(&target).expect("target"),
            "old content"
        );
        std::fs::remove_file(&target).expect("remove old");
        assert!(export(&fail).await.is_err());
        assert!(!target.exists());
        export(&harness.context).await.expect("complete export");
        let content = std::fs::read_to_string(&target).expect("export");
        if kind == 6 {
            assert!(lettuce_transfer::parse_chat_jsonl(&content, 1).is_ok());
        } else {
            assert!(serde_json::from_str::<Value>(&content).is_ok());
        }
        assert!(!std::fs::read_dir(&root).expect("folder").any(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .starts_with("entity-")
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
        let (a,b) = tokio::join!(super::import_commit(&harness.context,request.clone()), super::import_commit(&harness.context,request.clone()));
        let result = a.expect("first import");
        assert_eq!(b.expect("concurrent replay"),result);
        let graph = database.read_provider_backup_graph().expect("graph");
        std::fs::remove_file(&source.uri).expect("remove source");
        assert_eq!(super::import_commit(&harness.context,request.clone()).await.expect("replay without source"),result);
        assert_eq!(database.read_provider_backup_graph().expect("after"),graph);
        let mut changed = request;
        changed.digest.push('0');
        assert_eq!(super::import_commit(&harness.context,changed).await.expect_err("conflict").code,ApiErrorCode::Conflict);
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
    let ben = super::tests::create_character(database, "Ben", Default::default());
    let raw = "{\"character_name\":\"Ada and Ben\"}\n{\"name\":\"Ada\",\"is_user\":false,\"mes\":\"Ada message\"}\n{\"name\":\"Ben\",\"is_user\":false,\"mes\":\"Ben message\"}";
    let mapping = std::collections::BTreeMap::from([
        ("Ada".into(), harness.character_id),
        ("Ben".into(), ben),
    ]);
    let prepared = harness
        .context
        .backend()
        .chat_files()
        .prepare_group(raw, &mapping, harness.context.now())
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
            Err(ApiOperationError::Storage)
        },
    );
    assert_eq!(result, Err(ApiOperationError::Storage));
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
    let result = super::import_commit(&harness.context, request.clone())
        .await
        .expect("admit");
    let runner = super::JobRunner::new(harness.context.clone(), super::JobHandlers::standard());
    assert!(runner.run_once().await.expect("run"));
    runner.wait_idle().await;
    let dto::ImportCommitResult::Job { job_id } = result.clone() else {
        panic!("job")
    };
    let job = super::job_get(&harness.context, dto::JobGetRequest { job_id })
        .await
        .expect("job");
    assert_eq!(job.state, dto::JobStateDto::Succeeded);
    assert_eq!(
        super::import_commit(&harness.context, request)
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
            super::tests::png_bytes().as_slice(),
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
    let package = harness.context.backend().persona_files(media).read("{\"version\":1,\"persona\":{\"title\":\"Writer\",\"description\":\"Authored\",\"isDefault\":true}}",harness.context.now()).expect("persona");
    let (mut persona, _, _) = harness
        .context
        .backend()
        .persona_files(media)
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
            Err(ApiOperationError::Storage)
        });
        assert_eq!(result, Err(ApiOperationError::Storage));
        assert_eq!(
            database.read_provider_backup_graph().expect("after"),
            before
        );
    }
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn a_missing_export_avatar_is_a_named_warning_and_keeps_authored_content() {
    use base64::Engine;
    let (root, harness) = setup();
    let image = base64::engine::general_purpose::STANDARD.encode(super::tests::png_bytes());
    let source = source(
        &root,
        "persona.json",
        &json!({"version":1,"persona":{"title":"Writer","description":"Authored"},"avatarData":image}),
    );
    let p = preview(&harness.context, source.clone()).await;
    let dto::ImportCommitResult::Imported { id, warnings, .. } = super::import_commit(
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
    let result = super::persona_export(
        &harness.context,
        dto::EntityExportRequest {
            id,
            target: dto::FileTarget {
                uri: target.display().to_string(),
            },
        },
    )
    .await
    .expect("export");
    assert!(result.warnings.iter().any(|warning| warning.code
        == dto::FileWarningCode::ImageReadFailed
        && warning.field == "avatar"));
    let package = lettuce_transfer::parse_persona_import(
        &serde_json::from_slice::<Value>(&std::fs::read(target).expect("export")).expect("JSON"),
        1,
    )
    .expect("package");
    assert_eq!(package.persona.description, "Authored");
    assert!(package.avatar_data.is_none());
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn active_database_maintenance_refuses_import_and_export_before_writing() {
    let (root, harness) = setup();
    let source = source(&root, "book.json", &json!({"name":"Harbour","entries":[]}));
    let p = preview(&harness.context, source.clone()).await;
    let dto::ImportCommitResult::Imported { id, .. } = super::import_commit(
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
    let import = super::import_commit(
        &harness.context,
        commit_request(source, p.digest, json!({}), "during-maintenance"),
    )
    .await;
    let target = root.join("during-maintenance.usc");
    let export = super::lorebook_export(
        &harness.context,
        dto::LorebookExportRequest {
            id,
            format: dto::LorebookExportFormat::Usc,
            target: dto::FileTarget {
                uri: target.display().to_string(),
            },
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
    use super::JobHandler;
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
            super::import_commit(&harness.context, request.clone())
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
        let claimed = super::entity_import_jobs::ImportHandler
            .claim(&harness.context, &snapshot, WorkerId::new())
            .await
            .expect("claim")
            .expect("claimed");
        if changed {
            std::fs::write(&path, "{\"name\":\"Ada\",\"mes\":\"Changed\"}").expect("change");
        } else {
            super::job_cancel(
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
                Arc::new(super::backup_tests::Progress),
            )
            .await
            .expect("settle");
        let job = super::job_get(
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
            super::import_commit(&harness.context, request)
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
        let dto::ImportCommitResult::Imported{file_kind,warnings,..} = super::import_commit(&harness.context,commit_request(source,p.digest,json!({}),&format!("model-prompt-{index}"))).await.expect("inert fields accepted") else {panic!("model")};
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
        for bytes in [super::tests::png_bytes(), b"this is not an image".to_vec()] {
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
        let dto::ImportCommitResult::Imported { id, warnings, .. } = super::import_commit(
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
                !super::persona_get(&harness.context, dto::PersonaGetRequest { persona_id: id })
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
