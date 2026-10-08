use super::tests::{Reply, harness};
use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_models::ModelPathRelocation;
use lettuce_settings::DeviceSettingsStore;

fn save(
    id: Option<String>,
    key: Option<&str>,
    clear: bool,
    revision: Option<u64>,
    operation: &str,
) -> dto::ProviderAccountSaveRequest {
    dto::ProviderAccountSaveRequest {
        account: dto::ProviderAccountInput {
            id,
            provider_kind: "openai".into(),
            label: "Test provider".into(),
            base_url: None,
            enabled: true,
            streaming_enabled: true,
            allow_invalid_tls: false,
            config: serde_json::json!({"kind":"standard"}),
        },
        api_key: key.map(str::to_owned),
        clear_api_key: clear,
        expected_revision: revision,
        client_operation_id: operation.into(),
    }
}

#[tokio::test]
async fn account_save_replays_conflicts_and_preserves_or_clears_the_key() {
    let harness = harness(Reply::Text("Hello."));
    let request = save(None, Some("secret-canary"), false, None, "create-provider");
    let first = super::provider_account_save(&harness.context, request.clone())
        .await
        .expect("save");
    assert!(first.api_key_set);
    assert!(
        !serde_json::to_string(&first)
            .expect("view")
            .contains("secret-canary")
    );
    assert_eq!(
        super::provider_account_save(&harness.context, request)
            .await
            .expect("replay"),
        first
    );
    assert_eq!(
        super::provider_account_save(
            &harness.context,
            save(None, Some("another-key"), false, None, "create-provider")
        )
        .await
        .expect_err("digest")
        .code,
        ApiErrorCode::Conflict
    );
    let blank = super::provider_account_save(
        &harness.context,
        save(
            Some(first.id.clone()),
            Some(" "),
            false,
            Some(first.revision),
            "blank-provider",
        ),
    )
    .await
    .expect("blank");
    assert!(blank.api_key_set);
    assert_eq!(
        super::provider_account_save(
            &harness.context,
            save(
                Some(first.id.clone()),
                None,
                true,
                Some(first.revision),
                "stale-provider"
            )
        )
        .await
        .expect_err("stale")
        .code,
        ApiErrorCode::Conflict
    );
    let clear = super::provider_account_save(
        &harness.context,
        save(
            Some(first.id),
            None,
            true,
            Some(blank.revision),
            "clear-provider",
        ),
    )
    .await
    .expect("clear");
    assert!(!clear.api_key_set);
}

#[tokio::test]
async fn provider_delete_lists_models_and_cascade_keeps_conversation_snapshots() {
    use lettuce_models::{ModelCatalog, ModelProfileRepository};
    let harness = harness(Reply::Text("Hello."));
    let conversation = super::tests::launch(&harness, "provider-delete-chat").await;
    let conversation_id = conversation.parse().expect("conversation id");
    let frozen = lettuce_conversations::ConversationReader::get(
        harness.context.backend().database(),
        conversation_id,
    )
    .expect("snapshot");
    let profiles = harness
        .context
        .backend()
        .database()
        .model_profiles()
        .expect("models");
    let profile = profiles.first().expect("model");
    let character = lettuce_characters::CharacterRepository::get(
        harness.context.backend().database(),
        harness.character_id,
    )
    .expect("character")
    .expect("exists")
    .character;
    let mut defaults = character.defaults;
    defaults.model_profile_id = Some(profile.id);
    lettuce_characters::CharacterRepository::update_defaults(
        harness.context.backend().database(),
        harness.character_id,
        character.revision,
        defaults,
        harness.context.now(),
    )
    .expect("select model");
    let request = dto::ProviderAccountDeleteRequest {
        account_id: profile.provider_account_id.to_string(),
        delete_models: false,
        expected_revision: 1,
        client_operation_id: "delete-provider".into(),
    };
    let error = super::provider_account_delete(&harness.context, request.clone())
        .await
        .expect_err("in use");
    assert_eq!(error.code, ApiErrorCode::InUse);
    let mut request = request;
    request.delete_models = true;
    super::provider_account_delete(&harness.context, request.clone())
        .await
        .expect("cascade");
    super::provider_account_delete(&harness.context, request)
        .await
        .expect("replay");
    assert!(
        ModelProfileRepository::get(harness.context.backend().database(), profile.id)
            .expect("model")
            .is_none()
    );
    assert_eq!(
        lettuce_conversations::ConversationReader::get(
            harness.context.backend().database(),
            conversation_id
        )
        .expect("kept snapshot"),
        frozen
    );
    assert!(
        harness
            .events
            .events()
            .contains(&dto::ApiEvent::CharacterChanged {
                character_id: harness.character_id.to_string()
            })
    );
}

