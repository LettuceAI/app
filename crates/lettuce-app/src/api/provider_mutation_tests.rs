use super::tests::{Reply, harness};
use lettuce_contracts::{self as dto, ApiErrorCode};

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
async fn certificate_import_replays_content_digest_and_reloads_live_clients() {
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
            .expect_err("digest")
            .code,
        ApiErrorCode::Conflict
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
