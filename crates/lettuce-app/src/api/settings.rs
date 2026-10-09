use super::{
    ApiContext,
    app::settings_error,
    error::{api_error, invalid_field, parse_id},
};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_models::ModelSettingsLayer;
use lettuce_settings::{DeviceSettings, StoredGlobalSettings};
use serde::{Serialize, de::DeserializeOwned};

type Snapshot = (StoredGlobalSettings, ModelSettingsLayer, DeviceSettings);

fn convert<S: Serialize, T: DeserializeOwned>(value: S, field: &str) -> Result<T, ApiError> {
    serde_json::to_value(value)
        .map_err(|_| invalid_field(field, "setting cannot be represented"))
        .and_then(|value| {
            serde_json::from_value(value)
                .map_err(|_| invalid_field(field, "setting has an invalid value"))
        })
}

fn view((stored, sampler, device): Snapshot) -> Result<dto::SettingsView, ApiError> {
    let mut global = serde_json::to_value(&stored.settings)
        .map_err(|_| api_error(ApiErrorCode::Internal, "settings serialization failed"))?;
    global
        .as_object_mut()
        .ok_or_else(|| api_error(ApiErrorCode::Internal, "settings are not an object"))?
        .entry("ui_preferences")
        .or_insert_with(|| serde_json::json!({}));
    Ok(dto::SettingsView {
        global: convert(global, "global")?,
        sampler_defaults: convert(sampler, "sampler_defaults")?,
        device: dto::SettingsDeviceView {
            embedding_model_version: device.embedding.model_version.map(|version| match version {
                lettuce_settings::EmbeddingModelVersion::V3 => dto::SettingsEmbeddingVersion::V3,
                lettuce_settings::EmbeddingModelVersion::V4 => dto::SettingsEmbeddingVersion::V4,
                lettuce_settings::EmbeddingModelVersion::V5 => dto::SettingsEmbeddingVersion::V5,
            }),
            embedding_max_tokens: device.embedding.max_tokens,
            embedding_keep_model_loaded: device.embedding.keep_model_loaded,
            llm_models_dir: device.llm_models_dir,
            dictation_model_id: device.speech.dictation_model_id,
            trusted_certificates: super::providers::certificate_view(
                device.trusted_certificates,
                stored.revision,
            )
            .certificates,
        },
        default_model_profile_id: stored.default_model_profile_id.map(|id| id.to_string()),
        default_prompt_document_id: stored.default_prompt_document_id.map(|id| id.to_string()),
        dynamic_memory_model_profile_id: stored
            .dynamic_memory_model_profile_id
            .map(|id| id.to_string()),
        group_speaker_model_profile_id: stored
            .group_speaker_model_profile_id
            .map(|id| id.to_string()),
        revision: stored.revision.get(),
    })
}

pub async fn settings_get(context: &ApiContext) -> Result<dto::SettingsView, ApiError> {
    context
        .blocking(|context| {
            view(
                context
                    .backend()
                    .database()
                    .settings_snapshot()
                    .map_err(settings_error)?,
            )
        })
        .await
}

fn selection<T: std::str::FromStr>(
    current: &mut Option<T>,
    value: Option<dto::IdChange>,
    field: &str,
) -> Result<(), ApiError> {
    if let Some(value) = value {
        *current = match value {
            dto::IdChange::Set { id } => Some(parse_id(&id, field)?),
            dto::IdChange::Reset => None,
        };
    }
    Ok(())
}

fn validate_memory(value: &lettuce_settings::DynamicMemorySettings) -> Result<(), ApiError> {
    let scores = [
        value.cold_threshold_basis_points,
        value.delete_confidence_basis_points,
        value.max_hard_delete_ratio_basis_points,
        value.duplicate_threshold_basis_points,
        value.decay_rate_basis_points,
    ];
    if scores.into_iter().any(|score| score > 10000)
        || value
            .min_similarity_basis_points
            .is_some_and(|score| score > 10000)
        || value.summary_message_interval == 0
        || value.retrieval_limit == 0
        || value.max_entries == 0
        || value.hot_memory_token_budget == 0
        || !(1..=100).contains(&value.recursive_memory_loop_hard_cap)
    {
        return Err(invalid_field(
            "dynamic_memory",
            "memory policy is outside its valid range",
        ));
    }
    Ok(())
}