#[tokio::test]
async fn startup_cleanup_removes_a_secret_staged_before_the_account_commit() {
    use lettuce_settings::{
        SecretOwnerId, SecretPurpose, SecretRecord, SecretRef, SecretState, SecretValue,
    };
    let harness = harness(Reply::Text("Hello."));
    let record = SecretRecord::new(
        SecretRef::new(),
        SecretPurpose::ProviderApiKey {
            owner: SecretOwnerId::new(),
        },
    );
    harness
        .context
        .backend()
        .database()
        .stage_provider_secret(&record)
        .expect("stage");
    harness
        .context
        .secret_store()
        .put(
            record.clone(),
            SecretValue::new("orphan-canary").expect("key"),
            None,
        )
        .await
        .expect("secret");
    super::provider_mutations::cleanup_secrets(&harness.context, true)
        .await
        .expect("restart cleanup");
    assert_eq!(
        harness
            .context
            .secret_store()
            .status(&record.reference, &record.purpose)
            .await
            .expect("status")
            .state,
        SecretState::Missing
    );
    super::provider_mutations::cleanup_secrets(&harness.context, true)
        .await
        .expect("repeat restart");
    assert!(
        harness
            .context
            .backend()
            .database()
            .provider_secret_cleanup()
            .expect("journal")
            .is_empty()
    );
}

#[tokio::test]
async fn certificate_remove_rejects_stale_revision_and_unknown_identity() {
    let harness = harness(Reply::Text("Hello."));
    let current = super::certificates_list(&harness.context)
        .await
        .expect("list");
    let request = dto::CertificatesRemoveRequest {
        certificate_id: uuid::Uuid::new_v4().to_string(),
        expected_revision: current.revision,
    };
    assert_eq!(
        super::certificates_remove(&harness.context, request.clone())
            .await
            .expect_err("missing")
            .code,
        ApiErrorCode::NotFound
    );
    let mut stale = request;
    stale.expected_revision += 1;
    assert_eq!(
        super::certificates_remove(&harness.context, stale)
            .await
            .expect_err("stale")
            .code,
        ApiErrorCode::Conflict
    );
    assert_eq!(
        super::certificates_list(&harness.context)
            .await
            .expect("list"),
        current
    );
}

#[tokio::test]
async fn restart_after_commit_removes_the_old_key_and_keeps_the_committed_key() {
    use lettuce_models::ProviderAccountRepository;
    use lettuce_settings::{SecretPurpose, SecretRecord, SecretRef, SecretState, SecretValue};
    let harness = harness(Reply::Text("Hello."));
    let view = super::provider_account_save(
        &harness.context,
        save(None, Some("old-canary"), false, None, "old-key"),
    )
    .await
    .expect("save");
    let id = view.id.parse().expect("id");
    let mut account = ProviderAccountRepository::get(harness.context.backend().database(), id)
        .expect("account")
        .expect("exists");
    let previous = account.api_key_ref.expect("old ref");
    let purpose = SecretPurpose::ProviderApiKey {
        owner: account.secret_owner_id,
    };
    let next = SecretRef::new();
    let record = SecretRecord::new(next, purpose.clone());
    harness
        .context
        .backend()
        .database()
        .stage_provider_secret(&record)
        .expect("stage");
    harness
        .context
        .secret_store()
        .put(record, SecretValue::new("new-canary").expect("key"), None)
        .await
        .expect("put");
    account.api_key_ref = Some(next);
    harness
        .context
        .backend()
        .database()
        .commit_api_operation::<_, lettuce_models::ModelRepositoryError>(
            "crash-test",
            "commit",
            "digest",
            harness.context.now(),
            |scope| {
                scope.save_provider_account(
                    account,
                    Some(lettuce_types::Revision::new(view.revision)),
                )
            },
        )
        .expect("commit before crash");
    super::provider_mutations::cleanup_secrets(&harness.context, true)
        .await
        .expect("restart");
    assert_eq!(
        harness
            .context
            .secret_store()
            .status(&previous, &purpose)
            .await
            .expect("old status")
            .state,
        SecretState::Missing
    );
    assert_eq!(
        harness
            .context
            .secret_store()
            .load(&next, &purpose)
            .await
            .expect("new key")
            .with(str::to_owned),
        "new-canary"
    );
}

