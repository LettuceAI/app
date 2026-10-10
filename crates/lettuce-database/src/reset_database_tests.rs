use crate::*;

fn account() -> ProviderAccount {
    ProviderAccount {
        id: ProviderAccountId::new(),
        secret_owner_id: SecretOwnerId::new(),
        provider_kind: "openrouter".into(),
        protocol: ProviderProtocol::OpenAiCompatible,
        label: "Saved account".into(),
        endpoint: Some("https://openrouter.ai/api/v1".into()),
        enabled: true,
        streaming_enabled: true,
        allow_invalid_tls: false,
        api_key_ref: Some(SecretRef::new()),
        secret_headers: Vec::new(),
        config: ProviderConfig::Standard,
        revision: Revision::INITIAL,
        created_at: TimestampMillis::new(10),
        updated_at: TimestampMillis::new(10),
    }
}

#[test]
fn reset_seed_requires_a_fence_and_preserves_only_accounts_and_model_roots() {
    let root = std::env::temp_dir().join(format!("lettuce-reset-seed-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("root");
    let source = root.join("source.sqlite3");
    let target = root.join("fresh.sqlite3");
    let database = Database::open(&source).expect("source");
    let account = ProviderAccountRepository::upsert(&database, account(), None).expect("account");
    DeviceSettingsStore::update_device_settings(&database, &|settings| {
        settings.llm_models_dir = Some("/retained/llm".into());
        settings.retained_model_roots.whisper = Some("/retained/whisper".into());
        settings.speech.dictation_model_id = Some("old-model".into());
        settings.embedding.keep_model_loaded = true;
    })
    .expect("device settings");
    let old_ui = serde_json::Map::from_iter([("old".into(), serde_json::json!(true))]);
    DeviceUiStateStore::save_device_ui_state(&database, old_ui).expect("UI state");
    assert!(matches!(
        Database::read_reset_seed(&source),
        Err(ResetDatabaseError::SourceNotFenced)
    ));
    let fence = Database::lock_file_writes(&source).expect("fence");
    fence.set_fenced(true).expect("freeze");
    let seed = Database::read_reset_seed(&source).expect("seed");
    drop(fence);
    let fresh = Database::create_reset_database(&target, &seed).expect("fresh");
    assert_eq!(
        ProviderAccountRepository::get(&fresh, account.id).expect("account"),
        Some(account)
    );
    let device = DeviceSettingsStore::load_device_settings(&fresh).expect("device");
    assert_eq!(device.llm_models_dir.as_deref(), Some("/retained/llm"));
    assert_eq!(
        device.retained_model_roots.whisper.as_deref(),
        Some("/retained/whisper")
    );
    assert_eq!(
        device.speech,
        lettuce_settings::DeviceSpeechSettings::default()
    );
    assert_eq!(
        device.embedding,
        lettuce_settings::DeviceEmbeddingSettings::default()
    );
    assert!(
        DeviceUiStateStore::load_device_ui_state(&fresh)
            .expect("fresh UI")
            .is_empty()
    );
    assert!(matches!(
        Database::create_reset_database(&target, &seed),
        Err(ResetDatabaseError::Exists)
    ));
    assert_eq!(
        DeviceUiStateStore::load_device_ui_state(&database)
            .expect("source UI")
            .get("old"),
        Some(&serde_json::json!(true))
    );
    drop((fresh, database));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn malformed_reset_seed_rolls_back_all_provider_rows() {
    let root = std::env::temp_dir().join(format!(
        "lettuce-reset-seed-invalid-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).expect("root");
    let path = root.join("fresh.sqlite3");
    let first = account();
    let mut invalid = account();
    invalid.label = String::new();
    let seed = ResetDatabaseSeed {
        accounts: vec![first, invalid],
        device: DeviceSettings::default(),
    };
    assert!(matches!(
        Database::create_reset_database(&path, &seed),
        Err(ResetDatabaseError::InvalidData)
    ));
    let recovered = Database::open(&path).expect("open abandoned target");
    let count: i64 = recovered
        .connection()
        .expect("connection")
        .query_row("SELECT count(*) FROM provider_accounts", [], |row| {
            row.get(0)
        })
        .expect("count");
    assert_eq!(count, 0);
    drop(recovered);
    std::fs::remove_dir_all(root).expect("cleanup");
}