fn apply(
    stored: &mut StoredGlobalSettings,
    patch: dto::SettingsPatch,
    device_embedding: &mut Option<lettuce_settings::DeviceEmbeddingSettings>,
) -> Result<&'static str, ApiError> {
    let settings = &mut stored.settings;
    let section = match patch {
        dto::SettingsPatch::General {
            pure_mode,
            analytics_enabled,
            update_checks_enabled,
            developer_mode_enabled,
            auto_download_character_card_avatars,
            manual_mode_context_window,
            lorebook_scan_depth,
        } => {
            if let Some(value) = pure_mode {
                settings.pure_mode = convert(value, "pure_mode")?;
            }
            if let Some(value) = analytics_enabled {
                settings.analytics_enabled = value;
            }
            if let Some(value) = update_checks_enabled {
                settings.update_checks_enabled = value;
            }
            if let Some(value) = developer_mode_enabled {
                settings.developer_mode_enabled = value;
            }
            if let Some(value) = auto_download_character_card_avatars {
                settings.auto_download_character_card_avatars = value;
            }
            if let Some(value) = manual_mode_context_window {
                if value == 0 {
                    return Err(invalid_field(
                        "manual_mode_context_window",
                        "window must be positive",
                    ));
                }
                settings.manual_mode_context_window = value;
            }
            if let Some(value) = lorebook_scan_depth {
                if !lettuce_settings::LOREBOOK_SCAN_DEPTH_RANGE.contains(&value) {
                    return Err(invalid_field(
                        "lorebook_scan_depth",
                        "scan depth is outside its valid range",
                    ));
                }
                settings.lorebook_scan_depth = value;
            }
            "general"
        }
        dto::SettingsPatch::DynamicMemory { value } => {
            let value: lettuce_settings::DynamicMemorySettings = convert(value, "dynamic_memory")?;
            validate_memory(&value)?;
            if value.enabled && !settings.dynamic_memory.enabled {
                return Err(api_error(
                    ApiErrorCode::Unsupported,
                    "dynamic memory enable behavior awaits slice 7 Q16 clarification",
                ));
            }
            settings.dynamic_memory = value;
            "dynamic_memory"
        }
        dto::SettingsPatch::GroupDynamicMemory { value } => {
            let value: Option<lettuce_settings::DynamicMemorySettings> =
                convert(value, "group_dynamic_memory")?;
            if let Some(value) = &value {
                validate_memory(value)?;
            }
            settings.group_dynamic_memory = value;
            "group_dynamic_memory"
        }
        dto::SettingsPatch::DynamicMemoryPrompts { value } => {
            settings.dynamic_memory_prompts = convert(value, "dynamic_memory_prompts")?;
            "dynamic_memory_prompts"
        }
        dto::SettingsPatch::DynamicMemorySampler { enabled } => {
            settings.dynamic_memory_llama_sampler_overwrite_enabled = enabled;
            "dynamic_memory_sampler"
        }
        dto::SettingsPatch::HelpMeReply { value } => {
            if value.max_output_tokens == 0 {
                return Err(invalid_field(
                    "help_me_reply.max_output_tokens",
                    "output cap must be positive",
                ));
            }
            settings.help_me_reply = convert(value, "help_me_reply")?;
            "help_me_reply"
        }
        dto::SettingsPatch::LorebookGenerator { value } => {
            if value
                .default_target_count
                .is_some_and(|count| !(5..=50).contains(&count))
                || value
                    .max_output_tokens
                    .is_some_and(|count| !(256..=32768).contains(&count))
            {
                return Err(invalid_field(
                    "lorebook_generator",
                    "generator choice is outside its valid range",
                ));
            }
            settings.lorebook_generator = convert(value, "lorebook_generator")?;
            "lorebook_generator"
        }
        dto::SettingsPatch::LorebookEntryGenerator { value } => {
            settings.lorebook_entry_generator = convert(value, "lorebook_entry_generator")?;
            "lorebook_entry_generator"
        }
        dto::SettingsPatch::CompanionSoulWriter { value } => {
            settings.companion_soul_writer = convert(value, "companion_soul_writer")?;
            "companion_soul_writer"
        }
        dto::SettingsPatch::ImageGeneration { value } => {
            settings.image_generation = convert(value, "image_generation")?;
            "image_generation"
        }
        dto::SettingsPatch::Embedding { value } => {
            if value
                .dimensions
                .is_some_and(|dimensions| ![64, 128, 256, 512, 768].contains(&dimensions))
            {
                return Err(invalid_field(
                    "embedding.dimensions",
                    "embedding dimension is unsupported",
                ));
            }
            settings.embedding = convert(value, "embedding")?;
            "embedding"
        }
        dto::SettingsPatch::DeviceEmbedding {
            model_version,
            max_tokens,
            keep_model_loaded,
        } => {
            if max_tokens.is_some_and(|tokens| !(512..=4096).contains(&tokens)) {
                return Err(invalid_field(
                    "device_embedding.max_tokens",
                    "token window is outside its valid range",
                ));
            }
            *device_embedding = Some(lettuce_settings::DeviceEmbeddingSettings {
                model_version: convert(model_version, "device_embedding.model_version")?,
                max_tokens,
                keep_model_loaded,
            });
            "device_embedding"
        }
        dto::SettingsPatch::LocalRuntime {
            context_length,
            kv_cache_type,
        } => {
            settings.llama_default_context_length = context_length;
            settings.llama_default_kv_cache_type = convert(kv_cache_type, "kv_cache_type")?;
            "local_runtime"
        }
        dto::SettingsPatch::UiPreferences { changes } => {
            for mut change in changes {
                if let dto::UiPreferenceChange::LlamaSamplerPresets {
                    value: Some(presets),
                } = &mut change
                {
                    for preset in presets {
                        for value in [&mut preset.id, &mut preset.name].into_iter().flatten() {
                            *value = trim_preset_text(value).to_owned();
                        }
                    }
                }
                validate_ui_change(&change)?;
                let change = serde_json::to_value(change).map_err(|_| {
                    invalid_field("ui_preferences", "UI choice cannot be represented")
                })?;
                let key = change["key"]
                    .as_str()
                    .ok_or_else(|| invalid_field("ui_preferences", "UI key is missing"))?;
                let value = &change["value"];
                if value.is_null() {
                    settings.ui_preferences.0.remove(key);
                } else {
                    settings
                        .ui_preferences
                        .0
                        .insert(key.to_owned(), value.clone());
                }
            }
            "ui_preferences"
        }
        dto::SettingsPatch::Selections {
            default_model,
            default_prompt,
            dynamic_memory_model,
            group_speaker_model,
        } => {
            selection(
                &mut stored.default_model_profile_id,
                default_model,
                "default_model",
            )?;
            selection(
                &mut stored.default_prompt_document_id,
                default_prompt,
                "default_prompt",
            )?;
            selection(
                &mut stored.dynamic_memory_model_profile_id,
                dynamic_memory_model,
                "dynamic_memory_model",
            )?;
            selection(
                &mut stored.group_speaker_model_profile_id,
                group_speaker_model,
                "group_speaker_model",
            )?;
            "selections"
        }
    };
    if !settings.within_bounds() {
        return Err(invalid_field(section, "setting is outside its valid range"));
    }
    Ok(section)
}