#[tokio::test]
async fn concurrent_same_operation_replays_and_kind_changes_keep_models_attached() {
    use lettuce_models::ModelCatalog;
    let harness = harness(Reply::Text("Hello."));
    let request = save(None, Some("race-canary"), false, None, "race-save");
    let (left, right) = tokio::join!(
        super::provider_account_save(&harness.context, request.clone()),
        super::provider_account_save(&harness.context, request)
    );
    assert_eq!(left.expect("first"), right.expect("replay"));
    let profile = harness
        .context
        .backend()
        .database()
        .model_profiles()
        .expect("models")
        .remove(0);
    let mut edit = save(
        Some(profile.provider_account_id.to_string()),
        None,
        false,
        Some(1),
        "kind-change",
    );
    edit.account.provider_kind = "anthropic".into();
    let changed = super::provider_account_save(&harness.context, edit)
        .await
        .expect("change kind with model");
    assert_eq!(changed.provider_kind, "anthropic");
    assert_eq!(
        harness
            .context
            .backend()
            .database()
            .model_profiles()
            .expect("models")[0]
            .provider_account_id,
        profile.provider_account_id
    );
}

#[tokio::test]
async fn certificate_invalid_pem_writes_nothing() {
    let harness = harness(Reply::Text("Hello."));
    let file = TestFile::new();
    std::fs::write(
        file.path(),
        "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----",
    )
    .expect("write");
    let before = super::certificates_list(&harness.context)
        .await
        .expect("before");
    let request = dto::CertificatesImportRequest {
        source: dto::FileSource {
            uri: file.path().to_string_lossy().into_owned(),
        },
        client_operation_id: "invalid-pem".into(),
    };
    assert_eq!(
        super::certificates_import(&harness.context, request)
            .await
            .expect_err("invalid")
            .code,
        ApiErrorCode::InvalidInput
    );
    assert_eq!(
        super::certificates_list(&harness.context)
            .await
            .expect("after"),
        before
    );
}

