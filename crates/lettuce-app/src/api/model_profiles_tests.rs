use super::tests::{Reply, harness};
use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_models::{ModelCatalog, ModelProfileRepository};
use lettuce_settings::GlobalSettingsStore;

fn draft(context: &super::ApiContext, operation: &str) -> dto::ModelSaveRequest {
    let profile = context
        .backend()
        .database()
        .model_profiles()
        .expect("profiles")
        .remove(0);
    let mut config = profile.config;
    config.capabilities = lettuce_models::ModelCapabilities::default();
    dto::ModelSaveRequest {
        model: dto::ModelInput {
            id: None,
            provider_account_id: profile.provider_account_id.to_string(),
            external_model_id: "editor-model".into(),
            display_name: "Editor model".into(),
            kind: dto::ModelKindContract::Chat,
            config: serde_json::to_value(config).expect("config"),
            input_scopes: vec![dto::ModelModality::Text, dto::ModelModality::Image],
            output_scopes: vec![dto::ModelModality::Text],
            remote_metadata: None,
        },
        expected_revision: None,
        client_operation_id: operation.into(),
    }
}

#[tokio::test]
async fn declared_image_scopes_are_supported_and_explicit_unsupported_is_kept() {
    let h = harness(Reply::Text("Hello."));
    let request = draft(&h.context, "declared-image");
    let saved = super::model_save(&h.context, request.clone())
        .await
        .expect("save");
    let profile = ModelProfileRepository::get(
        h.context.backend().database(),
        saved.id.parse().expect("id"),
    )
    .expect("profile")
    .expect("exists");
    assert_eq!(
        profile.config.capabilities.input_modalities.image,
        lettuce_models::CapabilityStatus::Supported
    );
    assert_eq!(
        profile.config.capabilities.evidence.source,
        lettuce_models::CapabilityEvidenceSource::UserOverride
    );
    fn resolve(
        context: &super::ApiContext,
        profile: &lettuce_models::ModelProfile,
    ) -> Result<lettuce_models::ResolvedChatProfile, lettuce_models::ChatProfileResolutionError>
    {
        use lettuce_models::{
            ChatParameterResolutionInput, ChatRequirements, ExpectedModelIdentity, Modality,
            ProviderAccountRepository, resolve_chat_profile,
        };
        let account = ProviderAccountRepository::get(
            context.backend().database(),
            profile.provider_account_id,
        )
        .expect("account")
        .expect("exists");
        let expected = ExpectedModelIdentity {
            model_profile_id: profile.id,
            model_revision: profile.revision,
            provider_account_id: account.id,
            provider_account_revision: account.revision,
            external_model_id: profile.external_model_id.clone(),
            display_name: profile.display_name.clone(),
            provider_protocol: account.protocol,
            model_kind: profile.kind,
        };
        resolve_chat_profile(
            &expected,
            profile,
            &account,
            &ChatParameterResolutionInput::default(),
            &ChatRequirements {
                input_modalities: vec![Modality::Text, Modality::Image],
                ..Default::default()
            },
        )
    }
    assert!(resolve(&h.context, &profile).is_ok());
    let mut request = request;
    request.client_operation_id = "unsupported-image".into();
    request.model.config["capabilities"]["input_modalities"]["image"] =
        serde_json::json!("unsupported");
    let saved = super::model_save(&h.context, request)
        .await
        .expect("save unsupported");
    assert_eq!(
        saved.config["capabilities"]["input_modalities"]["image"],
        "unsupported"
    );
    let blocked = ModelProfileRepository::get(
        h.context.backend().database(),
        saved.id.parse().expect("id"),
    )
    .expect("profile")
    .expect("exists");
    assert!(matches!(
        resolve(&h.context, &blocked),
        Err(lettuce_models::ChatProfileResolutionError::ModalityUnsupported { .. })
    ));
}