pub async fn settings_update(
    context: &ApiContext,
    request: impl Into<dto::SettingsCommandInput<dto::SettingsUpdateRequest>>,
) -> Result<dto::SettingsView, ApiError> {
    let request = request
        .into()
        .into_result()
        .map_err(|error| invalid_field("request", &error))?;
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let (mut stored, sampler, _) = database.settings_snapshot().map_err(settings_error)?;
            if stored.revision.get() != request.expected_revision {
                return Err(api_error(
                    ApiErrorCode::Conflict,
                    "settings revision is stale",
                ));
            }
            let mut device_embedding = None;
            let section = apply(&mut stored, request.patch, &mut device_embedding)?;
            let result = database
                .save_settings_snapshot(stored, sampler, device_embedding, context.now())
                .map_err(settings_error)?;
            context.emit(dto::ApiEvent::SettingsChanged {
                section: section.to_owned(),
            });
            super::content_filter::refresh_logging(context)?;
            let result = view(result)?;
            Ok(result)
        })
        .await
}

pub async fn settings_sampler_defaults_update(
    context: &ApiContext,
    request: impl Into<dto::SettingsCommandInput<dto::SettingsSamplerDefaultsUpdateRequest>>,
) -> Result<dto::SettingsView, ApiError> {
    let request = request
        .into()
        .into_result()
        .map_err(|error| invalid_field("request", &error))?;
    validate_sampler_finite(&request.value)?;
    let sampler: ModelSettingsLayer = convert(request.value, "sampler_defaults")?;
    sampler
        .validate()
        .map_err(|error| invalid_field("sampler_defaults", error.to_string()))?;
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let (stored, _, _) = database.settings_snapshot().map_err(settings_error)?;
            if stored.revision.get() != request.expected_revision {
                return Err(api_error(
                    ApiErrorCode::Conflict,
                    "settings revision is stale",
                ));
            }
            let result = view(
                database
                    .save_settings_snapshot(stored, sampler, None, context.now())
                    .map_err(settings_error)?,
            )?;
            context.emit(dto::ApiEvent::SettingsChanged {
                section: "sampler_defaults".to_owned(),
            });
            Ok(result)
        })
        .await
}