#[tokio::test]
async fn certificate_import_replays_and_reloads_live_clients() {
    use lettuce_network::{JsonAuth, JsonClient, RequestPolicy};
    let harness = harness(Reply::Text("Hello."));
    let client = JsonClient::new().expect("client");
    harness
        .context
        .backend()
        .provider_json_clients
        .lock()
        .expect("registry")
        .push(client.downgrade());
    let (url, task) = tls_fixture().await;
    assert!(
        client
            .get_json(
                &url,
                "/",
                &[],
                JsonAuth::None,
                vec![],
                RequestPolicy::BROWSE
            )
            .await
            .is_err()
    );
    let file = TestFile::new();
    std::fs::write(
        file.path(),
        include_str!("../../tests/fixtures/provider-test-cert.pem"),
    )
    .expect("write");
    let request = dto::CertificatesImportRequest {
        source: dto::FileSource {
            uri: file.path().to_string_lossy().into_owned(),
        },
        client_operation_id: "import-root".into(),
    };
    let imported = super::certificates_import(&harness.context, request.clone())
        .await
        .expect("import");
    assert_eq!(
        super::certificates_import(&harness.context, request.clone())
            .await
            .expect("replay"),
        imported
    );
    assert_eq!(
        client
            .get_json(
                &url,
                "/",
                &[],
                JsonAuth::None,
                vec![],
                RequestPolicy::BROWSE
            )
            .await
            .expect("next request trusts root")
            .status,
        200
    );
    std::fs::write(
        file.path(),
        format!(
            "{}\n",
            include_str!("../../tests/fixtures/provider-test-cert.pem")
        ),
    )
    .expect("change content");
    assert_eq!(
        super::certificates_import(&harness.context, request)
            .await
            .expect("receipt ignores later source changes"),
        imported
    );
    let removed = super::certificates_remove(
        &harness.context,
        dto::CertificatesRemoveRequest {
            certificate_id: imported.certificates[0].id.clone(),
            expected_revision: imported.revision,
        },
    )
    .await
    .expect("remove");
    assert!(removed.certificates.is_empty());
    assert!(
        client
            .get_json(
                &url,
                "/",
                &[],
                JsonAuth::None,
                vec![],
                RequestPolicy::BROWSE
            )
            .await
            .is_err()
    );
    task.abort();
}

async fn tls_fixture() -> (String, tokio::task::JoinHandle<()>) {
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
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .expect("tls");
    let acceptor = TlsAcceptor::from(std::sync::Arc::new(config));
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let url = format!(
        "https://localhost:{}",
        listener.local_addr().expect("address").port()
    );
    let task = tokio::spawn(async move {
        loop {
            let (socket, _) = listener.accept().await.expect("accept");
            let Ok(mut socket) = acceptor.accept(socket).await else {
                continue;
            };
            let mut buffer = [0; 4096];
            if socket.read(&mut buffer).await.is_ok() {
                let _ = socket
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                    )
                    .await;
                let _ = socket.shutdown().await;
            }
        }
    });
    (url, task)
}

struct TestFile(std::path::PathBuf);
impl TestFile {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("s7a-{}.pem", uuid::Uuid::new_v4())))
    }
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}
impl Drop for TestFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[tokio::test]
async fn shutdown_before_a_write_saves_nothing() {
    use lettuce_models::ModelCatalog;
    let harness = harness(Reply::Text("Hello."));
    let before = harness
        .context
        .backend()
        .database()
        .provider_accounts()
        .expect("accounts");
    harness.context.begin_shutdown();
    assert_eq!(
        super::provider_account_save(
            &harness.context,
            save(None, Some("shutdown-canary"), false, None, "shutdown-save")
        )
        .await
        .expect_err("shutdown")
        .code,
        ApiErrorCode::Cancelled
    );
    assert_eq!(
        harness
            .context
            .backend()
            .database()
            .provider_accounts()
            .expect("accounts"),
        before
    );
}

