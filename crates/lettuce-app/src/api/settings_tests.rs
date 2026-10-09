use super::tests::{Reply, harness};
use super::{settings_get, settings_update};
use lettuce_contracts::{self as dto, ApiErrorCode, ApiEvent};

#[tokio::test]
async fn settings_update_cas_invalid_and_single_section_event() {
    let h = harness(Reply::Text("Hello."));
    let before = settings_get(&h.context).await.expect("settings");
    let invalid = dto::SettingsUpdateRequest {
        patch: dto::SettingsPatch::General {
            lorebook_scan_depth: Some(0),
            pure_mode: None,
            analytics_enabled: None,
            update_checks_enabled: None,
            developer_mode_enabled: None,
            auto_download_character_card_avatars: None,
            manual_mode_context_window: None,
        },
        expected_revision: before.revision,
    };
    assert_eq!(
        settings_update(&h.context, invalid)
            .await
            .expect_err("invalid")
            .code,
        ApiErrorCode::InvalidInput
    );
    assert_eq!(settings_get(&h.context).await.expect("settings"), before);
    let request = dto::SettingsUpdateRequest {
        patch: dto::SettingsPatch::General {
            lorebook_scan_depth: Some(11),
            pure_mode: Some(dto::SettingsPureMode::Off),
            analytics_enabled: None,
            update_checks_enabled: None,
            developer_mode_enabled: None,
            auto_download_character_card_avatars: None,
            manual_mode_context_window: None,
        },
        expected_revision: before.revision,
    };
    let after = settings_update(&h.context, request.clone())
        .await
        .expect("update");
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(
        h.filter_runtime.content_filter().level(),
        lettuce_inference::content_filter::PureModeLevel::Off
    );
    assert!(std::sync::Arc::ptr_eq(
        h.context.content_filter(),
        &h.filter_runtime.content_filter()
    ));
    assert_eq!(
        settings_update(&h.context, request)
            .await
            .expect_err("stale")
            .code,
        ApiErrorCode::Conflict
    );
    assert_eq!(h.events.events().into_iter().filter(|event| matches!(event, ApiEvent::SettingsChanged { section } if section == "general")).count(), 1);
}

#[tokio::test]
async fn filter_commands_are_gated_and_hits_are_coalesced() {
    let h = harness(Reply::Text("Hello."));
    assert_eq!(
        super::content_filter_log(&h.context)
            .await
            .expect_err("disabled")
            .code,
        ApiErrorCode::Unsupported
    );
    assert_eq!(
        super::content_filter_clear(&h.context)
            .await
            .expect_err("disabled")
            .code,
        ApiErrorCode::Unsupported
    );
    let mut settings = settings_get(&h.context).await.expect("settings");
    settings_update(
        &h.context,
        dto::SettingsUpdateRequest {
            expected_revision: settings.revision,
            patch: dto::SettingsPatch::General {
                developer_mode_enabled: Some(true),
                pure_mode: None,
                analytics_enabled: None,
                update_checks_enabled: None,
                auto_download_character_card_avatars: None,
                manual_mode_context_window: None,
                lorebook_scan_depth: None,
            },
        },
    )
    .await
    .expect("enable debug");
    let events_context = h.context.clone();
    let events_task = tokio::spawn(super::content_filter::run_events(
        events_context,
        std::future::pending(),
    ));
    for index in 0..300 {
        h.filter_runtime
            .content_filter()
            .check_text("decapitate and disembowel", index);
    }
    assert_eq!(
        super::content_filter_log(&h.context)
            .await
            .expect("log")
            .entries
            .len(),
        200
    );
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        h.events.until(|events| {
            events
                .iter()
                .any(|event| matches!(event, ApiEvent::ContentFilterHit))
        }),
    )
    .await
    .expect("hit event");
    assert_eq!(
        h.events
            .events()
            .into_iter()
            .filter(|event| matches!(event, ApiEvent::ContentFilterHit))
            .count(),
        1
    );
    super::content_filter_clear(&h.context)
        .await
        .expect("clear");
    assert!(
        super::content_filter_log(&h.context)
            .await
            .expect("log")
            .entries
            .is_empty()
    );
    settings = settings_get(&h.context).await.expect("settings");
    settings_update(
        &h.context,
        dto::SettingsUpdateRequest {
            expected_revision: settings.revision,
            patch: dto::SettingsPatch::General {
                developer_mode_enabled: Some(false),
                pure_mode: None,
                analytics_enabled: None,
                update_checks_enabled: None,
                auto_download_character_card_avatars: None,
                manual_mode_context_window: None,
                lorebook_scan_depth: None,
            },
        },
    )
    .await
    .expect("disable debug");
    assert_eq!(
        super::content_filter_log(&h.context)
            .await
            .expect_err("disabled")
            .code,
        ApiErrorCode::Unsupported
    );
    assert!(
        h.context
            .content_filter()
            .hit_log()
            .expect("log")
            .is_empty()
    );
    events_task.abort();
}