fn finite(values: impl IntoIterator<Item = Option<f64>>, field: &str) -> Result<(), ApiError> {
    if values.into_iter().flatten().any(|value| !value.is_finite()) {
        Err(invalid_field(field, "number must be finite"))
    } else {
        Ok(())
    }
}

fn validate_sampler_finite(value: &dto::SettingsModelSettingsLayer) -> Result<(), ApiError> {
    finite(
        [
            value.chat_parameters.temperature,
            value.chat_parameters.top_p,
            value.chat_parameters.frequency_penalty,
            value.chat_parameters.presence_penalty,
            value.chat_parameters.repetition_penalty,
        ],
        "sampler_defaults",
    )?;
    finite(
        [
            value.chat_parameters.ollama.tfs_z,
            value.chat_parameters.ollama.typical_p,
            value.chat_parameters.ollama.min_p,
            value.chat_parameters.ollama.mirostat_tau,
            value.chat_parameters.ollama.mirostat_eta,
        ],
        "sampler_defaults",
    )?;
    finite(
        [
            value.llama_cpp.sampler.min_p,
            value.llama_cpp.sampler.typical_p,
            value.llama_cpp.sampler.repeat_penalty,
            value.llama_cpp.sampler.dry_multiplier,
            value.llama_cpp.sampler.dry_base,
            value.llama_cpp.sampler.xtc_probability,
            value.llama_cpp.sampler.xtc_threshold,
            value.llama_cpp.sampler.adaptive_target,
            value.llama_cpp.sampler.adaptive_decay,
        ],
        "sampler_defaults",
    )?;
    finite(
        [
            value.llama_cpp.rope_freq_base,
            value.llama_cpp.rope_freq_scale,
            value.llama_cpp.dflash_min_probability,
        ],
        "sampler_defaults",
    )?;
    finite(
        [
            value.stable_diffusion.cfg_scale,
            value.stable_diffusion.denoising_strength,
            value.stable_diffusion.image_cfg_scale,
            value.stable_diffusion.distilled_guidance,
            value.stable_diffusion.eta,
            value.stable_diffusion.flow_shift,
            value.stable_diffusion.vae_tile_overlap,
            value.stable_diffusion.hires_scale,
            value.stable_diffusion.hires_denoising_strength,
            value.stable_diffusion.slg_scale,
            value.stable_diffusion.slg_layer_start,
            value.stable_diffusion.slg_layer_end,
        ],
        "sampler_defaults",
    )?;
    for lora in value.stable_diffusion.base_loras.iter().flatten() {
        finite([Some(lora.multiplier)], "sampler_defaults.base_loras")?;
    }
    Ok(())
}