#[tokio::test]
async fn provider_cascade_retains_creation_model_name_in_backup_snapshots() {
    use lettuce_models::ModelCatalog;
    use lettuce_transfer::ProviderBackupSource;
    let harness = harness(Reply::Text("Hello."));
    let book = super::lorebook_create(
        &harness.context,
        dto::LorebookCreateRequest {
            client_operation_id: "snapshot-book".into(),
            metadata: dto::LorebookMetadataInput {
                name: "History".into(),
                detection: dto::LorebookDetection::LatestUserMessage,
                icon_asset_id: None,
            },
            entries: vec![],
        },
    )
    .await
    .expect("book");
    super::lorebook_keywords_draft(
        &harness.context,
        dto::LorebookKeywordsDraftRequest {
            client_operation_id: "snapshot-keywords".into(),
            lorebook_id: book.lorebook.id,
            entry_id: None,
            title: Some("Coast".into()),
            content: "The coastal city.".into(),
            existing_keywords: vec![],
            direction: None,
        },
    )
    .await
    .expect("run");
    let profile = harness
        .context
        .backend()
        .database()
        .model_profiles()
        .expect("models")
        .remove(0);
    let before = harness
        .context
        .backend()
        .database()
        .read_provider_backup_graph()
        .expect("before");
    super::provider_account_delete(
        &harness.context,
        dto::ProviderAccountDeleteRequest {
            account_id: profile.provider_account_id.to_string(),
            expected_revision: 1,
            delete_models: true,
            client_operation_id: "snapshot-delete".into(),
        },
    )
    .await
    .expect("cascade");
    let mut after = harness
        .context
        .backend()
        .database()
        .read_provider_backup_graph()
        .expect("after");
    lettuce_transfer::canonicalize_and_validate(&mut after)
        .expect("valid graph after model deletion");
    assert_eq!(
        after.creation.lorebook_keyword_runs,
        before.creation.lorebook_keyword_runs
    );
    assert_eq!(
        after.creation.lorebook_keyword_runs[0]
            .run
            .profile
            .chat_profile
            .model_profile_id,
        profile.id
    );
    assert_eq!(
        after.creation.lorebook_keyword_runs[0]
            .run
            .profile
            .chat_profile
            .model_display_name,
        profile.display_name
    );
    let encoded =
        serde_json::to_value(&after.creation.lorebook_keyword_runs).expect("backup snapshot");
    assert_eq!(
        encoded[0]["run"]["profile"]["chat_profile"]["model_display_name"],
        profile.display_name
    );
}

#[derive(Default)]
struct PausedSecrets {
    store: lettuce_settings::InMemorySecretStore,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
    fail: std::sync::atomic::AtomicBool,
    fail_delete: std::sync::atomic::AtomicBool,
}
#[async_trait::async_trait]
impl lettuce_settings::SecretStore for PausedSecrets {
    async fn put(
        &self,
        record: lettuce_settings::SecretRecord,
        value: lettuce_settings::SecretValue,
        expected: Option<u64>,
    ) -> Result<lettuce_settings::SecretStatus, lettuce_settings::SecretStoreError> {
        if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(lettuce_settings::SecretStoreError::Backend(
                lettuce_settings::SecretBackendError::Unavailable,
            ));
        }
        let result = self.store.put(record, value, expected).await;
        self.entered.notify_one();
        self.release.notified().await;
        result
    }
    async fn load(
        &self,
        reference: &lettuce_settings::SecretRef,
        purpose: &lettuce_settings::SecretPurpose,
    ) -> Result<lettuce_settings::SecretValue, lettuce_settings::SecretStoreError> {
        self.store.load(reference, purpose).await
    }
    async fn status(
        &self,
        reference: &lettuce_settings::SecretRef,
        purpose: &lettuce_settings::SecretPurpose,
    ) -> Result<lettuce_settings::SecretStatus, lettuce_settings::SecretStoreError> {
        self.store.status(reference, purpose).await
    }
    async fn delete(
        &self,
        reference: &lettuce_settings::SecretRef,
        purpose: &lettuce_settings::SecretPurpose,
        expected: Option<u64>,
    ) -> Result<lettuce_settings::SecretStatus, lettuce_settings::SecretStoreError> {
        if self.fail_delete.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(lettuce_settings::SecretStoreError::Backend(lettuce_settings::SecretBackendError::Unavailable));
        }
        self.store.delete(reference, purpose, expected).await
    }
}

#[tokio::test]
async fn shutdown_after_secret_put_leaves_only_a_recoverable_staged_secret() {
    use lettuce_models::ModelCatalog;
    use std::sync::Arc;
    let harness = harness(Reply::Text("Hello."));
    let store = Arc::new(PausedSecrets::default());
    let context = harness.context.with_test_secrets(store.clone());
    let before = context
        .backend()
        .database()
        .provider_accounts()
        .expect("before");
    let writer = context.clone();
    let task = tokio::spawn(async move {
        super::provider_account_save(
            &writer,
            save(
                None,
                Some("interrupted-canary"),
                false,
                None,
                "interrupted-save",
            ),
        )
        .await
    });
    store.entered.notified().await;
    context.begin_shutdown();
    store.release.notify_one();
    assert_eq!(
        task.await.expect("writer").expect_err("shutdown").code,
        ApiErrorCode::Cancelled
    );
    assert_eq!(
        context
            .backend()
            .database()
            .provider_accounts()
            .expect("after"),
        before
    );
    assert_eq!(
        context
            .backend()
            .database()
            .provider_secret_cleanup()
            .expect("staged")
            .len(),
        1
    );
    super::provider_mutations::cleanup_secrets(&context, true)
        .await
        .expect("startup cleanup");
    assert!(
        context
            .backend()
            .database()
            .provider_secret_cleanup()
            .expect("journal")
            .is_empty()
    );
}