#[tokio::test]
async fn settings_wire_unknown_and_invalid_values_fail_typed_without_writes() {
    let h = harness(Reply::Text("Hello."));
    let before = settings_get(&h.context).await.expect("settings");
    for patch in [
        serde_json::json!({"section":"general","futureKey":true}),
        serde_json::json!({"section":"general","pure_mode":"bogus"}),
        serde_json::json!({"section":"general","creationHelperEnabled":true}),
        serde_json::json!({"section":"creation_helper"}),
        serde_json::json!({"section":"ui_preferences","changes":[{"key":"customColors","value":{"accent":null}}]}),
    ] {
        let request: dto::SettingsCommandInput<dto::SettingsUpdateRequest> =
            serde_json::from_value(
                serde_json::json!({"patch":patch,"expected_revision":before.revision}),
            )
            .expect("IPC request parsing reaches typed error mapping");
        let error = settings_update(&h.context, request)
            .await
            .expect_err("invalid");
        assert_eq!(error.code, ApiErrorCode::InvalidInput);
        assert!(error.details.is_some());
        assert_eq!(settings_get(&h.context).await.expect("settings"), before);
    }
}

#[tokio::test]
async fn settings_sections_preserve_creation_settings_and_emit_their_section() {
    let h = harness(Reply::Text("Hello."));
    let before = settings_get(&h.context).await.expect("settings");
    let global = before.global.clone();
    let patches = vec![
        (
            "dynamic_memory",
            dto::SettingsPatch::DynamicMemory {
                value: global.dynamic_memory,
            },
        ),
        (
            "group_dynamic_memory",
            dto::SettingsPatch::GroupDynamicMemory {
                value: global.group_dynamic_memory,
            },
        ),
        (
            "dynamic_memory_prompts",
            dto::SettingsPatch::DynamicMemoryPrompts {
                value: global.dynamic_memory_prompts,
            },
        ),
        (
            "dynamic_memory_sampler",
            dto::SettingsPatch::DynamicMemorySampler {
                enabled: global.dynamic_memory_llama_sampler_overwrite_enabled,
            },
        ),
        (
            "help_me_reply",
            dto::SettingsPatch::HelpMeReply {
                value: global.help_me_reply,
            },
        ),
        (
            "lorebook_generator",
            dto::SettingsPatch::LorebookGenerator {
                value: global.lorebook_generator,
            },
        ),
        (
            "lorebook_entry_generator",
            dto::SettingsPatch::LorebookEntryGenerator {
                value: global.lorebook_entry_generator,
            },
        ),
        (
            "companion_soul_writer",
            dto::SettingsPatch::CompanionSoulWriter {
                value: global.companion_soul_writer,
            },
        ),
        (
            "image_generation",
            dto::SettingsPatch::ImageGeneration {
                value: global.image_generation,
            },
        ),
        (
            "embedding",
            dto::SettingsPatch::Embedding {
                value: global.embedding,
            },
        ),
        (
            "device",
            dto::SettingsPatch::DeviceEmbedding {
                model_version: Some(dto::SettingsEmbeddingVersion::V5),
                max_tokens: Some(1024),
                keep_model_loaded: true,
            },
        ),
        (
            "local_runtime",
            dto::SettingsPatch::LocalRuntime {
                context_length: Some(8192),
                kv_cache_type: Some(dto::SettingsLlamaDefaultKvCacheType::Auto),
            },
        ),
        (
            "selections",
            dto::SettingsPatch::Selections {
                default_model: None,
                default_prompt: None,
                dynamic_memory_model: None,
                group_speaker_model: None,
            },
        ),
        (
            "ui_preferences",
            dto::SettingsPatch::UiPreferences {
                changes: vec![dto::UiPreferenceChange::Theme {
                    value: Some(dto::SettingsUiTheme::Dark),
                }],
            },
        ),
    ];
    for (section, patch) in patches {
        let current = settings_get(&h.context).await.expect("settings");
        let updated = settings_update(
            &h.context,
            dto::SettingsUpdateRequest {
                expected_revision: if matches!(&patch, dto::SettingsPatch::DeviceEmbedding { .. }) {
                    current.device.revision
                } else {
                    current.revision
                },
                patch,
            },
        )
        .await
        .expect("update");
        assert_eq!(
            updated.global.creation_helper,
            before.global.creation_helper
        );
        assert_eq!(h.events.events().iter().filter(|event| matches!(event, ApiEvent::SettingsChanged { section: emitted } if emitted == section)).count(), 1);
    }
    let after = settings_get(&h.context).await.expect("settings");
    assert_eq!(after.device.embedding_max_tokens, Some(1024));
    assert_eq!(after.global.ui_preferences["theme"], "dark");
}