#[tokio::test]
async fn first_save_default_delete_promotion_replay_digest_and_stale_cas() {
    let h = harness(Reply::Text("Hello."));
    let request = draft(&h.context, "first-save");
    let database = h.context.backend().database();
    for model in database.model_profiles().expect("profiles") {
        ModelProfileRepository::delete_and_clear_default(database, model.id).expect("clear");
    }
    let first = super::model_save(&h.context, request.clone())
        .await
        .expect("first");
    assert_eq!(
        database
            .load()
            .expect("settings")
            .default_model_profile_id
            .map(|id| id.to_string()),
        Some(first.id.clone())
    );
    assert_eq!(
        super::model_save(&h.context, request.clone())
            .await
            .expect("replay"),
        first
    );
    let mut changed = request.clone();
    changed.model.display_name = "changed".into();
    assert_eq!(
        super::model_save(&h.context, changed)
            .await
            .expect_err("digest conflict")
            .code,
        ApiErrorCode::Conflict
    );
    let mut second_request = request;
    second_request.client_operation_id = "second-save".into();
    let second = super::model_save(&h.context, second_request.clone())
        .await
        .expect("second");
    second_request.model.id = Some(second.id.clone());
    second_request.expected_revision = Some(second.revision + 1);
    second_request.client_operation_id = "stale-save".into();
    assert_eq!(
        super::model_save(&h.context, second_request)
            .await
            .expect_err("stale")
            .code,
        ApiErrorCode::Conflict
    );
    let deletion = dto::ModelDeleteRequest {
        model_id: first.id.clone(),
        expected_revision: first.revision,
        client_operation_id: "delete-first".into(),
    };
    super::model_delete(&h.context, deletion.clone())
        .await
        .expect("delete");
    super::model_delete(&h.context, deletion)
        .await
        .expect("replay deleted");
    assert_eq!(
        database
            .load()
            .expect("settings")
            .default_model_profile_id
            .map(|id| id.to_string()),
        Some(second.id)
    );
    assert_eq!(
        super::model_get(&h.context, dto::ModelGetRequest { model_id: first.id })
            .await
            .expect_err("deleted")
            .code,
        ApiErrorCode::NotFound
    );
}

#[tokio::test]
async fn default_set_cas_replay_and_models_feed_committed_only() {
    let h = harness(Reply::Text("Hello."));
    let mut feed = super::conversation_feed::ConversationFeed::start(&h.context)
        .await
        .expect("feed");
    let request = draft(&h.context, "feed-save");
    let saved = super::model_save(&h.context, request.clone())
        .await
        .expect("save");
    feed.publish(&h.context).await.expect("publish");
    assert_eq!(
        h.events
            .events()
            .iter()
            .filter(|event| **event == dto::ApiEvent::ModelsChanged)
            .count(),
        1
    );
    super::model_save(&h.context, request)
        .await
        .expect("replay");
    feed.publish(&h.context).await.expect("replay publish");
    assert_eq!(
        h.events
            .events()
            .iter()
            .filter(|event| **event == dto::ApiEvent::ModelsChanged)
            .count(),
        1
    );
    let settings = h.context.backend().database().load().expect("settings");
    let set = dto::ModelDefaultSetRequest {
        model_id: Some(saved.id.clone()),
        expected_revision: settings.revision.get(),
        client_operation_id: "set-default".into(),
    };
    let result = super::model_default_set(&h.context, set.clone())
        .await
        .expect("set");
    assert_eq!(
        super::model_default_set(&h.context, set.clone())
            .await
            .expect("replay"),
        result
    );
    let mut stale = set;
    stale.client_operation_id = "stale-default".into();
    assert_eq!(
        super::model_default_set(&h.context, stale)
            .await
            .expect_err("stale")
            .code,
        ApiErrorCode::Conflict
    );
    assert_eq!(
        super::models_list(&h.context)
            .await
            .expect("list")
            .models
            .len(),
        2
    );
    h.context.begin_shutdown();
    assert_eq!(
        super::model_delete(
            &h.context,
            dto::ModelDeleteRequest {
                model_id: saved.id,
                expected_revision: saved.revision,
                client_operation_id: "shutdown-delete".into()
            }
        )
        .await
        .expect_err("shutdown")
        .code,
        ApiErrorCode::Cancelled
    );
}