#[tokio::test]
async fn unavailable_secret_write_is_typed_and_never_commits_an_account() {
    use lettuce_models::ModelCatalog;
    use std::sync::Arc;
    let harness = harness(Reply::Text("Hello."));
    let store = Arc::new(PausedSecrets::default());
    store.fail.store(true, std::sync::atomic::Ordering::SeqCst);
    let context = harness.context.with_test_secrets(store);
    let before = context
        .backend()
        .database()
        .provider_accounts()
        .expect("before");
    let error = super::provider_account_save(
        &context,
        save(
            None,
            Some("unavailable-canary"),
            false,
            None,
            "unavailable-save",
        ),
    )
    .await
    .expect_err("unavailable");
    assert_eq!(error.code, ApiErrorCode::Unavailable);
    assert!(
        !serde_json::to_string(&error)
            .expect("error")
            .contains("unavailable-canary")
    );
    assert_eq!(
        context
            .backend()
            .database()
            .provider_accounts()
            .expect("after"),
        before
    );
}

#[tokio::test]
async fn invalid_stored_certificate_is_visible_and_removable_without_blocking_runtime() {
    use lettuce_settings::DeviceSettingsStore;
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let mut device = database.load_device_settings().expect("device");
    device.trusted_certificates.push(lettuce_settings::TrustedCertificate {
        id: uuid::Uuid::new_v4(), name: "broken.pem".into(),
        pem: "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----".into(),
        imported_at: 1,
    });
    database.save_device_settings(device).expect("stored marker-only root");
    let tls = harness.context.backend().tls_policy().expect("TLS");
    harness.context.backend().provider_runtime(harness.context.secret_store().clone(), &tls).expect("runtime opens");
    let view = super::certificates_list(&harness.context).await.expect("list");
    assert!(!view.certificates[0].valid);
    assert_eq!(view.certificates[0].reason, Some(dto::CertificateInvalidReason::InvalidPem));
    let removed = super::certificates_remove(&harness.context, dto::CertificatesRemoveRequest {
        certificate_id: view.certificates[0].id.clone(), expected_revision: view.revision,
    }).await.expect("remove");
    assert!(removed.certificates.is_empty());
}

#[tokio::test]
async fn cleanup_failure_never_blocks_startup_or_committed_save_and_delete() {
    let harness = harness(Reply::Text("Hello."));
    let store = std::sync::Arc::new(PausedSecrets::default());
    let context = harness.context.with_test_secrets(store.clone());
    store.release.notify_one();
    let first = super::provider_account_save(&context, save(None, Some("old-key"), false, None, "cleanup-first")).await.expect("first");
    store.fail_delete.store(true, std::sync::atomic::Ordering::SeqCst);
    store.release.notify_one();
    let updated = super::provider_account_save(&context, save(Some(first.id), Some("new-key"), false, Some(first.revision), "cleanup-update")).await.expect("committed save returns its view");
    let request = dto::ProviderAccountDeleteRequest { account_id: updated.id, expected_revision: updated.revision, delete_models: false, client_operation_id: "cleanup-delete".into() };
    super::provider_account_delete(&context, request.clone()).await.expect("committed delete succeeds");
    super::provider_account_delete(&context, request).await.expect("replay succeeds");
    assert_eq!(context.backend().database().provider_secret_cleanup().expect("journal").len(), 2);
    let workers = super::startup(&context).await.expect("cleanup does not prevent startup");
    tokio::time::timeout(std::time::Duration::from_secs(30), workers.started()).await.expect("workers start");
    assert_eq!(context.backend().database().provider_secret_cleanup().expect("retained").len(), 2);
    workers.stop().await;
    store.fail_delete.store(false, std::sync::atomic::Ordering::SeqCst);
    super::provider_mutations::cleanup_secrets(&context, true).await.expect("retry");
    assert!(context.backend().database().provider_secret_cleanup().expect("cleared").is_empty());
}