fn trim_preset_text(value: &str) -> &str {
    value.trim_matches(|character| matches!(character, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'))
}

fn validate_ui_change(change: &dto::UiPreferenceChange) -> Result<(), ApiError> {
    use dto::UiPreferenceChange;
    match change {
        UiPreferenceChange::SettingsCardOpacity { value } => {
            if value.is_some_and(|value| {
                !value.is_finite() || value.fract() != 0.0 || !(0.0..=100.0).contains(&value)
            }) {
                return Err(invalid_field(
                    "settingsCardOpacity",
                    "opacity must be an integer from 0 to 100",
                ));
            }
        }
        UiPreferenceChange::CustomColorPresets {
            value: Some(presets),
        } => {
            for preset in presets {
                let id = preset.id.as_deref().ok_or_else(|| {
                    invalid_field("customColorPresets.id", "preset id is required")
                })?;
                let _: uuid::Uuid = parse_id(id, "customColorPresets.id")?;
                if preset
                    .name
                    .as_deref()
                    .is_none_or(|name| name.trim().is_empty())
                    || preset.colors.is_none()
                    || preset
                        .created_at
                        .is_none_or(|time| !time.is_finite() || time < 0.0 || time.fract() != 0.0)
                {
                    return Err(invalid_field("customColorPresets", "preset is invalid"));
                }
                if preset.settings_card_opacity.is_some_and(|value| {
                    !value.is_finite() || value.fract() != 0.0 || !(0.0..=100.0).contains(&value)
                }) {
                    return Err(invalid_field(
                        "customColorPresets.settingsCardOpacity",
                        "opacity is invalid",
                    ));
                }
            }
        }
        UiPreferenceChange::Accessibility { value: Some(value) } => {
            for sound in [&value.send, &value.success, &value.failure]
                .into_iter()
                .flatten()
            {
                if sound
                    .volume
                    .is_some_and(|volume| !volume.is_finite() || !(0.0..=1.0).contains(&volume))
                {
                    return Err(invalid_field(
                        "accessibility.volume",
                        "sound volume must be from 0 to 1",
                    ));
                }
            }
        }
        UiPreferenceChange::ChatAppearance { value: Some(value) } => {
            for (field, value, low, high) in [
                (
                    "chatColumnWidthPx",
                    value.chat_column_width_px,
                    400.0,
                    2400.0,
                ),
                ("backgroundDim", value.background_dim, 0.0, 80.0),
                ("backgroundBlur", value.background_blur, 0.0, 20.0),
                ("bubbleOpacity", value.bubble_opacity, 20.0, 100.0),
            ] {
                if value.is_some_and(|value| !value.is_finite() || !(low..=high).contains(&value)) {
                    return Err(invalid_field(
                        field,
                        "appearance value is outside its valid range",
                    ));
                }
            }
            if value
                .chat_column_width_px
                .is_some_and(|width| width.fract() != 0.0)
            {
                return Err(invalid_field(
                    "chatColumnWidthPx",
                    "column width must be an integer",
                ));
            }
            if let Some(slots) = &value.chat_widget_slots {
                for widget in slots
                    .left
                    .iter()
                    .flatten()
                    .chain(slots.right.iter().flatten())
                {
                    validate_widget(widget)?;
                }
            }
        }
        UiPreferenceChange::LlamaSamplerPresets {
            value: Some(presets),
        } => {
            for preset in presets {
                if preset
                    .id
                    .as_deref()
                    .is_none_or(|value| value.is_empty() || value.encode_utf16().count() > 128)
                    || preset
                        .name
                        .as_deref()
                        .is_none_or(|value| value.is_empty() || value.encode_utf16().count() > 64)
                    || preset.stages.is_none()
                {
                    return Err(invalid_field(
                        "llamaSamplerPresets",
                        "preset requires id, name and stages",
                    ));
                }
            }
        }
        UiPreferenceChange::NavItems { value: Some(items) } if items.is_empty() => {
            return Err(invalid_field("navItems", "navigation needs an item"));
        }
        _ => {}
    }
    Ok(())
}

fn validate_widget(widget: &dto::SettingsWidgetNode) -> Result<(), ApiError> {
    match widget {
        dto::SettingsWidgetNode::Box { children, .. } => {
            for child in children {
                validate_widget(child)?;
            }
        }
        dto::SettingsWidgetNode::StatTracker { stats, .. } => {
            for stat in stats {
                finite(
                    [Some(stat.value), stat.min, stat.max],
                    "chatAppearance.stats",
                )?;
                if stat.min.zip(stat.max).is_some_and(|(min, max)| max < min) {
                    return Err(invalid_field(
                        "chatAppearance.stats",
                        "stat bounds are reversed",
                    ));
                }
            }
        }
        dto::SettingsWidgetNode::Memory { limit, .. } => {
            finite([*limit], "chatAppearance.memory.limit")?;
        }
        _ => {}
    }
    Ok(())
}