#[tokio::test]
async fn settings_concurrent_cas_and_sampler_share_one_revision() {
    let h = harness(Reply::Text("Hello."));
    let before = settings_get(&h.context).await.expect("settings");
    let patch = dto::SettingsPatch::UiPreferences {
        changes: vec![dto::UiPreferenceChange::Theme {
            value: Some(dto::SettingsUiTheme::Light),
        }],
    };
    let request = dto::SettingsUpdateRequest {
        patch,
        expected_revision: before.revision,
    };
    let (first, second) = tokio::join!(
        settings_update(&h.context, request.clone()),
        settings_update(&h.context, request)
    );
    assert!(first.is_ok() ^ second.is_ok());
    assert_eq!(
        first
            .err()
            .or_else(|| second.err())
            .expect("one conflict")
            .code,
        ApiErrorCode::Conflict
    );
    assert_eq!(
        super::settings_sampler_defaults_update(
            &h.context,
            dto::SettingsSamplerDefaultsUpdateRequest {
                value: before.sampler_defaults.clone(),
                expected_revision: before.revision
            }
        )
        .await
        .expect_err("shared revision")
        .code,
        ApiErrorCode::Conflict
    );
    let current = settings_get(&h.context).await.expect("settings");
    let mut invalid = current.sampler_defaults;
    invalid.chat_parameters.temperature = Some(f64::NAN);
    assert_eq!(
        super::settings_sampler_defaults_update(
            &h.context,
            dto::SettingsSamplerDefaultsUpdateRequest {
                value: invalid,
                expected_revision: current.revision
            }
        )
        .await
        .expect_err("finite")
        .code,
        ApiErrorCode::InvalidInput
    );
    assert_eq!(
        settings_get(&h.context).await.expect("settings").revision,
        current.revision
    );
}

#[tokio::test]
async fn settings_sampler_wire_validation_and_commit_event() {
    let h = harness(Reply::Text("Hello."));
    let before = settings_get(&h.context).await.expect("settings");
    let mut wire = serde_json::to_value(&before.sampler_defaults).expect("layer");
    wire["chat_parameters"]["unknown"] = serde_json::json!(true);
    let request: dto::SettingsCommandInput<dto::SettingsSamplerDefaultsUpdateRequest> =
        serde_json::from_value(
            serde_json::json!({"value":wire,"expected_revision":before.revision}),
        )
        .expect("IPC");
    assert_eq!(
        super::settings_sampler_defaults_update(&h.context, request)
            .await
            .expect_err("unknown")
            .code,
        ApiErrorCode::InvalidInput
    );
    assert_eq!(settings_get(&h.context).await.expect("settings"), before);
    let mut value = before.sampler_defaults;
    value.chat_parameters.max_output_tokens = Some(3000);
    let request = dto::SettingsSamplerDefaultsUpdateRequest {
        value,
        expected_revision: before.revision,
    };
    let after = super::settings_sampler_defaults_update(&h.context, request.clone())
        .await
        .expect("save");
    assert_eq!(
        after.sampler_defaults.chat_parameters.max_output_tokens,
        Some(3000)
    );
    assert_eq!(
        super::settings_sampler_defaults_update(&h.context, request)
            .await
            .expect_err("retry")
            .code,
        ApiErrorCode::Conflict
    );
    assert_eq!(h.events.events().iter().filter(|event| matches!(event, ApiEvent::SettingsChanged { section } if section == "sampler_defaults")).count(), 1);
}