#[tokio::test]
async fn cleanup_failure_at_startup_preserves_staged_secret_and_starts_workers() {
    use lettuce_settings::{SecretStore, SecretRecord, SecretRef, SecretPurpose, SecretOwnerId, SecretValue};
    let harness = harness(Reply::Text("Hello."));
    let store = std::sync::Arc::new(PausedSecrets::default());
    let context = harness.context.with_test_secrets(store.clone());
    let record = SecretRecord::new(SecretRef::new(), SecretPurpose::ProviderApiKey { owner: SecretOwnerId::new() });
    context.backend().database().stage_provider_secret(&record).expect("stage");
    store.store.put(record, SecretValue::new("pending-key").expect("key"), None).await.expect("put");
    store.fail_delete.store(true, std::sync::atomic::Ordering::SeqCst);
    let workers = super::startup(&context).await.expect("startup");
    tokio::time::timeout(std::time::Duration::from_secs(30), workers.started()).await.expect("started");
    assert_eq!(context.backend().database().provider_secret_cleanup().expect("journal").len(),1);
    workers.stop().await;
}

#[tokio::test]
async fn account_receipt_digest_and_backup_are_independent_of_key_value() {
    use lettuce_transfer::ProviderBackupSource;
    let harness = harness(Reply::Text("Hello."));
    let request = save(None, Some("receipt-key-canary"), false, None, "keyless-receipt");
    super::provider_account_save(&harness.context, request.clone()).await.expect("save");
    let receipt = harness.context.backend().database().lookup_api_operation("provider_account_save", "keyless-receipt").expect("lookup").expect("receipt");
    let mut keyless = serde_json::to_value(&request).expect("request");
    keyless["api_key"] = serde_json::json!(true);
    let expected = super::jobs::local::digest(&keyless).expect("keyless digest");
    assert_eq!(receipt.request_digest, expected);
    let backup = harness.context.backend().database().read_provider_backup_graph().expect("backup");
    let bytes = serde_json::to_string(&backup).expect("backup bytes");
    assert!(!bytes.contains("receipt-key-canary"));
    assert!(!bytes.contains(&super::jobs::local::digest(&request).expect("old keyed digest")));
    assert!(!bytes.contains(&blake3::hash(b"receipt-key-canary").to_hex().to_string()));
}

#[tokio::test]
async fn certificate_import_replays_after_source_grant_is_gone() {
    let harness = harness(Reply::Text("Hello."));
    let file = TestFile::new();
    std::fs::write(file.path(), include_str!("../../tests/fixtures/provider-test-cert.pem")).expect("source");
    let request = dto::CertificatesImportRequest { source: dto::FileSource { uri: file.path().to_string_lossy().into_owned() }, client_operation_id: "expired-grant".into() };
    let first = super::certificates_import(&harness.context, request.clone()).await.expect("import");
    std::fs::remove_file(file.path()).expect("expire source");
    assert_eq!(super::certificates_import(&harness.context, request).await.expect("receipt before file access"), first);
}

#[tokio::test]
async fn certificate_source_above_cap_is_rejected_without_writing() {
    let harness = harness(Reply::Text("Hello."));
    let file = TestFile::new();
    let pem = include_str!("../../tests/fixtures/provider-test-cert.pem");
    std::fs::write(file.path(), pem.repeat(1024*1024/pem.len()+1)).expect("large valid bundle");
    let before = super::certificates_list(&harness.context).await.expect("before");
    assert_eq!(super::certificates_import(&harness.context, dto::CertificatesImportRequest { source: dto::FileSource { uri: file.path().to_string_lossy().into_owned() }, client_operation_id: "oversized-root".into() }).await.expect_err("oversize").code, ApiErrorCode::InvalidInput);
    assert_eq!(super::certificates_list(&harness.context).await.expect("after"), before);
}

