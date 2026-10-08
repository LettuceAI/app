use crate::{ApiOperationError, Database};
use lettuce_types::{ProviderAccountId, TimestampMillis};

#[test]
fn warning_levels_deduplicate_across_reopen_and_reset_for_new_window() {
    let folder = std::env::temp_dir().join(format!("s7b-quota-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&folder).expect("folder");
    let path = folder.join("quota.sqlite");
    let account = ProviderAccountId::new();
    {
        let db = Database::open(&path).expect("database");
        for level in [75, 90, 100] {
            assert!(
                db.record_provider_quota_warning(account, "window", level)
                    .expect("warning")
            );
            assert!(
                !db.record_provider_quota_warning(account, "window", level)
                    .expect("duplicate")
            );
        }
    }
    let db = Database::open(&path).expect("reopen");
    for level in [75, 90, 100] {
        assert!(
            !db.record_provider_quota_warning(account, "window", level)
                .expect("after restart")
        );
    }
    assert!(
        db.record_provider_quota_warning(account, "next-window", 75)
            .expect("new window")
    );
    drop(db);
    std::fs::remove_dir_all(folder).expect("cleanup");
}

#[test]
fn model_changes_are_signalled_once_on_commit_and_never_on_rollback() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let db = Database::open_in_memory().expect("database");
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    db.on_model_change(move || {
        observed.fetch_add(1, Ordering::SeqCst);
    });
    let connection = db.connection().expect("connection");
    connection
        .execute_batch("BEGIN; UPDATE app_settings SET default_model_profile_id=NULL; ROLLBACK;")
        .expect("rollback");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    drop(connection);
    let result: Result<(), ApiOperationError> = db.commit_api_operation(
        "rollback-model",
        "key",
        "digest",
        TimestampMillis::new(0),
        |scope| {
            scope
                .transaction
                .execute("INSERT INTO model_changes DEFAULT VALUES", [])
                .expect("write");
            Err(ApiOperationError::Storage)
        },
    );
    assert!(result.is_err());
    assert_eq!(db.model_change_position().expect("position"), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    db.connection().expect("connection").execute_batch("BEGIN; INSERT INTO model_changes DEFAULT VALUES; INSERT INTO model_changes DEFAULT VALUES; COMMIT;").expect("commit");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

fn seed_account(db: &Database) -> lettuce_types::ProviderAccountId {
    use lettuce_models::{
        ProviderAccount, ProviderAccountRepository, ProviderConfig, ProviderProtocol,
    };
    use lettuce_settings::SecretOwnerId;
    use lettuce_types::Revision;
    let id = ProviderAccountId::new();
    ProviderAccountRepository::upsert(
        db,
        ProviderAccount {
            id,
            secret_owner_id: SecretOwnerId::new(),
            provider_kind: "ollama".into(),
            protocol: ProviderProtocol::Ollama,
            label: "Ollama".into(),
            endpoint: Some("http://localhost:11434".into()),
            enabled: true,
            streaming_enabled: true,
            allow_invalid_tls: false,
            api_key_ref: None,
            secret_headers: vec![],
            config: ProviderConfig::Standard,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(0),
            updated_at: TimestampMillis::new(0),
        },
        None,
    )
    .expect("account");
    id
}

#[test]
fn failure_between_model_default_and_receipt_writes_rolls_everything_back() {
    use lettuce_models::{ModelCatalog, ModelKind, ModelProfile, ModelRepositoryError};
    use lettuce_settings::GlobalSettingsStore;
    use lettuce_types::{ModelProfileId, Revision};
    let db = Database::open_in_memory().expect("database");
    let account = seed_account(&db);
    let before = db.load().expect("settings");
    let position = db.model_change_position().expect("position");
    let model = ModelProfile {
        id: ModelProfileId::new(),
        provider_account_id: account,
        external_model_id: "model".into(),
        display_name: "Model".into(),
        kind: ModelKind::Chat,
        config: lettuce_models::ModelProfileConfig {
            chat_parameters: Default::default(),
            feature_parameters: Default::default(),
            capabilities: Default::default(),
            llama_cpp: Default::default(),
            stable_diffusion: Default::default(),
        },
        revision: Revision::INITIAL,
        created_at: TimestampMillis::new(1),
        updated_at: TimestampMillis::new(1),
    };
    let result: Result<ModelProfile, ModelRepositoryError> = db.commit_api_operation(
        "model_save",
        "crash-key",
        "digest",
        TimestampMillis::new(1),
        |scope| {
            scope.save_model_profile(model.clone(), None, true)?;
            Err(ModelRepositoryError::Storage)
        },
    );
    assert!(result.is_err());
    assert!(db.model_profiles().expect("models").is_empty());
    assert_eq!(db.load().expect("settings"), before);
    assert_eq!(db.model_change_position().expect("position"), position);
    assert!(
        db.lookup_api_operation("model_save", "crash-key")
            .expect("receipt")
            .is_none()
    );
    db.commit_api_operation::<ModelProfile, ModelRepositoryError>(
        "model_save",
        "crash-key",
        "digest",
        TimestampMillis::new(1),
        |scope| scope.save_model_profile(model.clone(), None, true),
    )
    .expect("retry");
    let before = db.load().expect("default");
    let position = db.model_change_position().expect("position");
    let deletion: Result<(), ModelRepositoryError> = db.commit_api_operation(
        "model_delete",
        "crash-delete",
        "digest",
        TimestampMillis::new(2),
        |scope| {
            scope.delete_model_profile(model.id, model.revision, TimestampMillis::new(2))?;
            Err(ModelRepositoryError::Storage)
        },
    );
    assert!(deletion.is_err());
    assert_eq!(db.model_profiles().expect("models"), vec![model]);
    assert_eq!(db.load().expect("default"), before);
    assert_eq!(db.model_change_position().expect("position"), position);
    assert!(
        db.lookup_api_operation("model_delete", "crash-delete")
            .expect("receipt")
            .is_none()
    );
}