#[tokio::test]
async fn settings_missing_selection_and_blocked_enable_preserve_snapshot() {
    let h = harness(Reply::Text("Hello."));
    let before = settings_get(&h.context).await.expect("settings");
    let patches = [
        dto::SettingsPatch::Selections {
            default_model: Some(dto::IdChange::Set {
                id: lettuce_types::ModelProfileId::new().to_string(),
            }),
            default_prompt: None,
            dynamic_memory_model: None,
            group_speaker_model: None,
        },
        dto::SettingsPatch::Selections {
            default_model: None,
            default_prompt: Some(dto::IdChange::Set {
                id: lettuce_types::PromptDocumentId::new().to_string(),
            }),
            dynamic_memory_model: None,
            group_speaker_model: None,
        },
    ];
    for patch in patches {
        let error = settings_update(
            &h.context,
            dto::SettingsUpdateRequest {
                patch,
                expected_revision: before.revision,
            },
        )
        .await
        .expect_err("missing reference");
        assert!(error.details.is_some());
        assert_eq!(settings_get(&h.context).await.expect("settings"), before);
    }
    let mut value = before.global.dynamic_memory.clone();
    value.enabled = true;
    assert_eq!(
        settings_update(
            &h.context,
            dto::SettingsUpdateRequest {
                patch: dto::SettingsPatch::DynamicMemory { value },
                expected_revision: before.revision
            }
        )
        .await
        .expect_err("embedding required")
        .code,
        ApiErrorCode::ModelRequired
    );
    assert_eq!(settings_get(&h.context).await.expect("settings"), before);
    assert!(h.events.events().is_empty());
}

#[tokio::test]
async fn settings_preserve_large_preset_collection() {
    let h = harness(Reply::Text("Hello."));
    let before = settings_get(&h.context).await.expect("settings");
    let presets = (0..5000)
        .map(|index| dto::SettingsUiLlamaSamplerPreset {
            id: Some(index.to_string()),
            name: Some("Saved sampler".to_owned()),
            stages: Some(vec![
                dto::SettingsLlamaSamplerStage::Penalties,
                dto::SettingsLlamaSamplerStage::TopK,
                dto::SettingsLlamaSamplerStage::TopP,
                dto::SettingsLlamaSamplerStage::MinP,
                dto::SettingsLlamaSamplerStage::Temp,
            ]),
        })
        .collect();
    settings_update(
        &h.context,
        dto::SettingsUpdateRequest {
            patch: dto::SettingsPatch::UiPreferences {
                changes: vec![dto::UiPreferenceChange::LlamaSamplerPresets {
                    value: Some(presets),
                }],
            },
            expected_revision: before.revision,
        },
    )
    .await
    .expect("save collection");
    let after = settings_get(&h.context).await.expect("settings");
    assert_eq!(
        after.global.ui_preferences["llamaSamplerPresets"]
            .as_array()
            .expect("presets")
            .len(),
        5000
    );
}

#[tokio::test]
async fn settings_sampler_preset_names_keep_legacy_trim_and_unicode_length() {
    let h = harness(Reply::Text("Hello."));
    let before = settings_get(&h.context).await.expect("settings");
    let name = "ş".repeat(64);
    let preset = dto::SettingsUiLlamaSamplerPreset {
        id: Some(" id ".to_owned()),
        name: Some(format!("\u{feff} {name} \u{feff}")),
        stages: Some(vec![dto::SettingsLlamaSamplerStage::Temp]),
    };
    let after = settings_update(
        &h.context,
        dto::SettingsUpdateRequest {
            patch: dto::SettingsPatch::UiPreferences {
                changes: vec![dto::UiPreferenceChange::LlamaSamplerPresets {
                    value: Some(vec![preset]),
                }],
            },
            expected_revision: before.revision,
        },
    )
    .await
    .expect("valid Unicode preset");
    assert_eq!(
        after.global.ui_preferences["llamaSamplerPresets"][0]["name"],
        name
    );
    assert_eq!(
        after.global.ui_preferences["llamaSamplerPresets"][0]["id"],
        "id"
    );
}