#[tokio::test]
async fn duplicate_certificate_reports_its_existing_identity() {
    let harness = harness(Reply::Text("Hello."));
    let file = TestFile::new();
    std::fs::write(file.path(), include_str!("../../tests/fixtures/provider-test-cert.pem")).expect("source");
    let mut request = dto::CertificatesImportRequest { source: dto::FileSource { uri: file.path().to_string_lossy().into_owned() }, client_operation_id: "duplicate-first".into() };
    let first = super::certificates_import(&harness.context, request.clone()).await.expect("first");
    request.client_operation_id = "duplicate-second".into();
    let error = super::certificates_import(&harness.context, request).await.expect_err("duplicate");
    assert_eq!(error.code, ApiErrorCode::Conflict);
    assert_eq!(serde_json::to_value(error.details).expect("detail"), serde_json::json!({"type":"certificate_already_imported", "certificate_id":first.certificates[0].id}));
}

struct CertificateRaceSettings<'a>(&'a lettuce_database::Database);
impl lettuce_settings::DeviceSettingsStore for CertificateRaceSettings<'_> {
    fn load_device_settings(&self) -> Result<lettuce_settings::DeviceSettings, lettuce_settings::GlobalSettingsStoreError> {
        let stale = self.0.load_device_settings()?;
        self.0.commit_api_operation("certificates_import", "folder-race", "folder-race-digest", lettuce_types::TimestampMillis::new(1), |scope| {
            scope.import_certificate(lettuce_settings::TrustedCertificate {
                id: uuid::Uuid::new_v4(), name: "root.pem".into(), imported_at: 1,
                pem: include_str!("../../tests/fixtures/provider-test-cert.pem").into(),
            }).map_err(|_| lettuce_models::ModelRepositoryError::Storage)
        }).expect("concurrent certificate commit");
        Ok(stale)
    }
    fn save_device_settings(&self, settings: lettuce_settings::DeviceSettings) -> Result<(), lettuce_settings::GlobalSettingsStoreError> { self.0.save_device_settings(settings) }
    fn update_device_settings(&self, update: &dyn Fn(&mut lettuce_settings::DeviceSettings)) -> Result<(), lettuce_settings::GlobalSettingsStoreError> { self.0.update_device_settings(update) }
}
impl lettuce_models::ModelPathRelocation for CertificateRaceSettings<'_> {
    fn relocate_model_paths(&self, relocate: &dyn Fn(&str)->Option<String>, now: lettuce_types::TimestampMillis) -> Result<u32, lettuce_models::ModelRepositoryError> { self.0.relocate_model_paths(relocate, now) }
    fn relocate_model_paths_and_save_device(&self, relocate: &dyn Fn(&str)->Option<String>, device: lettuce_settings::DeviceSettings, now: lettuce_types::TimestampMillis) -> Result<u32, lettuce_models::ModelRepositoryError> { self.0.relocate_model_paths_and_save_device(relocate, device, now) }
}

#[test]
fn folder_selection_keeps_an_interleaved_certificate_import() {
    use lettuce_settings::DeviceSettingsStore;
    let database = lettuce_database::Database::open_in_memory().expect("database");
    let root = std::env::temp_dir().join(format!("s7a-folder-race-{}", uuid::Uuid::new_v4()));
    let app = root.join("app");
    let models = root.join("models");
    crate::models::gguf_library::set_llm_models_dir(&CertificateRaceSettings(&database), &app, models.to_str().expect("path"), false, lettuce_types::TimestampMillis::new(1), "folder-race", &||false).expect("folder selection");
    let after = database.load_device_settings().expect("both changes");
    assert_eq!(after.trusted_certificates.len(),1);
    assert_eq!(after.llm_models_dir.as_deref(), models.to_str());
    std::fs::remove_dir_all(root).expect("cleanup");
}