#[tokio::test]
async fn duplicate_copies_config_keeps_default_and_accepts_non_unique_names() {
    let h = harness(Reply::Text("Hello."));
    let source = h
        .context
        .backend()
        .database()
        .model_profiles()
        .expect("models")
        .remove(0);
    let request = dto::ModelDuplicateRequest {
        model_id: source.id.to_string(),
        display_name: "Model (Copy)".into(),
        expected_revision: source.revision.get(),
        client_operation_id: "duplicate-model".into(),
    };
    let copy = super::model_duplicate(&h.context, request.clone())
        .await
        .expect("duplicate");
    assert_ne!(copy.id, source.id.to_string());
    assert_eq!(copy.display_name, request.display_name);
    assert_eq!(
        copy.config,
        serde_json::to_value(&source.config).expect("config")
    );
    assert_eq!(
        copy.provider_account_id,
        source.provider_account_id.to_string()
    );
    assert_eq!(copy.external_model_id, source.external_model_id);
    assert_eq!(
        h.context
            .backend()
            .database()
            .load()
            .expect("settings")
            .default_model_profile_id,
        Some(source.id)
    );
    assert_eq!(
        super::model_duplicate(&h.context, request.clone())
            .await
            .expect("replay"),
        copy
    );
    let mut request = request;
    request.client_operation_id = "duplicate-again".into();
    assert!(
        super::model_duplicate(&h.context, request.clone())
            .await
            .is_ok()
    );
    request.client_operation_id = "blank-duplicate".into();
    request.display_name = " ".into();
    assert_eq!(
        super::model_duplicate(&h.context, request)
            .await
            .expect_err("blank")
            .code,
        ApiErrorCode::InvalidInput
    );
}

#[tokio::test]
async fn editor_local_path_edits_are_not_treated_as_incoming_sync() {
    use lettuce_models::ProviderAccountRepository;
    let h = harness(Reply::Text("Hello."));
    let database = h.context.backend().database();
    let mut account = database.provider_accounts().expect("accounts").remove(0);
    let expected = account.revision;
    account.provider_kind = "llamacpp".into();
    account.protocol = lettuce_models::ProviderProtocol::LlamaCpp;
    ProviderAccountRepository::upsert(database, account, Some(expected)).expect("local account");
    let mut request = draft(&h.context, "local-editor-path");
    let mut profile = database.model_profiles().expect("profiles").remove(0);
    let expected = profile.revision;
    profile.external_model_id = "/old/model.gguf".into();
    profile = ModelProfileRepository::upsert(database, profile, Some(expected)).expect("old path");
    request.model.id = Some(profile.id.to_string());
    request.model.external_model_id = "/new/model.gguf".into();
    request.expected_revision = Some(profile.revision.get());
    assert_eq!(
        super::model_save(&h.context, request)
            .await
            .expect("save new path")
            .external_model_id,
        "/new/model.gguf"
    );
}

