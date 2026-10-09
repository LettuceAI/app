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
        h.context.content_filter().level(),
        lettuce_inference::content_filter::PureModeLevel::Off
    );
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
        h.context
            .content_filter()
            .check_text("decapitate and disembowel", index);
    }
    assert_eq!(
        super::content_filter_log(&h.context)
            .await
            .expect("log")
            .entries
            .len(),
        300
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
            "device_embedding",
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
                patch,
                expected_revision: current.revision,
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
        .expect_err("undecided enable")
        .code,
        ApiErrorCode::Unsupported
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