#[tokio::test]
async fn group_memory_enable_requires_embedding_without_writes() {
    let h = harness(Reply::Text("Hello."));
    let before = settings_get(&h.context).await.expect("settings");
    let mut value = before.global.dynamic_memory.clone();
    value.enabled = true;
    let error = settings_update(
        &h.context,
        dto::SettingsUpdateRequest {
            expected_revision: before.revision,
            patch: dto::SettingsPatch::GroupDynamicMemory { value: Some(value) },
        },
    )
    .await
    .expect_err("embedding required");
    assert_eq!(error.code, ApiErrorCode::ModelRequired);
    assert_eq!(
        error.details,
        Some(dto::ApiErrorDetails::Model {
            model: dto::RequiredModel::Embedding
        })
    );
    assert_eq!(settings_get(&h.context).await.expect("unchanged"), before);
}

#[tokio::test]
async fn sync_settings_commit_emits_and_refreshes_shared_filter() {
    use lettuce_settings::GlobalSettingsStore;
    let h = harness(Reply::Text("Hello."));
    let worker = tokio::spawn(super::content_filter::run_events(
        h.context.clone(),
        std::future::pending(),
    ));
    use lettuce_sync::{IncomingChangeRepository, LocalChangeJournal};
    let database = h.context.backend().database();
    let source = lettuce_database::Database::open_in_memory().expect("peer");
    let mut stored = source.load().expect("settings");
    stored.settings.pure_mode = lettuce_settings::PureMode::Off;
    source
        .save(
            stored.settings,
            stored.default_model_profile_id,
            stored.revision,
        )
        .expect("peer commit");
    source
        .journal_current_state(h.context.now())
        .expect("source journal");
    database
        .journal_current_state(h.context.now())
        .expect("target journal");
    let batch = source
        .outbound_changes(
            &database.local_frontier().expect("frontier"),
            lettuce_sync::MAX_OUTBOUND_CHANGES,
            lettuce_sync::MAX_OUTBOUND_PAYLOAD_BYTES,
        )
        .expect("batch");
    let id = lettuce_types::OperationId::new();
    database
        .stage_incoming_batch(
            lettuce_sync::SyncDeviceId::new(),
            id,
            &lettuce_sync::canonical_batch_hash(&batch.changes),
            &batch.changes,
            h.context.now(),
        )
        .expect("stage");
    assert_eq!(
        database
            .apply_incoming_batch(id, h.context.now())
            .expect("apply")
            .state,
        lettuce_sync::IncomingBatchState::Committed
    );
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        h.events.until(|events| {
            events
                .iter()
                .any(|event| matches!(event, ApiEvent::SettingsChanged { .. }))
        }),
    )
    .await
    .expect("commit event");
    assert_eq!(
        h.filter_runtime.content_filter().level(),
        lettuce_inference::content_filter::PureModeLevel::Off
    );
    assert!(std::sync::Arc::ptr_eq(
        h.context.content_filter(),
        &h.filter_runtime.content_filter()
    ));
    worker.abort();
}

#[tokio::test]
async fn hit_burst_emits_without_a_coalescing_timer() {
    let h = harness(Reply::Text("Hello."));
    h.context
        .content_filter()
        .apply_settings(
            0,
            true,
            lettuce_inference::content_filter::PureModeLevel::Standard,
        )
        .expect("enable");
    for index in 0..10 {
        h.filter_runtime
            .content_filter()
            .check_text("decapitate and disembowel", index);
    }
    let worker = tokio::spawn(super::content_filter::run_events(
        h.context.clone(),
        std::future::pending(),
    ));
    tokio::time::timeout(
        std::time::Duration::from_millis(50),
        h.events.until(|events| {
            events
                .iter()
                .any(|event| matches!(event, ApiEvent::ContentFilterHit))
        }),
    )
    .await
    .expect("no timer");
    worker.abort();
}

#[test]
fn committed_result_survives_filter_refresh_failure() {
    let result =
        super::settings::finish_committed(42, || Err(super::content_filter::filter_error()));
    assert_eq!(result, 42);
}

struct InstalledEmbedding;