#[tokio::test]
async fn concurrent_retries_and_model_edits_commit_one_result() {
    let h = harness(Reply::Text("Hello."));
    let request = draft(&h.context, "concurrent-create");
    let (a, b) = tokio::join!(
        super::model_save(&h.context, request.clone()),
        super::model_save(&h.context, request.clone())
    );
    let saved = a.expect("first");
    assert_eq!(saved, b.expect("replay"));
    let mut a = request;
    a.model.id = Some(saved.id.clone());
    a.expected_revision = Some(saved.revision);
    a.client_operation_id = "concurrent-edit-a".into();
    let mut b = a.clone();
    b.model.display_name = "Other editor".into();
    b.client_operation_id = "concurrent-edit-b".into();
    let (a, b) = tokio::join!(
        super::model_save(&h.context, a),
        super::model_save(&h.context, b)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert_eq!(
        a.err().or_else(|| b.err()).expect("loser").code,
        ApiErrorCode::Conflict
    );
}

#[tokio::test]
async fn remote_evidence_fills_missing_metadata_from_declarations_and_replay_survives_source_delete()
 {
    let h = harness(Reply::Text("Hello."));
    let mut request = draft(&h.context, "remote-save");
    request.model.remote_metadata = Some(dto::RemoteModelContract {
        id: request.model.external_model_id.clone(),
        display_name: None,
        description: None,
        context_length: Some(8192),
        input_modalities: Some(vec!["image".into()]),
        output_modalities: None,
        supported_endpoints: None,
        input_price: None,
        output_price: None,
    });
    let saved = super::model_save(&h.context, request.clone())
        .await
        .expect("remote save");
    assert_eq!(
        saved.config["capabilities"]["evidence"]["source"],
        "provider_reported"
    );
    assert_eq!(saved.config["capabilities"]["context_length"], 8192);
    assert_eq!(
        saved.config["capabilities"]["input_modalities"]["image"],
        "supported"
    );
    assert_eq!(
        saved.config["capabilities"]["output_modalities"]["text"],
        "supported"
    );
    let duplicate = dto::ModelDuplicateRequest {
        model_id: saved.id.clone(),
        display_name: "Copy".into(),
        expected_revision: saved.revision,
        client_operation_id: "duplicate-removed-source".into(),
    };
    let copy = super::model_duplicate(&h.context, duplicate.clone())
        .await
        .expect("copy");
    super::model_delete(
        &h.context,
        dto::ModelDeleteRequest {
            model_id: saved.id,
            expected_revision: saved.revision,
            client_operation_id: "delete-copy-source".into(),
        },
    )
    .await
    .expect("delete");
    assert_eq!(
        super::model_duplicate(&h.context, duplicate)
            .await
            .expect("replay"),
        copy
    );
    assert_eq!(
        super::model_save(&h.context, request)
            .await
            .expect("save replay after removal")
            .config["capabilities"]["context_length"],
        8192
    );
}

#[tokio::test]
async fn declared_scope_edit_removes_supported_image_and_remote_metadata_overrides_echo() {
    let h = harness(Reply::Text("Hello."));
    let mut request = draft(&h.context, "scope-edit-first");
    let first = super::model_save(&h.context, request.clone())
        .await
        .expect("first");
    request.model.id = Some(first.id.clone());
    request.model.config = first.config;
    request.expected_revision = Some(first.revision);
    request.client_operation_id = "scope-edit-second".into();
    request.model.input_scopes = vec![dto::ModelModality::Text];
    let edited = super::model_save(&h.context, request.clone())
        .await
        .expect("edit");
    assert_eq!(
        edited.config["capabilities"]["input_modalities"]["image"],
        "unknown"
    );
    let serialized = serde_json::to_value(&edited).expect("view");
    assert_eq!(serialized["input_scopes"], serde_json::json!(["text"]));
    assert_eq!(serialized["output_scopes"], serde_json::json!(["text"]));
    request.model.config = edited.config;
    request.model.config["capabilities"]["input_modalities"]["image"] =
        serde_json::json!("unsupported");
    request.expected_revision = Some(edited.revision);
    request.client_operation_id = "scope-edit-remote".into();
    request.model.remote_metadata = Some(serde_json::from_value(serde_json::json!({"id":"editor-model","display_name":"Editor model","input_modalities":["text","image"]})).expect("metadata"));
    let remote = super::model_save(&h.context, request)
        .await
        .expect("remote");
    assert_eq!(
        remote.config["capabilities"]["input_modalities"]["image"],
        "supported"
    );
}

#[tokio::test]
async fn model_delete_emits_models_settings_changed() {
    let h = harness(Reply::Text("Hello."));
    let saved = super::model_save(&h.context, draft(&h.context, "delete-settings-save"))
        .await
        .expect("save");
    super::model_delete(
        &h.context,
        dto::ModelDeleteRequest {
            model_id: saved.id,
            expected_revision: saved.revision,
            client_operation_id: "delete-settings".into(),
        },
    )
    .await
    .expect("delete");
    assert!(h.events.events().iter().any(
        |event| matches!(event, dto::ApiEvent::SettingsChanged { section } if section == "models")
    ));
}