#[async_trait::async_trait]
impl super::ModelLoader for InstalledEmbedding {
    fn installed(&self, _: &super::ApiContext, model: dto::RequiredModel) -> bool {
        model == dto::RequiredModel::Embedding
    }
    async fn prepare(&self, _: &super::ApiContext) -> bool {
        panic!("enable must not load models")
    }
    fn embedding(
        &self,
        _: &super::ApiContext,
    ) -> super::ModelLoad<std::sync::Arc<dyn crate::MemoryEmbeddingEngine>> {
        panic!("enable must not load models")
    }
    fn emotion(
        &self,
        _: &super::ApiContext,
    ) -> super::ModelLoad<std::sync::Arc<dyn crate::CompanionEmotionEngine>> {
        panic!("enable must not load models")
    }
}

#[tokio::test]
async fn memory_enable_seeds_the_default_once_and_group_enable_succeeds() {
    let h = super::tests::harness_in(
        Reply::Text("Hello."),
        std::sync::Arc::new(lettuce_jobs::SystemClock),
        None,
        None,
        std::sync::Arc::new(InstalledEmbedding),
    );
    let before = settings_get(&h.context).await.expect("settings");
    let mut value = before.global.dynamic_memory.clone();
    value.enabled = true;
    let after = settings_update(
        &h.context,
        dto::SettingsUpdateRequest {
            expected_revision: before.revision,
            patch: dto::SettingsPatch::DynamicMemory {
                value: value.clone(),
            },
        },
    )
    .await
    .expect("enable");
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(
        after.dynamic_memory_model_profile_id,
        before.default_model_profile_id
    );
    let after_group = settings_update(
        &h.context,
        dto::SettingsUpdateRequest {
            expected_revision: after.revision,
            patch: dto::SettingsPatch::GroupDynamicMemory { value: Some(value) },
        },
    )
    .await
    .expect("group enable");
    assert_eq!(
        after_group.dynamic_memory_model_profile_id,
        after.dynamic_memory_model_profile_id
    );
    assert!(
        after_group
            .global
            .group_dynamic_memory
            .expect("group")
            .enabled
    );
}

#[tokio::test]
async fn inheriting_enabled_group_memory_requires_embedding() {
    use lettuce_settings::GlobalSettingsStore;
    let h = harness(Reply::Text("Hello."));
    let database = h.context.backend().database();
    let mut stored = database.load().expect("settings");
    stored.settings.dynamic_memory.enabled = true;
    stored.settings.group_dynamic_memory = Some(lettuce_settings::DynamicMemorySettings::default());
    database
        .save(
            stored.settings,
            stored.default_model_profile_id,
            stored.revision,
        )
        .expect("existing policy");
    let before = settings_get(&h.context).await.expect("settings");
    let error = settings_update(
        &h.context,
        dto::SettingsUpdateRequest {
            expected_revision: before.revision,
            patch: dto::SettingsPatch::GroupDynamicMemory { value: None },
        },
    )
    .await
    .expect_err("inherited enable needs embedding");
    assert_eq!(error.code, ApiErrorCode::ModelRequired);
    assert_eq!(settings_get(&h.context).await.expect("unchanged"), before);
}

#[tokio::test]
async fn models_folder_relocation_worker_emits_device_settings_changed() {
    use lettuce_models::ModelPathRelocation;
    use lettuce_settings::DeviceSettingsStore;
    let h = harness(Reply::Text("Hello."));
    let worker = tokio::spawn(super::content_filter::run_events(
        h.context.clone(),
        std::future::pending(),
    ));
    let database = h.context.backend().database();
    let mut device = database.load_device_settings().expect("device");
    device.llm_models_dir = Some("/tmp/s7c-relocated-models".into());
    database
        .relocate_model_paths_and_save_device(&|_| None, device, h.context.now())
        .expect("relocation commit");
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        h.events.until(|events| {
            events.iter().any(|event| {
                matches!(event,
            ApiEvent::SettingsChanged { section } if section == "device")
            })
        }),
    )
    .await
    .expect("device event from worker");
    assert_eq!(
        database
            .load_device_settings()
            .expect("device")
            .llm_models_dir
            .as_deref(),
        Some("/tmp/s7c-relocated-models")
    );
    assert_eq!(
        h.events
            .events()
            .iter()
            .filter(|event| matches!(event, ApiEvent::SettingsChanged { .. }))
            .count(),
        1
    );
    worker.abort();
}
