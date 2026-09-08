use std::{collections::BTreeMap, path::Path, str::FromStr};

use lettuce_models::{
    CapabilityEvidence, CapabilityEvidenceSource, CapabilityStatus, ChatParameterProfile,
    CustomAuth, CustomModelList, CustomProviderConfig, CustomRoles, CustomToolChoiceMode, JsonPath,
    ModalityCapabilities, ModelCapabilities, ModelKind, ModelProfileConfig, OllamaOptions,
    OpenRouterOptions, ParameterSupport, PromptCacheRetention, PromptCaching, ProviderAccount,
    ProviderConfig, ProviderProtocol, QueryParameterName, ReasoningEffort, ReasoningMode,
    SecretHeader, WireRole, validate_provider_connection,
};
use lettuce_settings::{HeaderName, SecretOwnerId, SecretRef};
use lettuce_transfer::{
    LEGACY_DATABASE_SCHEMA_VERSION, LEGACY_LOREBOOK_ENTRIES_PER_BOOK_LIMIT,
    LEGACY_LOREBOOK_ENTRY_PLAN_LIMIT, LEGACY_LOREBOOK_PLAN_LIMIT, LEGACY_MODEL_PROFILE_PLAN_LIMIT,
    LEGACY_PERSONA_PLAN_LIMIT, LEGACY_PROVIDER_ACCOUNT_PLAN_LIMIT, LegacyCrop,
    LegacyDatabaseInventory, LegacyDatabasePreflightError, LegacyImageRecommendation,
    LegacyKeywordMatchMode, LegacyLorebookCandidate, LegacyLorebookDetectionPolicy,
    LegacyLorebookEntryCandidate, LegacyLorebookPlan, LegacyMediaReference,
    LegacyModelProfileCandidate, LegacyPendingProviderSecret, LegacyPersonaCandidate,
    LegacyPersonaPlan, LegacyProviderAccountCandidate, LegacyProviderModelPlan,
};
use lettuce_types::{
    LorebookEntryId, LorebookId, ModelProfileId, PersonaId, ProviderAccountId, Revision,
    TimestampMillis,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::{Map, Value};

const ROOT_TABLES: [(&str, &str); 10] = [
    ("provider_credentials", "provider_accounts"),
    ("models", "models"),
    ("prompt_templates", "prompts"),
    ("personas", "personas"),
    ("characters", "characters"),
    ("lorebooks", "lorebooks"),
    ("chat_templates", "chat_templates"),
    ("sessions", "direct_conversations"),
    ("group_characters", "group_profiles"),
    ("group_sessions", "group_conversations"),
];

pub fn preflight_legacy_database(
    path: impl AsRef<Path>,
) -> Result<LegacyDatabaseInventory, LegacyDatabasePreflightError> {
    let connection = open_validated(path)?;
    let counts = ROOT_TABLES
        .map(|(table, label)| count(&connection, table, label))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LegacyDatabaseInventory {
        schema_version: LEGACY_DATABASE_SCHEMA_VERSION,
        provider_accounts: counts[0],
        models: counts[1],
        prompts: counts[2],
        personas: counts[3],
        characters: counts[4],
        lorebooks: counts[5],
        chat_templates: counts[6],
        direct_conversations: counts[7],
        group_profiles: counts[8],
        group_conversations: counts[9],
    })
}

pub fn plan_legacy_personas(
    path: impl AsRef<Path>,
) -> Result<LegacyPersonaPlan, LegacyDatabasePreflightError> {
    let connection = open_validated(path)?;
    plan_legacy_personas_with_limit(&connection, LEGACY_PERSONA_PLAN_LIMIT)
}

pub fn plan_legacy_lorebooks(
    path: impl AsRef<Path>,
) -> Result<LegacyLorebookPlan, LegacyDatabasePreflightError> {
    let connection = open_validated(path)?;
    require_table(&connection, "lorebook_entries")?;
    plan_legacy_lorebooks_with_limits(
        &connection,
        LEGACY_LOREBOOK_PLAN_LIMIT,
        LEGACY_LOREBOOK_ENTRY_PLAN_LIMIT,
        LEGACY_LOREBOOK_ENTRIES_PER_BOOK_LIMIT,
    )
}

pub fn plan_legacy_provider_models(
    path: impl AsRef<Path>,
) -> Result<LegacyProviderModelPlan, LegacyDatabasePreflightError> {
    let connection = open_validated(path)?;
    plan_legacy_provider_models_with_limits(
        &connection,
        LEGACY_PROVIDER_ACCOUNT_PLAN_LIMIT,
        LEGACY_MODEL_PROFILE_PLAN_LIMIT,
    )
}

fn plan_legacy_provider_models_with_limits(
    connection: &Connection,
    provider_limit: u32,
    model_limit: u32,
) -> Result<LegacyProviderModelPlan, LegacyDatabasePreflightError> {
    require_count_limit(connection, "provider_credentials", provider_limit)?;
    require_count_limit(connection, "models", model_limit)?;
    let (default_provider_id, default_model_id, created_at, updated_at) = connection
        .query_row(
            "SELECT default_provider_credential_id,default_model_id,created_at,updated_at FROM settings WHERE id = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    if updated_at < created_at {
        return Err(provider_malformed("timestamps"));
    }
    let default_provider_account_id = default_provider_id
        .as_deref()
        .map(ProviderAccountId::from_str)
        .transpose()
        .map_err(|_| provider_malformed("default_provider_credential_id"))?;
    let default_model_profile_id = default_model_id
        .as_deref()
        .map(ModelProfileId::from_str)
        .transpose()
        .map_err(|_| model_malformed("default_model_id"))?;

    let mut statement = connection
        .prepare(
            "SELECT id,provider_id,label,CASE WHEN api_key IS NOT NULL AND trim(api_key) <> '' THEN 1 ELSE 0 END,base_url,default_model,config FROM provider_credentials ORDER BY provider_id COLLATE NOCASE ASC,label COLLATE NOCASE ASC,id ASC",
        )
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    let mut provider_accounts = Vec::new();
    for row in rows {
        let (
            source_id,
            provider_kind,
            label,
            api_key_present,
            endpoint,
            default_model,
            config_json,
        ) = row.map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
        let id = ProviderAccountId::from_str(&source_id).map_err(|_| provider_malformed("id"))?;
        require_provider_non_blank(&provider_kind, "provider_id")?;
        require_provider_non_blank(&label, "label")?;
        let protocol = legacy_provider_protocol(&provider_kind)
            .ok_or_else(|| provider_malformed("provider_id"))?;
        let endpoint = normalized_optional(endpoint);
        let default_model = normalized_optional(default_model);
        let config_value = parse_optional_object(config_json, "config", provider_malformed)?;
        let streaming_enabled = optional_bool(&config_value, "streamingEnabled", true)?;
        let allow_invalid_tls = optional_bool(&config_value, "allowInvalidTls", false)?;
        let (config, mapped_config_fields) = legacy_provider_config(&provider_kind, &config_value)?;
        let mut pending_secrets = Vec::new();
        if api_key_present == 1 {
            pending_secrets.push(LegacyPendingProviderSecret::ApiKey);
        } else if api_key_present != 0 {
            return Err(provider_malformed("api_key"));
        }
        pending_secrets.extend(read_pending_headers(connection, &source_id)?);
        let deferred_config_fields = deferred_fields(&config_value, &mapped_config_fields);
        let secret_headers = pending_secrets
            .iter()
            .filter_map(|secret| match secret {
                LegacyPendingProviderSecret::Header { name } => Some(SecretHeader {
                    name: name.clone(),
                    secret_ref: SecretRef::new(),
                }),
                LegacyPendingProviderSecret::ApiKey => None,
            })
            .collect();
        validate_provider_connection(&ProviderAccount {
            id,
            secret_owner_id: SecretOwnerId::from_uuid(id.as_uuid()),
            provider_kind: provider_kind.clone(),
            protocol,
            label: label.clone(),
            endpoint: endpoint.clone(),
            enabled: true,
            streaming_enabled,
            allow_invalid_tls,
            api_key_ref: Some(SecretRef::from_uuid(id.as_uuid())),
            secret_headers,
            config: config.clone(),
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(created_at),
            updated_at: TimestampMillis::new(updated_at),
        })
        .map_err(|_| provider_malformed("connection"))?;
        provider_accounts.push(LegacyProviderAccountCandidate {
            id,
            secret_owner_id: SecretOwnerId::from_uuid(id.as_uuid()),
            provider_kind,
            protocol,
            label,
            endpoint,
            enabled: true,
            streaming_enabled,
            allow_invalid_tls,
            default_model,
            config,
            pending_secrets,
            deferred_config_fields,
            created_at: TimestampMillis::new(created_at),
            updated_at: TimestampMillis::new(updated_at),
        });
    }
    let has_llama_models: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM models WHERE lower(provider_id)='llamacpp')",
            [],
            |row| row.get(0),
        )
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    if has_llama_models {
        let id = legacy_builtin_llama_account_id();
        if provider_accounts.iter().any(|provider| provider.id == id) {
            return Err(provider_malformed("id"));
        }
        provider_accounts.push(LegacyProviderAccountCandidate {
            id,
            secret_owner_id: SecretOwnerId::from_uuid(id.as_uuid()),
            provider_kind: "llamacpp".to_owned(),
            protocol: ProviderProtocol::LlamaCpp,
            label: "llama.cpp (Local)".to_owned(),
            endpoint: None,
            enabled: true,
            streaming_enabled: true,
            allow_invalid_tls: false,
            default_model: None,
            config: ProviderConfig::Standard,
            pending_secrets: Vec::new(),
            deferred_config_fields: Vec::new(),
            created_at: TimestampMillis::new(created_at),
            updated_at: TimestampMillis::new(updated_at),
        });
        provider_accounts.sort_by(|left, right| {
            left.provider_kind
                .to_ascii_lowercase()
                .cmp(&right.provider_kind.to_ascii_lowercase())
                .then_with(|| {
                    left.label
                        .to_ascii_lowercase()
                        .cmp(&right.label.to_ascii_lowercase())
                })
                .then_with(|| left.id.cmp(&right.id))
        });
    }
    if default_provider_account_id
        .is_some_and(|id| !provider_accounts.iter().any(|provider| provider.id == id))
    {
        return Err(LegacyDatabasePreflightError::OrphanRecord {
            table: "settings.default_provider_credential_id",
            parent_table: "provider_credentials",
        });
    }

    let mut statement = connection
        .prepare(
            "SELECT id,name,provider_id,provider_credential_id,provider_label,display_name,created_at,model_type,input_scopes,output_scopes,advanced_model_settings,prompt_template_id,system_prompt FROM models ORDER BY created_at ASC,id ASC",
        )
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
            ))
        })
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    let mut model_profiles = Vec::new();
    for row in rows {
        let (
            id,
            external_model_id,
            provider_kind,
            explicit_provider_id,
            provider_label,
            display_name,
            created_at,
            model_type,
            input_scopes,
            output_scopes,
            advanced_json,
            prompt_template_id,
            deprecated_system_prompt,
        ) = row.map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
        let id = ModelProfileId::from_str(&id).map_err(|_| model_malformed("id"))?;
        require_model_non_blank(&external_model_id, "name")?;
        require_model_non_blank(&provider_kind, "provider_id")?;
        require_model_non_blank(&provider_label, "provider_label")?;
        require_model_non_blank(&display_name, "display_name")?;
        legacy_provider_protocol(&provider_kind).ok_or_else(|| model_malformed("provider_id"))?;
        let explicit_provider_id = explicit_provider_id
            .as_deref()
            .map(ProviderAccountId::from_str)
            .transpose()
            .map_err(|_| model_malformed("provider_credential_id"))?;
        let provider_account_id = resolve_legacy_provider_account(
            &provider_accounts,
            &provider_kind,
            explicit_provider_id,
            &provider_label,
            &external_model_id,
            default_provider_account_id,
        )?;
        let input_modalities = parse_modalities(input_scopes, "input_scopes")?;
        let output_modalities = parse_modalities(output_scopes, "output_scopes")?;
        let advanced =
            parse_optional_object(advanced_json, "advanced_model_settings", model_malformed)?;
        let (chat_parameters, mapped_advanced_fields) =
            legacy_chat_parameters(&provider_kind, &advanced)?;
        let account = provider_accounts
            .iter()
            .find(|account| account.id == provider_account_id)
            .ok_or(LegacyDatabasePreflightError::OrphanRecord {
                table: "models",
                parent_table: "provider_credentials",
            })?;
        let capabilities = ModelCapabilities {
            format_version: lettuce_models::MODEL_CAPABILITIES_FORMAT_VERSION,
            evidence: CapabilityEvidence {
                source: CapabilityEvidenceSource::UserOverride,
                source_version: 1,
                observed_at: TimestampMillis::new(created_at),
            },
            input_modalities,
            output_modalities,
            streaming: if account.streaming_enabled {
                CapabilityStatus::Supported
            } else {
                CapabilityStatus::Unsupported
            },
            tools: CapabilityStatus::Unknown,
            structured_output: CapabilityStatus::Unknown,
            reasoning: CapabilityStatus::Unknown,
            prompt_cache: CapabilityStatus::Unknown,
            context_length: None,
            max_visible_output_tokens: None,
            max_total_completion_tokens: None,
            parameter_support: ParameterSupport::default(),
        };
        let config = ModelProfileConfig {
            chat_parameters,
            lorebook_generator_parameters: Default::default(),
            capabilities,
        };
        config
            .chat_parameters
            .validate()
            .map_err(|_| model_malformed("advanced_model_settings"))?;
        config
            .capabilities
            .validate()
            .map_err(|_| model_malformed("capabilities"))?;
        let kind = match model_type.as_str() {
            "chat" | "multimodel" => ModelKind::Chat,
            "imagegeneration" => ModelKind::Image,
            _ => return Err(model_malformed("model_type")),
        };
        model_profiles.push(LegacyModelProfileCandidate {
            id,
            provider_account_id,
            source_provider_kind: provider_kind,
            source_provider_label: provider_label,
            external_model_id,
            display_name,
            kind,
            config,
            prompt_template_id: normalized_optional(prompt_template_id),
            deprecated_system_prompt: normalized_optional(deprecated_system_prompt),
            deferred_advanced_fields: deferred_fields(&advanced, &mapped_advanced_fields),
            created_at: TimestampMillis::new(created_at),
        });
    }
    if default_model_profile_id.is_some_and(|id| !model_profiles.iter().any(|model| model.id == id))
    {
        return Err(LegacyDatabasePreflightError::OrphanRecord {
            table: "settings.default_model_id",
            parent_table: "models",
        });
    }
    Ok(LegacyProviderModelPlan {
        provider_accounts,
        model_profiles,
        default_provider_account_id,
        default_model_profile_id,
    })
}

fn open_validated(path: impl AsRef<Path>) -> Result<Connection, LegacyDatabasePreflightError> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| LegacyDatabasePreflightError::Unavailable)?;
    for (table, _) in ROOT_TABLES {
        require_table(&connection, table)?;
    }
    require_table(&connection, "settings")?;
    let version: Option<i64> = connection
        .query_row(
            "SELECT migration_version FROM settings WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    let version = version.ok_or(LegacyDatabasePreflightError::MissingSettings)?;
    if version != i64::from(LEGACY_DATABASE_SCHEMA_VERSION) {
        return Err(LegacyDatabasePreflightError::UnsupportedVersion {
            found: version,
            supported: LEGACY_DATABASE_SCHEMA_VERSION,
        });
    }
    Ok(connection)
}

fn plan_legacy_personas_with_limit(
    connection: &Connection,
    limit: u32,
) -> Result<LegacyPersonaPlan, LegacyDatabasePreflightError> {
    let count = count(connection, "personas", "personas")?;
    if count > u64::from(limit) {
        return Err(LegacyDatabasePreflightError::LimitExceeded {
            table: "personas",
            limit,
        });
    }
    let mut statement = connection
        .prepare(
            "SELECT id,title,description,nickname,avatar_path,avatar_crop_x,avatar_crop_y,avatar_crop_scale,design_description,design_reference_image_ids,active_lorebook_ids,is_default,lora_name,lora_strength,created_at,updated_at FROM personas ORDER BY created_at ASC,id ASC",
        )
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<f64>>(5)?,
                row.get::<_, Option<f64>>(6)?,
                row.get::<_, Option<f64>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, i64>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<f64>>(13)?,
                row.get::<_, i64>(14)?,
                row.get::<_, i64>(15)?,
            ))
        })
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    let mut personas = Vec::with_capacity(count as usize);
    let mut default_persona_id = None;
    for row in rows {
        let (
            id,
            title,
            description,
            nickname,
            avatar_path,
            crop_x,
            crop_y,
            crop_scale,
            design_description,
            design_reference_image_ids,
            active_lorebook_ids,
            is_default,
            lora_name,
            lora_strength,
            created_at,
            updated_at,
        ) = row.map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
        let id = PersonaId::from_str(&id).map_err(|_| malformed("id"))?;
        require_non_blank(&title, "title")?;
        require_non_blank(&description, "description")?;
        let avatar = avatar_path
            .map(|locator| {
                require_non_blank(&locator, "avatar_path")?;
                Ok(LegacyMediaReference { locator })
            })
            .transpose()?;
        let avatar_crop = legacy_crop(crop_x, crop_y, crop_scale)?;
        let design_references = parse_media_references(design_reference_image_ids)?;
        let active_lorebook_ids = parse_lorebook_ids(&active_lorebook_ids)?;
        let image_recommendation = legacy_image_recommendation(lora_name, lora_strength)?;
        if updated_at < created_at {
            return Err(malformed("timestamps"));
        }
        match is_default {
            0 => {}
            1 if default_persona_id.replace(id).is_none() => {}
            1 => return Err(malformed("is_default")),
            _ => return Err(malformed("is_default")),
        }
        personas.push(LegacyPersonaCandidate {
            id,
            title,
            description,
            nickname,
            avatar,
            avatar_crop,
            design_description,
            design_references,
            image_recommendation,
            active_lorebook_ids,
            created_at: TimestampMillis::new(created_at),
            updated_at: TimestampMillis::new(updated_at),
        });
    }
    Ok(LegacyPersonaPlan {
        personas,
        default_persona_id,
    })
}

fn plan_legacy_lorebooks_with_limits(
    connection: &Connection,
    lorebook_limit: u32,
    entry_limit: u32,
    entries_per_book_limit: u32,
) -> Result<LegacyLorebookPlan, LegacyDatabasePreflightError> {
    let entries_per_book_limit_usize = usize::try_from(entries_per_book_limit).map_err(|_| {
        LegacyDatabasePreflightError::LimitExceeded {
            table: "lorebook_entries_per_book",
            limit: entries_per_book_limit,
        }
    })?;
    require_count_limit(connection, "lorebooks", lorebook_limit)?;
    require_count_limit(connection, "lorebook_entries", entry_limit)?;
    let mut statement = connection
        .prepare(
            "SELECT id,name,avatar_path,keyword_detection_mode,created_at,updated_at FROM lorebooks ORDER BY created_at ASC,id ASC",
        )
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    let mut lorebooks = Vec::new();
    let mut indexes = BTreeMap::new();
    for row in rows {
        let (id, name, avatar, detection_policy, created_at, updated_at) =
            row.map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
        let id = LorebookId::from_str(&id).map_err(|_| lorebook_malformed("id"))?;
        require_lorebook_non_blank(&name, "name")?;
        let avatar = avatar
            .map(|locator| {
                require_lorebook_non_blank(&locator, "avatar_path")?;
                Ok(LegacyMediaReference { locator })
            })
            .transpose()?;
        let detection_policy = match detection_policy.as_str() {
            "recent_message_window" => LegacyLorebookDetectionPolicy::RecentMessageWindow,
            "latest_user_message" => LegacyLorebookDetectionPolicy::LatestUserMessage,
            _ => return Err(lorebook_malformed("keyword_detection_mode")),
        };
        if updated_at < created_at {
            return Err(lorebook_malformed("timestamps"));
        }
        indexes.insert(id, lorebooks.len());
        lorebooks.push(LegacyLorebookCandidate {
            id,
            name,
            avatar,
            detection_policy,
            entries: Vec::new(),
            created_at: TimestampMillis::new(created_at),
            updated_at: TimestampMillis::new(updated_at),
        });
    }

    let mut statement = connection
        .prepare(
            "SELECT id,lorebook_id,title,enabled,always_active,keywords,case_sensitive,keyword_match_mode,content,priority,display_order,created_at,updated_at FROM lorebook_entries ORDER BY lorebook_id ASC,display_order ASC,created_at ASC,id ASC",
        )
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, i32>(9)?,
                row.get::<_, i32>(10)?,
                row.get::<_, i64>(11)?,
                row.get::<_, i64>(12)?,
            ))
        })
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    for row in rows {
        let (
            id,
            lorebook_id,
            title,
            enabled,
            always_active,
            keywords,
            case_sensitive,
            match_mode,
            content,
            priority,
            display_order,
            created_at,
            updated_at,
        ) = row.map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
        let id = LorebookEntryId::from_str(&id).map_err(|_| entry_malformed("id"))?;
        let lorebook_id =
            LorebookId::from_str(&lorebook_id).map_err(|_| entry_malformed("lorebook_id"))?;
        let Some(index) = indexes.get(&lorebook_id).copied() else {
            return Err(LegacyDatabasePreflightError::OrphanRecord {
                table: "lorebook_entries",
                parent_table: "lorebooks",
            });
        };
        if lorebooks[index].entries.len() >= entries_per_book_limit_usize {
            return Err(LegacyDatabasePreflightError::LimitExceeded {
                table: "lorebook_entries_per_book",
                limit: entries_per_book_limit,
            });
        }
        let enabled = legacy_flag(enabled, "enabled")?;
        let always_active = legacy_flag(always_active, "always_active")?;
        let case_sensitive = legacy_flag(case_sensitive, "case_sensitive")?;
        let keywords = serde_json::from_str(&keywords).map_err(|_| entry_malformed("keywords"))?;
        let match_mode = match match_mode.as_str() {
            "literal" => LegacyKeywordMatchMode::Literal,
            "regex" => LegacyKeywordMatchMode::Regex,
            _ => return Err(entry_malformed("keyword_match_mode")),
        };
        if updated_at < created_at {
            return Err(entry_malformed("timestamps"));
        }
        lorebooks[index].entries.push(LegacyLorebookEntryCandidate {
            id,
            title,
            enabled,
            always_active,
            keywords,
            case_sensitive,
            match_mode,
            content,
            priority,
            display_order,
            created_at: TimestampMillis::new(created_at),
            updated_at: TimestampMillis::new(updated_at),
        });
    }
    Ok(LegacyLorebookPlan { lorebooks })
}

fn require_count_limit(
    connection: &Connection,
    table: &'static str,
    limit: u32,
) -> Result<(), LegacyDatabasePreflightError> {
    if count(connection, table, table)? > u64::from(limit) {
        Err(LegacyDatabasePreflightError::LimitExceeded { table, limit })
    } else {
        Ok(())
    }
}

fn legacy_flag(value: i64, field: &'static str) -> Result<bool, LegacyDatabasePreflightError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(entry_malformed(field)),
    }
}

fn require_lorebook_non_blank(
    value: &str,
    field: &'static str,
) -> Result<(), LegacyDatabasePreflightError> {
    if value.trim().is_empty() {
        Err(lorebook_malformed(field))
    } else {
        Ok(())
    }
}

fn lorebook_malformed(field: &'static str) -> LegacyDatabasePreflightError {
    LegacyDatabasePreflightError::MalformedRecord {
        table: "lorebooks",
        field,
    }
}

fn entry_malformed(field: &'static str) -> LegacyDatabasePreflightError {
    LegacyDatabasePreflightError::MalformedRecord {
        table: "lorebook_entries",
        field,
    }
}

fn legacy_crop(
    x: Option<f64>,
    y: Option<f64>,
    scale: Option<f64>,
) -> Result<Option<LegacyCrop>, LegacyDatabasePreflightError> {
    match (x, y, scale) {
        (None, None, None) => Ok(None),
        (Some(x), Some(y), Some(scale))
            if x.is_finite() && y.is_finite() && scale.is_finite() && scale > 0.0 =>
        {
            Ok(Some(LegacyCrop { x, y, scale }))
        }
        _ => Err(malformed("avatar_crop")),
    }
}

fn parse_media_references(
    value: Option<String>,
) -> Result<Vec<LegacyMediaReference>, LegacyDatabasePreflightError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let values: Vec<String> =
        serde_json::from_str(&value).map_err(|_| malformed("design_reference_image_ids"))?;
    values
        .into_iter()
        .map(|locator| {
            require_non_blank(&locator, "design_reference_image_ids")?;
            Ok(LegacyMediaReference { locator })
        })
        .collect()
}

fn parse_lorebook_ids(value: &str) -> Result<Vec<LorebookId>, LegacyDatabasePreflightError> {
    let values: Vec<String> =
        serde_json::from_str(value).map_err(|_| malformed("active_lorebook_ids"))?;
    values
        .into_iter()
        .map(|value| LorebookId::from_str(&value).map_err(|_| malformed("active_lorebook_ids")))
        .collect()
}

fn legacy_image_recommendation(
    name: Option<String>,
    strength: Option<f64>,
) -> Result<Option<LegacyImageRecommendation>, LegacyDatabasePreflightError> {
    match (name, strength) {
        (None, None) => Ok(None),
        (Some(model_name), strength) => {
            require_non_blank(&model_name, "lora_name")?;
            let strength = strength.unwrap_or(0.8);
            if !strength.is_finite() || !(0.0..=2.0).contains(&strength) {
                return Err(malformed("lora_strength"));
            }
            Ok(Some(LegacyImageRecommendation {
                model_name,
                strength,
            }))
        }
        (None, Some(_)) => Err(malformed("lora_strength")),
    }
}

fn require_non_blank(value: &str, field: &'static str) -> Result<(), LegacyDatabasePreflightError> {
    if value.trim().is_empty() {
        Err(malformed(field))
    } else {
        Ok(())
    }
}

fn malformed(field: &'static str) -> LegacyDatabasePreflightError {
    LegacyDatabasePreflightError::MalformedRecord {
        table: "personas",
        field,
    }
}

fn require_table(
    connection: &Connection,
    table: &'static str,
) -> Result<(), LegacyDatabasePreflightError> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get(0),
        )
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    if exists {
        Ok(())
    } else {
        Err(LegacyDatabasePreflightError::MissingTable { table })
    }
}

fn count(
    connection: &Connection,
    table: &'static str,
    label: &'static str,
) -> Result<u64, LegacyDatabasePreflightError> {
    let sql = format!("SELECT count(*) FROM {table}");
    let value: i64 = connection
        .query_row(&sql, [], |row| row.get(0))
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    checked_count(value, label)
}

fn checked_count(value: i64, table: &'static str) -> Result<u64, LegacyDatabasePreflightError> {
    u64::try_from(value).map_err(|_| LegacyDatabasePreflightError::CountOutOfRange { table })
}

fn provider_malformed(field: &'static str) -> LegacyDatabasePreflightError {
    LegacyDatabasePreflightError::MalformedRecord {
        table: "provider_credentials",
        field,
    }
}

fn model_malformed(field: &'static str) -> LegacyDatabasePreflightError {
    LegacyDatabasePreflightError::MalformedRecord {
        table: "models",
        field,
    }
}

fn require_provider_non_blank(
    value: &str,
    field: &'static str,
) -> Result<(), LegacyDatabasePreflightError> {
    if value.trim().is_empty() {
        Err(provider_malformed(field))
    } else {
        Ok(())
    }
}

fn require_model_non_blank(
    value: &str,
    field: &'static str,
) -> Result<(), LegacyDatabasePreflightError> {
    if value.trim().is_empty() {
        Err(model_malformed(field))
    } else {
        Ok(())
    }
}

fn normalized_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn parse_optional_object(
    value: Option<String>,
    field: &'static str,
    malformed: fn(&'static str) -> LegacyDatabasePreflightError,
) -> Result<Map<String, Value>, LegacyDatabasePreflightError> {
    match value {
        None => Ok(Map::new()),
        Some(value) => serde_json::from_str::<Value>(&value)
            .ok()
            .and_then(|value| value.as_object().cloned())
            .ok_or_else(|| malformed(field)),
    }
}

fn optional_bool(
    object: &Map<String, Value>,
    key: &'static str,
    default: bool,
) -> Result<bool, LegacyDatabasePreflightError> {
    object
        .get(key)
        .map(|value| value.as_bool().ok_or_else(|| provider_malformed("config")))
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn legacy_provider_protocol(provider_kind: &str) -> Option<ProviderProtocol> {
    let protocol = match provider_kind.to_ascii_lowercase().as_str() {
        "anthropic" | "custom-anthropic" => ProviderProtocol::Anthropic,
        "gemini" | "google" | "google-gemini" | "gemini-agent-platform-express" => {
            ProviderProtocol::Gemini
        }
        "ollama" => ProviderProtocol::Ollama,
        "llamacpp" => ProviderProtocol::LlamaCpp,
        "stability" | "automatic1111" | "comfyui" | "diffusers" | "sdcpp" => {
            ProviderProtocol::StableDiffusion
        }
        "chutes" | "openai" | "cerebras" | "openrouter" | "literouter" | "pollinations"
        | "mistral" | "deepseek" | "nanogpt" | "xai" | "zai" | "moonshot" | "featherless"
        | "qwen" | "nvidia" | "anannas" | "groq" | "lmstudio" | "intenserp" | "custom" => {
            ProviderProtocol::OpenAiCompatible
        }
        _ => return None,
    };
    Some(protocol)
}

fn legacy_provider_config(
    provider_kind: &str,
    object: &Map<String, Value>,
) -> Result<(ProviderConfig, Vec<&'static str>), LegacyDatabasePreflightError> {
    let mut mapped = vec!["streamingEnabled", "allowInvalidTls"];
    if !provider_kind.eq_ignore_ascii_case("custom")
        && !provider_kind.eq_ignore_ascii_case("custom-anthropic")
    {
        return Ok((ProviderConfig::Standard, mapped));
    }
    mapped.extend([
        "chatEndpoint",
        "modelsEndpoint",
        "fetchModelsEnabled",
        "modelsListPath",
        "modelsIdPath",
        "modelsDisplayNamePath",
        "modelsDescriptionPath",
        "modelsContextLengthPath",
        "systemRole",
        "userRole",
        "assistantRole",
        "supportsStream",
        "mergeSameRoleMessages",
        "sendChatTemplateKwargs",
        "toolChoiceMode",
        "authMode",
        "authHeaderName",
        "authQueryParamName",
    ]);
    let string = |key: &'static str| -> Result<Option<String>, LegacyDatabasePreflightError> {
        object
            .get(key)
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| provider_malformed("config"))
            })
            .transpose()
    };
    let role = |key: &'static str| -> Result<Option<WireRole>, LegacyDatabasePreflightError> {
        string(key)?
            .filter(|value| !value.is_empty())
            .map(WireRole::new)
            .transpose()
            .map_err(|_| provider_malformed("config"))
    };
    let chat_path = string("chatEndpoint")?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            if provider_kind.eq_ignore_ascii_case("custom-anthropic") {
                "/v1/messages".to_owned()
            } else {
                "/chat/completions".to_owned()
            }
        });
    let fetch_models = optional_bool(object, "fetchModelsEnabled", false)?;
    let models_path = if fetch_models {
        Some(
            string("modelsEndpoint")?
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| provider_malformed("config"))?,
        )
    } else {
        None
    };
    let path = |key: &'static str, default: &'static str| {
        let value = string(key)?
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| default.to_owned());
        JsonPath::new(value).map_err(|_| provider_malformed("config"))
    };
    let optional_path = |key: &'static str, default: Option<&'static str>| {
        let value = string(key)?.or_else(|| default.map(str::to_owned));
        value
            .filter(|value| !value.is_empty())
            .map(JsonPath::new)
            .transpose()
            .map_err(|_| provider_malformed("config"))
    };
    let auth = match string("authMode")?
        .unwrap_or_else(|| "header".to_owned())
        .to_ascii_lowercase()
        .as_str()
    {
        "bearer" => CustomAuth::Bearer,
        "header" => CustomAuth::Header {
            name: HeaderName::new(
                string("authHeaderName")?.unwrap_or_else(|| "x-api-key".to_owned()),
            )
            .map_err(|_| provider_malformed("config"))?,
        },
        "query" => CustomAuth::Query {
            name: QueryParameterName::new(
                string("authQueryParamName")?.unwrap_or_else(|| "api_key".to_owned()),
            )
            .map_err(|_| provider_malformed("config"))?,
        },
        "none" => CustomAuth::None,
        _ => return Err(provider_malformed("config")),
    };
    let tool_choice_mode = match string("toolChoiceMode")?
        .unwrap_or_else(|| "auto".to_owned())
        .to_ascii_lowercase()
        .as_str()
    {
        "auto" => CustomToolChoiceMode::Auto,
        "required" => CustomToolChoiceMode::Required,
        "none" => CustomToolChoiceMode::None,
        "omit" => CustomToolChoiceMode::Omit,
        "passthrough" => CustomToolChoiceMode::Passthrough,
        _ => return Err(provider_malformed("config")),
    };
    Ok((
        ProviderConfig::Custom(CustomProviderConfig {
            chat_path,
            models_path,
            model_list: CustomModelList {
                list_path: path("modelsListPath", "data")?,
                id_path: path("modelsIdPath", "id")?,
                display_name_path: optional_path("modelsDisplayNamePath", Some("name"))?,
                description_path: optional_path("modelsDescriptionPath", Some("description"))?,
                context_length_path: optional_path("modelsContextLengthPath", None)?,
            },
            streaming: optional_bool(object, "supportsStream", true)?,
            auth,
            roles: CustomRoles {
                system: role("systemRole")?,
                user: role("userRole")?,
                assistant: role("assistantRole")?,
            },
            merge_same_role_messages: optional_bool(object, "mergeSameRoleMessages", true)?,
            send_chat_template_kwargs: optional_bool(object, "sendChatTemplateKwargs", false)?,
            tool_choice_mode,
        }),
        mapped,
    ))
}

fn read_pending_headers(
    connection: &Connection,
    provider_id: &str,
) -> Result<Vec<LegacyPendingProviderSecret>, LegacyDatabasePreflightError> {
    let (is_null, is_valid, json_kind): (i64, Option<i64>, Option<String>) = connection
        .query_row(
            "SELECT headers IS NULL,json_valid(headers),CASE WHEN json_valid(headers) THEN json_type(headers) END FROM provider_credentials WHERE id=?1",
            [provider_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    if is_null == 1 {
        return Ok(Vec::new());
    }
    if is_null != 0 || is_valid != Some(1) || json_kind.as_deref() != Some("object") {
        return Err(provider_malformed("headers"));
    }
    let mut statement = connection
        .prepare(
            "SELECT key,type,length(value) FROM json_each((SELECT headers FROM provider_credentials WHERE id=?1)) ORDER BY lower(key) ASC,key ASC",
        )
        .map_err(|_| LegacyDatabasePreflightError::InvalidSchema)?;
    let rows = statement
        .query_map([provider_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(|_| provider_malformed("headers"))?;
    let mut names = Vec::new();
    for row in rows {
        let (name, value_type, value_len) = row.map_err(|_| provider_malformed("headers"))?;
        if value_type != "text" || value_len == 0 {
            return Err(provider_malformed("headers"));
        }
        let name = HeaderName::new(name).map_err(|_| provider_malformed("headers"))?;
        if names
            .iter()
            .any(|existing: &HeaderName| existing.as_str().eq_ignore_ascii_case(name.as_str()))
        {
            return Err(provider_malformed("headers"));
        }
        names.push(name);
    }
    if names.len() > 16 {
        return Err(provider_malformed("headers"));
    }
    Ok(names
        .into_iter()
        .map(|name| LegacyPendingProviderSecret::Header { name })
        .collect())
}

fn deferred_fields(object: &Map<String, Value>, mapped: &[&str]) -> Vec<String> {
    let mut fields = object
        .keys()
        .filter(|key| !mapped.contains(&key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    fields.sort();
    fields
}

fn resolve_legacy_provider_account(
    providers: &[LegacyProviderAccountCandidate],
    provider_kind: &str,
    explicit_id: Option<ProviderAccountId>,
    provider_label: &str,
    model_name: &str,
    default_id: Option<ProviderAccountId>,
) -> Result<ProviderAccountId, LegacyDatabasePreflightError> {
    if provider_kind.eq_ignore_ascii_case("llamacpp") {
        return Ok(legacy_builtin_llama_account_id());
    }
    if let Some(provider) = explicit_id.and_then(|id| {
        providers
            .iter()
            .find(|provider| provider.id == id && provider.provider_kind == provider_kind)
    }) {
        return Ok(provider.id);
    }
    let candidates = providers
        .iter()
        .filter(|provider| provider.provider_kind == provider_kind)
        .collect::<Vec<_>>();
    if let Some(provider) = default_id.and_then(|id| {
        candidates
            .iter()
            .copied()
            .find(|provider| provider.id == id)
    }) {
        return Ok(provider.id);
    }
    if let [provider] = candidates.as_slice() {
        return Ok(provider.id);
    }
    if let Some(provider) = candidates
        .iter()
        .copied()
        .find(|provider| provider.label == provider_label)
    {
        return Ok(provider.id);
    }
    if let Some(provider) = candidates
        .iter()
        .copied()
        .find(|provider| provider.default_model.as_deref() == Some(model_name))
    {
        return Ok(provider.id);
    }
    Err(LegacyDatabasePreflightError::OrphanRecord {
        table: "models",
        parent_table: "provider_credentials",
    })
}

fn legacy_builtin_llama_account_id() -> ProviderAccountId {
    ProviderAccountId::from_str("6c657474-7563-652d-6c6c-616d61637070")
        .expect("static legacy llama account id")
}

fn parse_modalities(
    value: Option<String>,
    field: &'static str,
) -> Result<ModalityCapabilities, LegacyDatabasePreflightError> {
    let values = match value {
        None => vec![Value::String("text".to_owned())],
        Some(value) => {
            serde_json::from_str::<Vec<Value>>(&value).map_err(|_| model_malformed(field))?
        }
    };
    let mut modalities = ModalityCapabilities {
        text: CapabilityStatus::Unsupported,
        image: CapabilityStatus::Unsupported,
        audio: CapabilityStatus::Unsupported,
    };
    for value in values {
        match value.as_str() {
            Some("text") => modalities.text = CapabilityStatus::Supported,
            Some("image") => modalities.image = CapabilityStatus::Supported,
            Some("audio") => modalities.audio = CapabilityStatus::Supported,
            _ => return Err(model_malformed(field)),
        }
    }
    if matches!(
        (modalities.text, modalities.image, modalities.audio),
        (
            CapabilityStatus::Unsupported,
            CapabilityStatus::Unsupported,
            CapabilityStatus::Unsupported
        )
    ) {
        modalities.text = CapabilityStatus::Supported;
    }
    Ok(modalities)
}

fn legacy_chat_parameters(
    provider_kind: &str,
    object: &Map<String, Value>,
) -> Result<(ChatParameterProfile, Vec<&'static str>), LegacyDatabasePreflightError> {
    let mapped = vec![
        "temperature",
        "topP",
        "topK",
        "maxOutputTokens",
        "contextLength",
        "frequencyPenalty",
        "presencePenalty",
        "ollamaNumCtx",
        "ollamaNumPredict",
        "ollamaNumKeep",
        "ollamaNumBatch",
        "ollamaNumGpu",
        "ollamaNumThread",
        "ollamaTfsZ",
        "ollamaTypicalP",
        "ollamaMinP",
        "ollamaMirostat",
        "ollamaMirostatTau",
        "ollamaMirostatEta",
        "ollamaRepeatPenalty",
        "ollamaSeed",
        "ollamaStop",
        "reasoningEnabled",
        "reasoningEffort",
        "reasoningBudgetTokens",
        "promptCachingEnabled",
        "promptCachingTtl",
        "openRouterProvider",
    ];
    let f64_value = |key: &'static str| -> Result<Option<f64>, LegacyDatabasePreflightError> {
        object
            .get(key)
            .filter(|value| !value.is_null())
            .map(|value| {
                value
                    .as_f64()
                    .ok_or_else(|| model_malformed("advanced_model_settings"))
            })
            .transpose()
    };
    let u32_value = |key: &'static str| -> Result<Option<u32>, LegacyDatabasePreflightError> {
        object
            .get(key)
            .filter(|value| !value.is_null())
            .map(|value| {
                value
                    .as_u64()
                    .and_then(|value| u32::try_from(value).ok())
                    .ok_or_else(|| model_malformed("advanced_model_settings"))
            })
            .transpose()
    };
    let bool_value = |key: &'static str| -> Result<Option<bool>, LegacyDatabasePreflightError> {
        object
            .get(key)
            .filter(|value| !value.is_null())
            .map(|value| {
                value
                    .as_bool()
                    .ok_or_else(|| model_malformed("advanced_model_settings"))
            })
            .transpose()
    };
    let string_value = |key: &'static str| -> Result<Option<String>, LegacyDatabasePreflightError> {
        object
            .get(key)
            .filter(|value| !value.is_null())
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| model_malformed("advanced_model_settings"))
            })
            .transpose()
    };
    let positive_or_none = |value: Option<u32>| value.filter(|value| *value != 0);
    let reasoning_mode = bool_value("reasoningEnabled")?.map(|enabled| {
        if enabled {
            ReasoningMode::Enabled
        } else {
            ReasoningMode::Disabled
        }
    });
    let reasoning_effort = if reasoning_mode == Some(ReasoningMode::Disabled) {
        None
    } else {
        string_value("reasoningEffort")?
            .map(|value| match value.as_str() {
                "low" => Ok(ReasoningEffort::Low),
                "medium" => Ok(ReasoningEffort::Medium),
                "high" => Ok(ReasoningEffort::High),
                _ => Err(model_malformed("advanced_model_settings")),
            })
            .transpose()?
    };
    let reasoning_budget_tokens = if reasoning_mode == Some(ReasoningMode::Disabled) {
        None
    } else {
        u32_value("reasoningBudgetTokens")?
    };
    let prompt_caching = match bool_value("promptCachingEnabled")? {
        None => None,
        Some(false) => Some(PromptCaching::Disabled),
        Some(true) => {
            let default = if provider_kind == "openai" {
                "in_memory"
            } else {
                "5min"
            };
            let retention = match string_value("promptCachingTtl")?
                .unwrap_or_else(|| default.to_owned())
                .as_str()
            {
                "in_memory" => PromptCacheRetention::InMemory,
                "5min" => PromptCacheRetention::FiveMinutes,
                "1h" => PromptCacheRetention::OneHour,
                "24h" => PromptCacheRetention::TwentyFourHours,
                _ => return Err(model_malformed("advanced_model_settings")),
            };
            Some(PromptCaching::Enabled { retention })
        }
    };
    let stop = object
        .get("ollamaStop")
        .filter(|value| !value.is_null())
        .map(|value| {
            value
                .as_array()
                .ok_or_else(|| model_malformed("advanced_model_settings"))?
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| model_malformed("advanced_model_settings"))
                })
                .collect()
        })
        .transpose()?;
    let pinned_provider = object
        .get("openRouterProvider")
        .filter(|value| !value.is_null())
        .map(|value| {
            value
                .as_object()
                .and_then(|value| value.get("id"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| model_malformed("advanced_model_settings"))
        })
        .transpose()?;
    Ok((
        ChatParameterProfile {
            temperature: f64_value("temperature")?,
            top_p: f64_value("topP")?,
            top_k: u32_value("topK")?,
            max_output_tokens: u32_value("maxOutputTokens")?
                .or(u32_value("ollamaNumPredict")?)
                .and_then(|value| (value != 0).then_some(value)),
            context_length: positive_or_none(
                u32_value("contextLength")?.or(u32_value("ollamaNumCtx")?),
            ),
            frequency_penalty: f64_value("frequencyPenalty")?,
            presence_penalty: f64_value("presencePenalty")?,
            repetition_penalty: f64_value("ollamaRepeatPenalty")?,
            reasoning_mode,
            reasoning_effort,
            reasoning_budget_tokens,
            prompt_caching,
            ollama: OllamaOptions {
                num_keep: u32_value("ollamaNumKeep")?,
                num_batch: u32_value("ollamaNumBatch")?,
                num_gpu: u32_value("ollamaNumGpu")?,
                num_thread: u32_value("ollamaNumThread")?,
                tfs_z: f64_value("ollamaTfsZ")?,
                typical_p: f64_value("ollamaTypicalP")?,
                min_p: f64_value("ollamaMinP")?,
                mirostat: u32_value("ollamaMirostat")?,
                mirostat_tau: f64_value("ollamaMirostatTau")?,
                mirostat_eta: f64_value("ollamaMirostatEta")?,
                seed: u32_value("ollamaSeed")?,
                stop,
            },
            openrouter: OpenRouterOptions { pinned_provider },
        },
        mapped,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_types::MediaBlobId;

    fn legacy_database(version: i64) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("lettuce-legacy-{}.db", MediaBlobId::new()));
        let connection = Connection::open(&path).expect("create legacy database");
        connection
            .execute_batch(
                "CREATE TABLE settings (id INTEGER PRIMARY KEY, migration_version INTEGER NOT NULL);
                 CREATE TABLE provider_credentials (id TEXT PRIMARY KEY);
                 CREATE TABLE models (id TEXT PRIMARY KEY);
                 CREATE TABLE prompt_templates (id TEXT PRIMARY KEY);
                 CREATE TABLE personas (
                   id TEXT PRIMARY KEY,
                   title TEXT NOT NULL,
                   description TEXT NOT NULL,
                   nickname TEXT,
                   avatar_path TEXT,
                   avatar_crop_x REAL,
                   avatar_crop_y REAL,
                   avatar_crop_scale REAL,
                   design_description TEXT,
                   design_reference_image_ids TEXT,
                   active_lorebook_ids TEXT NOT NULL DEFAULT '[]',
                   is_default INTEGER NOT NULL DEFAULT 0,
                   lora_name TEXT,
                   lora_strength REAL,
                   created_at INTEGER NOT NULL,
                   updated_at INTEGER NOT NULL
                 );
                 CREATE TABLE characters (id TEXT PRIMARY KEY);
                 CREATE TABLE lorebooks (
                   id TEXT PRIMARY KEY,
                   name TEXT NOT NULL,
                   avatar_path TEXT,
                   keyword_detection_mode TEXT NOT NULL DEFAULT 'recent_message_window',
                   created_at INTEGER NOT NULL,
                   updated_at INTEGER NOT NULL
                 );
                 CREATE TABLE lorebook_entries (
                   id TEXT PRIMARY KEY,
                   lorebook_id TEXT NOT NULL,
                   title TEXT NOT NULL DEFAULT '',
                   enabled INTEGER NOT NULL DEFAULT 1,
                   always_active INTEGER NOT NULL DEFAULT 0,
                   keywords TEXT NOT NULL DEFAULT '[]',
                   case_sensitive INTEGER NOT NULL DEFAULT 0,
                   keyword_match_mode TEXT NOT NULL DEFAULT 'literal',
                   content TEXT NOT NULL,
                   priority INTEGER NOT NULL DEFAULT 0,
                   display_order INTEGER NOT NULL DEFAULT 0,
                   created_at INTEGER NOT NULL,
                   updated_at INTEGER NOT NULL,
                   FOREIGN KEY(lorebook_id) REFERENCES lorebooks(id) ON DELETE CASCADE
                 );
                 CREATE TABLE chat_templates (id TEXT PRIMARY KEY);
                 CREATE TABLE sessions (id TEXT PRIMARY KEY);
                 CREATE TABLE group_characters (id TEXT PRIMARY KEY);
                 CREATE TABLE group_sessions (id TEXT PRIMARY KEY);",
            )
            .expect("create legacy schema");
        connection
            .execute(
                "INSERT INTO settings (id, migration_version) VALUES (1, ?1)",
                [version],
            )
            .expect("insert settings");
        for table in ["characters", "characters", "sessions"] {
            connection
                .execute(
                    &format!("INSERT INTO {table} (id) VALUES (?1)"),
                    [MediaBlobId::new().to_string()],
                )
                .expect("insert root");
        }
        connection
            .execute(
                "INSERT INTO personas (id,title,description,created_at,updated_at) VALUES (?1,'Reader','Reads stories',10,10)",
                [PersonaId::new().to_string()],
            )
            .expect("insert persona");
        drop(connection);
        path
    }

    fn provider_model_database() -> std::path::PathBuf {
        let path = legacy_database(i64::from(LEGACY_DATABASE_SCHEMA_VERSION));
        let connection = Connection::open(&path).expect("open legacy database");
        connection
            .execute_batch(
                "DROP TABLE provider_credentials;
                 DROP TABLE models;
                 DROP TABLE settings;
                 CREATE TABLE settings (
                   id INTEGER PRIMARY KEY CHECK(id=1),
                   default_provider_credential_id TEXT,
                   default_model_id TEXT,
                   migration_version INTEGER NOT NULL,
                   created_at INTEGER NOT NULL,
                   updated_at INTEGER NOT NULL
                 );
                 CREATE TABLE provider_credentials (
                   id TEXT PRIMARY KEY,
                   provider_id TEXT NOT NULL,
                   label TEXT NOT NULL,
                   api_key_ref TEXT,
                   api_key TEXT,
                   base_url TEXT,
                   default_model TEXT,
                   headers TEXT,
                   config TEXT
                 );
                 CREATE TABLE models (
                   id TEXT PRIMARY KEY,
                   name TEXT NOT NULL,
                   provider_id TEXT NOT NULL,
                   provider_credential_id TEXT,
                   provider_label TEXT NOT NULL,
                   display_name TEXT NOT NULL,
                   created_at INTEGER NOT NULL,
                   model_type TEXT NOT NULL DEFAULT 'chat',
                   input_scopes TEXT,
                   output_scopes TEXT,
                   advanced_model_settings TEXT,
                   prompt_template_id TEXT,
                   system_prompt TEXT
                 );",
            )
            .expect("create provider model schema");
        drop(connection);
        path
    }

    #[test]
    fn valid_legacy_database_reports_bounded_root_inventory() {
        let path = legacy_database(i64::from(LEGACY_DATABASE_SCHEMA_VERSION));
        let inventory = preflight_legacy_database(&path).expect("preflight");

        assert_eq!(inventory.schema_version, LEGACY_DATABASE_SCHEMA_VERSION);
        assert_eq!(inventory.characters, 2);
        assert_eq!(inventory.personas, 1);
        assert_eq!(inventory.direct_conversations, 1);
        assert_eq!(inventory.group_conversations, 0);
        std::fs::remove_file(path).expect("remove legacy database");
    }

    #[test]
    fn unsupported_or_incomplete_legacy_schema_is_rejected() {
        let unsupported = legacy_database(91);
        assert_eq!(
            preflight_legacy_database(&unsupported),
            Err(LegacyDatabasePreflightError::UnsupportedVersion {
                found: 91,
                supported: LEGACY_DATABASE_SCHEMA_VERSION,
            })
        );
        std::fs::remove_file(unsupported).expect("remove unsupported database");

        let incomplete = std::env::temp_dir().join(format!(
            "lettuce-legacy-incomplete-{}.db",
            MediaBlobId::new()
        ));
        drop(Connection::open(&incomplete).expect("create incomplete database"));
        assert_eq!(
            preflight_legacy_database(&incomplete),
            Err(LegacyDatabasePreflightError::MissingTable {
                table: "provider_credentials"
            })
        );
        std::fs::remove_file(incomplete).expect("remove incomplete database");
    }

    #[test]
    fn negative_count_is_rejected_by_the_bounded_conversion() {
        assert_eq!(
            checked_count(-1, "characters"),
            Err(LegacyDatabasePreflightError::CountOutOfRange {
                table: "characters"
            })
        );
    }

    #[test]
    fn provider_model_plan_preserves_mapped_fields_defaults_and_source_bytes() {
        let path = provider_model_database();
        let custom_id =
            ProviderAccountId::from_str("00000000-0000-0000-0000-000000000010").expect("custom id");
        let router_id =
            ProviderAccountId::from_str("00000000-0000-0000-0000-000000000011").expect("router id");
        let custom_model_id = ModelProfileId::from_str("00000000-0000-0000-0000-000000000020")
            .expect("custom model id");
        let router_model_id = ModelProfileId::from_str("00000000-0000-0000-0000-000000000021")
            .expect("router model id");
        let connection = Connection::open(&path).expect("open legacy database");
        connection
            .execute(
                "INSERT INTO settings VALUES (1,?1,?2,92,100,120)",
                rusqlite::params![router_id.to_string(), router_model_id.to_string()],
            )
            .expect("insert settings");
        connection
            .execute(
                "INSERT INTO provider_credentials (id,provider_id,label,api_key_ref,api_key,base_url,default_model,headers,config) VALUES (?1,'openrouter','Router','obsolete-ref','router-secret','https://router.example/api/','router-model',?2,?3)",
                rusqlite::params![
                    router_id.to_string(),
                    r#"{"X-Zeta":"zeta-secret","X-Alpha":"alpha-secret"}"#,
                    r#"{"streamingEnabled":false,"allowInvalidTls":true,"sproutEnabled":true}"#
                ],
            )
            .expect("insert router");
        connection
            .execute(
                "INSERT INTO provider_credentials (id,provider_id,label,api_key,base_url,headers,config) VALUES (?1,'custom','Custom',NULL,'https://custom.example',NULL,?2)",
                rusqlite::params![
                    custom_id.to_string(),
                    r#"{"chatEndpoint":"/v2/chat","fetchModelsEnabled":true,"modelsEndpoint":"/v2/models","modelsListPath":"payload.items","modelsIdPath":"key","modelsDisplayNamePath":"title","modelsDescriptionPath":"summary","modelsContextLengthPath":"limits.context","systemRole":"instruction","userRole":"human","assistantRole":"bot","supportsStream":false,"mergeSameRoleMessages":false,"sendChatTemplateKwargs":true,"toolChoiceMode":"required","authMode":"query","authQueryParamName":"token"}"#
                ],
            )
            .expect("insert custom");
        connection
            .execute(
                "INSERT INTO models (id,name,provider_id,provider_credential_id,provider_label,display_name,created_at,model_type,input_scopes,output_scopes,advanced_model_settings,prompt_template_id,system_prompt) VALUES (?1,'router-model','openrouter',?2,'Router','Router Model',20,'chat','[\"text\",\"image\"]','[\"text\"]',?3,'prompt-one','Legacy system')",
                rusqlite::params![
                    router_model_id.to_string(),
                    router_id.to_string(),
                    r#"{"temperature":0.7,"topP":0.8,"topK":40,"maxOutputTokens":2048,"contextLength":0,"frequencyPenalty":0.2,"presencePenalty":-0.1,"reasoningEnabled":true,"reasoningEffort":"high","reasoningBudgetTokens":4096,"promptCachingEnabled":true,"promptCachingTtl":"1h","openRouterProvider":{"id":"route-a","name":"Route A"},"llamaGpuLayers":18}"#
                ],
            )
            .expect("insert router model");
        connection
            .execute(
                "INSERT INTO models (id,name,provider_id,provider_credential_id,provider_label,display_name,created_at,model_type,input_scopes,output_scopes,advanced_model_settings) VALUES (?1,'custom-model','custom',?2,'Custom','Custom Model',10,'chat',NULL,NULL,?3)",
                rusqlite::params![
                    custom_model_id.to_string(),
                    custom_id.to_string(),
                    r#"{"ollamaNumPredict":512,"ollamaNumCtx":8192,"ollamaNumKeep":8,"ollamaNumBatch":64,"ollamaNumGpu":2,"ollamaNumThread":6,"ollamaTfsZ":0.9,"ollamaTypicalP":0.8,"ollamaMinP":0.1,"ollamaMirostat":2,"ollamaMirostatTau":5.0,"ollamaMirostatEta":0.2,"ollamaRepeatPenalty":1.1,"ollamaSeed":9,"ollamaStop":["END"]}"#
                ],
            )
            .expect("insert custom model");
        drop(connection);
        let before = std::fs::read(&path).expect("read source before planning");

        let plan = plan_legacy_provider_models(&path).expect("plan provider models");

        assert_eq!(
            std::fs::read(&path).expect("read source after planning"),
            before
        );
        assert_eq!(plan.default_provider_account_id, Some(router_id));
        assert_eq!(plan.default_model_profile_id, Some(router_model_id));
        assert_eq!(
            plan.provider_accounts
                .iter()
                .map(|provider| provider.id)
                .collect::<Vec<_>>(),
            vec![custom_id, router_id]
        );
        let custom = &plan.provider_accounts[0];
        assert_eq!(custom.secret_owner_id.as_uuid(), custom_id.as_uuid());
        assert_eq!(custom.protocol, ProviderProtocol::OpenAiCompatible);
        assert_eq!(custom.endpoint.as_deref(), Some("https://custom.example"));
        assert!(custom.enabled);
        assert!(custom.streaming_enabled);
        assert!(!custom.allow_invalid_tls);
        assert_eq!(custom.created_at, TimestampMillis::new(100));
        assert_eq!(custom.updated_at, TimestampMillis::new(120));
        assert!(custom.pending_secrets.is_empty());
        let ProviderConfig::Custom(config) = &custom.config else {
            panic!("custom provider config");
        };
        assert_eq!(config.chat_path, "/v2/chat");
        assert_eq!(config.models_path.as_deref(), Some("/v2/models"));
        assert_eq!(config.model_list.list_path.as_str(), "payload.items");
        assert_eq!(config.model_list.id_path.as_str(), "key");
        assert_eq!(
            config
                .model_list
                .display_name_path
                .as_ref()
                .map(JsonPath::as_str),
            Some("title")
        );
        assert_eq!(
            config
                .model_list
                .description_path
                .as_ref()
                .map(JsonPath::as_str),
            Some("summary")
        );
        assert_eq!(
            config
                .model_list
                .context_length_path
                .as_ref()
                .map(JsonPath::as_str),
            Some("limits.context")
        );
        assert_eq!(
            config.roles.system.as_ref().map(WireRole::as_str),
            Some("instruction")
        );
        assert_eq!(
            config.roles.user.as_ref().map(WireRole::as_str),
            Some("human")
        );
        assert_eq!(
            config.roles.assistant.as_ref().map(WireRole::as_str),
            Some("bot")
        );
        assert!(!config.streaming);
        assert!(!config.merge_same_role_messages);
        assert!(config.send_chat_template_kwargs);
        assert_eq!(config.tool_choice_mode, CustomToolChoiceMode::Required);
        assert!(matches!(
            &config.auth,
            CustomAuth::Query { name } if name.as_str() == "token"
        ));
        let router = &plan.provider_accounts[1];
        assert_eq!(router.protocol, ProviderProtocol::OpenAiCompatible);
        assert_eq!(
            router.endpoint.as_deref(),
            Some("https://router.example/api/")
        );
        assert!(!router.streaming_enabled);
        assert!(router.allow_invalid_tls);
        assert_eq!(router.default_model.as_deref(), Some("router-model"));
        assert_eq!(router.deferred_config_fields, vec!["sproutEnabled"]);
        assert_eq!(
            router.pending_secrets,
            vec![
                LegacyPendingProviderSecret::ApiKey,
                LegacyPendingProviderSecret::Header {
                    name: HeaderName::new("X-Alpha").expect("header")
                },
                LegacyPendingProviderSecret::Header {
                    name: HeaderName::new("X-Zeta").expect("header")
                }
            ]
        );
        let debug = format!("{plan:?}");
        assert!(!debug.contains("router-secret"));
        assert!(!debug.contains("alpha-secret"));

        assert_eq!(
            plan.model_profiles
                .iter()
                .map(|model| model.id)
                .collect::<Vec<_>>(),
            vec![custom_model_id, router_model_id]
        );
        let custom_model = &plan.model_profiles[0];
        assert_eq!(custom_model.provider_account_id, custom_id);
        assert_eq!(
            custom_model.config.chat_parameters.max_output_tokens,
            Some(512)
        );
        assert_eq!(
            custom_model.config.chat_parameters.context_length,
            Some(8192)
        );
        assert_eq!(
            custom_model.config.chat_parameters.repetition_penalty,
            Some(1.1)
        );
        assert_eq!(custom_model.config.chat_parameters.ollama.num_keep, Some(8));
        assert_eq!(
            custom_model.config.chat_parameters.ollama.num_batch,
            Some(64)
        );
        assert_eq!(custom_model.config.chat_parameters.ollama.num_gpu, Some(2));
        assert_eq!(
            custom_model.config.chat_parameters.ollama.num_thread,
            Some(6)
        );
        assert_eq!(custom_model.config.chat_parameters.ollama.tfs_z, Some(0.9));
        assert_eq!(
            custom_model.config.chat_parameters.ollama.typical_p,
            Some(0.8)
        );
        assert_eq!(custom_model.config.chat_parameters.ollama.min_p, Some(0.1));
        assert_eq!(custom_model.config.chat_parameters.ollama.mirostat, Some(2));
        assert_eq!(
            custom_model.config.chat_parameters.ollama.mirostat_tau,
            Some(5.0)
        );
        assert_eq!(
            custom_model.config.chat_parameters.ollama.mirostat_eta,
            Some(0.2)
        );
        assert_eq!(custom_model.config.chat_parameters.ollama.seed, Some(9));
        assert_eq!(
            custom_model.config.chat_parameters.ollama.stop,
            Some(vec!["END".into()])
        );
        assert_eq!(
            custom_model.config.capabilities.input_modalities.text,
            CapabilityStatus::Supported
        );
        assert_eq!(
            custom_model.config.capabilities.input_modalities.image,
            CapabilityStatus::Unsupported
        );
        let router_model = &plan.model_profiles[1];
        assert_eq!(router_model.provider_account_id, router_id);
        assert_eq!(router_model.source_provider_kind, "openrouter");
        assert_eq!(router_model.source_provider_label, "Router");
        assert_eq!(router_model.external_model_id, "router-model");
        assert_eq!(router_model.display_name, "Router Model");
        assert_eq!(router_model.kind, ModelKind::Chat);
        assert_eq!(router_model.created_at, TimestampMillis::new(20));
        assert_eq!(router_model.config.chat_parameters.temperature, Some(0.7));
        assert_eq!(router_model.config.chat_parameters.top_p, Some(0.8));
        assert_eq!(router_model.config.chat_parameters.top_k, Some(40));
        assert_eq!(
            router_model.config.chat_parameters.max_output_tokens,
            Some(2048)
        );
        assert_eq!(router_model.config.chat_parameters.context_length, None);
        assert_eq!(
            router_model.config.chat_parameters.frequency_penalty,
            Some(0.2)
        );
        assert_eq!(
            router_model.config.chat_parameters.presence_penalty,
            Some(-0.1)
        );
        assert_eq!(
            router_model.config.chat_parameters.reasoning_mode,
            Some(ReasoningMode::Enabled)
        );
        assert_eq!(
            router_model.config.chat_parameters.reasoning_effort,
            Some(ReasoningEffort::High)
        );
        assert_eq!(
            router_model.config.chat_parameters.reasoning_budget_tokens,
            Some(4096)
        );
        assert_eq!(
            router_model.config.chat_parameters.prompt_caching,
            Some(PromptCaching::Enabled {
                retention: PromptCacheRetention::OneHour
            })
        );
        assert_eq!(
            router_model
                .config
                .chat_parameters
                .openrouter
                .pinned_provider
                .as_deref(),
            Some("route-a")
        );
        assert_eq!(
            router_model.prompt_template_id.as_deref(),
            Some("prompt-one")
        );
        assert_eq!(
            router_model.deprecated_system_prompt.as_deref(),
            Some("Legacy system")
        );
        assert_eq!(
            router_model.deferred_advanced_fields,
            vec!["llamaGpuLayers"]
        );
        assert_eq!(
            router_model.config.capabilities.input_modalities.image,
            CapabilityStatus::Supported
        );
        assert_eq!(
            router_model.config.capabilities.output_modalities.text,
            CapabilityStatus::Supported
        );
        assert_eq!(
            router_model.config.capabilities.output_modalities.image,
            CapabilityStatus::Unsupported
        );
        assert_eq!(
            router_model.config.capabilities.streaming,
            CapabilityStatus::Unsupported
        );
        std::fs::remove_file(path).expect("remove legacy database");
    }

    #[test]
    fn provider_model_plan_rejects_malformed_orphans_and_bounds() {
        let path = provider_model_database();
        let provider_id = ProviderAccountId::new();
        let model_id = ModelProfileId::new();
        let connection = Connection::open(&path).expect("open legacy database");
        connection
            .execute("INSERT INTO settings VALUES (1,NULL,NULL,92,10,10)", [])
            .expect("insert settings");
        connection
            .execute(
                "INSERT INTO provider_credentials (id,provider_id,label,headers) VALUES (?1,'openai','Primary','not-json')",
                [provider_id.to_string()],
            )
            .expect("insert malformed provider");
        assert_eq!(
            plan_legacy_provider_models(&path),
            Err(provider_malformed("headers"))
        );
        connection
            .execute(
                "UPDATE provider_credentials SET headers=NULL WHERE id=?1",
                [provider_id.to_string()],
            )
            .expect("fix provider");
        connection
            .execute(
                "INSERT INTO models (id,name,provider_id,provider_credential_id,provider_label,display_name,created_at) VALUES (?1,'model-a','anthropic',NULL,'Missing','Model A',20)",
                [model_id.to_string()],
            )
            .expect("insert orphan model");
        assert_eq!(
            plan_legacy_provider_models(&path),
            Err(LegacyDatabasePreflightError::OrphanRecord {
                table: "models",
                parent_table: "provider_credentials"
            })
        );
        connection
            .execute("DELETE FROM models", [])
            .expect("delete orphan model");
        assert_eq!(
            plan_legacy_provider_models_with_limits(&connection, 0, 1),
            Err(LegacyDatabasePreflightError::LimitExceeded {
                table: "provider_credentials",
                limit: 0
            })
        );
        connection
            .execute(
                "INSERT INTO models (id,name,provider_id,provider_credential_id,provider_label,display_name,created_at) VALUES (?1,'model-a','openai',?2,'Primary','Model A',20)",
                rusqlite::params![model_id.to_string(), provider_id.to_string()],
            )
            .expect("insert model");
        assert_eq!(
            plan_legacy_provider_models_with_limits(&connection, 1, 0),
            Err(LegacyDatabasePreflightError::LimitExceeded {
                table: "models",
                limit: 0
            })
        );
        connection
            .execute(
                "UPDATE settings SET default_model_id=?1",
                [ModelProfileId::new().to_string()],
            )
            .expect("set orphan default");
        assert_eq!(
            plan_legacy_provider_models(&path),
            Err(LegacyDatabasePreflightError::OrphanRecord {
                table: "settings.default_model_id",
                parent_table: "models"
            })
        );
        drop(connection);
        std::fs::remove_file(path).expect("remove legacy database");
    }

    #[test]
    fn provider_model_plan_maps_each_legacy_protocol_family() {
        let path = provider_model_database();
        let connection = Connection::open(&path).expect("open legacy database");
        connection
            .execute("INSERT INTO settings VALUES (1,NULL,NULL,92,10,10)", [])
            .expect("insert settings");
        for (ordinal, provider_kind) in ["anthropic", "gemini", "ollama", "sdcpp", "openai"]
            .into_iter()
            .enumerate()
        {
            connection
                .execute(
                    "INSERT INTO provider_credentials (id,provider_id,label) VALUES (?1,?2,?3)",
                    rusqlite::params![
                        ProviderAccountId::new().to_string(),
                        provider_kind,
                        format!("Provider {ordinal}")
                    ],
                )
                .expect("insert provider");
        }
        let llama_model_id = ModelProfileId::new();
        connection
            .execute(
                "INSERT INTO models (id,name,provider_id,provider_label,display_name,created_at) VALUES (?1,'local.gguf','llamacpp','llama.cpp (Local)','Local Model',20)",
                [llama_model_id.to_string()],
            )
            .expect("insert llama model");
        drop(connection);

        let plan = plan_legacy_provider_models(&path).expect("plan provider protocols");
        let protocol = |kind: &str| {
            plan.provider_accounts
                .iter()
                .find(|provider| provider.provider_kind == kind)
                .map(|provider| provider.protocol)
        };
        assert_eq!(protocol("anthropic"), Some(ProviderProtocol::Anthropic));
        assert_eq!(protocol("gemini"), Some(ProviderProtocol::Gemini));
        assert_eq!(protocol("ollama"), Some(ProviderProtocol::Ollama));
        assert_eq!(protocol("llamacpp"), Some(ProviderProtocol::LlamaCpp));
        assert_eq!(
            plan.model_profiles
                .iter()
                .find(|model| model.id == llama_model_id)
                .map(|model| model.provider_account_id),
            Some(legacy_builtin_llama_account_id())
        );
        assert_eq!(protocol("sdcpp"), Some(ProviderProtocol::StableDiffusion));
        assert_eq!(protocol("openai"), Some(ProviderProtocol::OpenAiCompatible));
        std::fs::remove_file(path).expect("remove legacy database");
    }

    #[test]
    fn provider_model_plan_preserves_legacy_credential_resolution_order() {
        let path = provider_model_database();
        let default_id = ProviderAccountId::new();
        let other_openai_id = ProviderAccountId::new();
        let sole_id = ProviderAccountId::new();
        let label_first_id = ProviderAccountId::new();
        let label_match_id = ProviderAccountId::new();
        let model_default_id = ProviderAccountId::new();
        let other_router_id = ProviderAccountId::new();
        let connection = Connection::open(&path).expect("open legacy database");
        connection
            .execute(
                "INSERT INTO settings VALUES (1,?1,NULL,92,10,10)",
                [default_id.to_string()],
            )
            .expect("insert settings");
        for (id, kind, label, default_model) in [
            (default_id, "openai", "Default", None),
            (other_openai_id, "openai", "Other", None),
            (sole_id, "anthropic", "Only", None),
            (label_first_id, "custom", "First", None),
            (label_match_id, "custom", "Match", None),
            (model_default_id, "openrouter", "Route One", Some("target")),
            (other_router_id, "openrouter", "Route Two", None),
        ] {
            connection
                .execute(
                    "INSERT INTO provider_credentials (id,provider_id,label,base_url,default_model,config) VALUES (?1,?2,?3,CASE WHEN ?2='custom' THEN 'https://custom.example' END,?4,CASE WHEN ?2='custom' THEN '{\"authMode\":\"none\"}' END)",
                    rusqlite::params![id.to_string(), kind, label, default_model],
                )
                .expect("insert provider");
        }
        let cases = [
            ("openai", "Other", "openai-model", default_id),
            ("anthropic", "Ignored", "anthropic-model", sole_id),
            ("custom", "Match", "custom-model", label_match_id),
            ("openrouter", "Missing", "target", model_default_id),
        ];
        for (ordinal, (kind, label, name, _)) in cases.iter().enumerate() {
            connection
                .execute(
                    "INSERT INTO models (id,name,provider_id,provider_label,display_name,created_at) VALUES (?1,?2,?3,?4,?5,?6)",
                    rusqlite::params![
                        ModelProfileId::new().to_string(),
                        name,
                        kind,
                        label,
                        format!("Model {ordinal}"),
                        20 + i64::try_from(ordinal).expect("ordinal")
                    ],
                )
                .expect("insert model");
        }
        drop(connection);

        let plan = plan_legacy_provider_models(&path).expect("plan credential resolution");
        assert_eq!(plan.model_profiles.len(), cases.len());
        for (model, (_, _, _, expected_provider)) in plan.model_profiles.iter().zip(cases) {
            assert_eq!(model.provider_account_id, expected_provider);
        }
        std::fs::remove_file(path).expect("remove legacy database");
    }

    #[test]
    fn persona_plan_preserves_fields_default_and_stable_order() {
        let path = legacy_database(i64::from(LEGACY_DATABASE_SCHEMA_VERSION));
        let connection = Connection::open(&path).expect("open legacy database");
        connection
            .execute("DELETE FROM personas", [])
            .expect("clear personas");
        let first_id =
            PersonaId::from_str("00000000-0000-0000-0000-000000000001").expect("first id");
        let second_id =
            PersonaId::from_str("00000000-0000-0000-0000-000000000002").expect("second id");
        let lorebook_id =
            LorebookId::from_str("00000000-0000-0000-0000-000000000003").expect("lorebook id");
        connection
            .execute(
                "INSERT INTO personas (id,title,description,created_at,updated_at) VALUES (?1,'Second','Second description',20,21)",
                [second_id.to_string()],
            )
            .expect("insert second persona");
        connection
            .execute(
                "INSERT INTO personas (id,title,description,nickname,avatar_path,avatar_crop_x,avatar_crop_y,avatar_crop_scale,design_description,design_reference_image_ids,active_lorebook_ids,is_default,lora_name,lora_strength,created_at,updated_at) VALUES (?1,'First','First description','F','avatars/first.png',0.2,0.3,1.4,'Design notes',?2,?3,1,'portrait-style',NULL,10,15)",
                rusqlite::params![
                    first_id.to_string(),
                    "[\"image-ref-1\",\"image-ref-2\"]",
                    format!("[\"{lorebook_id}\"]")
                ],
            )
            .expect("insert first persona");
        drop(connection);

        let plan = plan_legacy_personas(&path).expect("plan personas");

        assert_eq!(plan.default_persona_id, Some(first_id));
        assert_eq!(plan.personas.len(), 2);
        let first = &plan.personas[0];
        assert_eq!(first.id, first_id);
        assert_eq!(first.title, "First");
        assert_eq!(first.description, "First description");
        assert_eq!(first.nickname.as_deref(), Some("F"));
        assert_eq!(
            first
                .avatar
                .as_ref()
                .map(|reference| reference.locator.as_str()),
            Some("avatars/first.png")
        );
        assert_eq!(
            first.avatar_crop,
            Some(LegacyCrop {
                x: 0.2,
                y: 0.3,
                scale: 1.4
            })
        );
        assert_eq!(first.design_description.as_deref(), Some("Design notes"));
        assert_eq!(
            first
                .design_references
                .iter()
                .map(|reference| reference.locator.as_str())
                .collect::<Vec<_>>(),
            ["image-ref-1", "image-ref-2"]
        );
        assert_eq!(
            first.image_recommendation,
            Some(LegacyImageRecommendation {
                model_name: "portrait-style".into(),
                strength: 0.8
            })
        );
        assert_eq!(first.active_lorebook_ids, [lorebook_id]);
        assert_eq!(first.created_at, TimestampMillis::new(10));
        assert_eq!(first.updated_at, TimestampMillis::new(15));
        assert_eq!(plan.personas[1].id, second_id);
        std::fs::remove_file(path).expect("remove legacy database");
    }

    #[test]
    fn persona_plan_rejects_malformed_fields() {
        let path = legacy_database(i64::from(LEGACY_DATABASE_SCHEMA_VERSION));
        let connection = Connection::open(&path).expect("open legacy database");
        connection
            .execute(
                "UPDATE personas SET design_reference_image_ids = 'not-json'",
                [],
            )
            .expect("corrupt persona");
        drop(connection);

        assert_eq!(
            plan_legacy_personas(&path),
            Err(LegacyDatabasePreflightError::MalformedRecord {
                table: "personas",
                field: "design_reference_image_ids"
            })
        );
        std::fs::remove_file(path).expect("remove legacy database");
    }

    #[test]
    fn persona_plan_enforces_its_record_limit_before_loading_rows() {
        let path = legacy_database(i64::from(LEGACY_DATABASE_SCHEMA_VERSION));
        let connection = Connection::open(&path).expect("open legacy database");

        assert_eq!(
            plan_legacy_personas_with_limit(&connection, 0),
            Err(LegacyDatabasePreflightError::LimitExceeded {
                table: "personas",
                limit: 0
            })
        );
        drop(connection);
        std::fs::remove_file(path).expect("remove legacy database");
    }

    #[test]
    fn lorebook_plan_preserves_roots_entries_and_stable_order() {
        let path = legacy_database(i64::from(LEGACY_DATABASE_SCHEMA_VERSION));
        let connection = Connection::open(&path).expect("open legacy database");
        let first_id =
            LorebookId::from_str("00000000-0000-0000-0000-000000000011").expect("first id");
        let second_id =
            LorebookId::from_str("00000000-0000-0000-0000-000000000012").expect("second id");
        let first_entry_id = LorebookEntryId::from_str("00000000-0000-0000-0000-000000000021")
            .expect("first entry id");
        let second_entry_id = LorebookEntryId::from_str("00000000-0000-0000-0000-000000000022")
            .expect("second entry id");
        connection
            .execute(
                "INSERT INTO lorebooks (id,name,keyword_detection_mode,created_at,updated_at) VALUES (?1,'Second book','recent_message_window',20,21)",
                [second_id.to_string()],
            )
            .expect("insert second lorebook");
        connection
            .execute(
                "INSERT INTO lorebooks (id,name,avatar_path,keyword_detection_mode,created_at,updated_at) VALUES (?1,'First book','lorebooks/first.png','latest_user_message',10,15)",
                [first_id.to_string()],
            )
            .expect("insert first lorebook");
        connection
            .execute(
                "INSERT INTO lorebook_entries (id,lorebook_id,title,enabled,always_active,keywords,case_sensitive,keyword_match_mode,content,priority,display_order,created_at,updated_at) VALUES (?1,?2,'Later',0,0,'[]',0,'literal','Later content',2,4,12,13)",
                rusqlite::params![second_entry_id.to_string(), first_id.to_string()],
            )
            .expect("insert later entry");
        connection
            .execute(
                "INSERT INTO lorebook_entries (id,lorebook_id,title,enabled,always_active,keywords,case_sensitive,keyword_match_mode,content,priority,display_order,created_at,updated_at) VALUES (?1,?2,'Earlier',1,1,'[\"hero.*\",\"city\"]',1,'regex','Earlier content',9,2,11,14)",
                rusqlite::params![first_entry_id.to_string(), first_id.to_string()],
            )
            .expect("insert earlier entry");
        drop(connection);

        let plan = plan_legacy_lorebooks(&path).expect("plan lorebooks");

        assert_eq!(plan.lorebooks.len(), 2);
        let first = &plan.lorebooks[0];
        assert_eq!(first.id, first_id);
        assert_eq!(first.name, "First book");
        assert_eq!(
            first
                .avatar
                .as_ref()
                .map(|reference| reference.locator.as_str()),
            Some("lorebooks/first.png")
        );
        assert_eq!(
            first.detection_policy,
            LegacyLorebookDetectionPolicy::LatestUserMessage
        );
        assert_eq!(first.created_at, TimestampMillis::new(10));
        assert_eq!(first.updated_at, TimestampMillis::new(15));
        assert_eq!(first.entries.len(), 2);
        let entry = &first.entries[0];
        assert_eq!(entry.id, first_entry_id);
        assert_eq!(entry.title, "Earlier");
        assert!(entry.enabled);
        assert!(entry.always_active);
        assert_eq!(entry.keywords, ["hero.*", "city"]);
        assert!(entry.case_sensitive);
        assert_eq!(entry.match_mode, LegacyKeywordMatchMode::Regex);
        assert_eq!(entry.content, "Earlier content");
        assert_eq!(entry.priority, 9);
        assert_eq!(entry.display_order, 2);
        assert_eq!(entry.created_at, TimestampMillis::new(11));
        assert_eq!(entry.updated_at, TimestampMillis::new(14));
        assert_eq!(first.entries[1].id, second_entry_id);
        assert_eq!(plan.lorebooks[1].id, second_id);
        std::fs::remove_file(path).expect("remove legacy database");
    }

    #[test]
    fn lorebook_plan_rejects_malformed_enums_and_keywords() {
        let path = legacy_database(i64::from(LEGACY_DATABASE_SCHEMA_VERSION));
        let connection = Connection::open(&path).expect("open legacy database");
        connection
            .execute(
                "INSERT INTO lorebooks (id,name,keyword_detection_mode,created_at,updated_at) VALUES (?1,'Book','unknown',1,1)",
                [LorebookId::new().to_string()],
            )
            .expect("insert malformed lorebook");
        drop(connection);

        assert_eq!(
            plan_legacy_lorebooks(&path),
            Err(LegacyDatabasePreflightError::MalformedRecord {
                table: "lorebooks",
                field: "keyword_detection_mode"
            })
        );
        let connection = Connection::open(&path).expect("reopen legacy database");
        connection
            .execute(
                "UPDATE lorebooks SET keyword_detection_mode = 'recent_message_window'",
                [],
            )
            .expect("repair detection policy");
        let lorebook_id: String = connection
            .query_row("SELECT id FROM lorebooks", [], |row| row.get(0))
            .expect("read lorebook id");
        connection
            .execute(
                "INSERT INTO lorebook_entries (id,lorebook_id,keywords,content,created_at,updated_at) VALUES (?1,?2,'not-json','Entry',1,1)",
                rusqlite::params![LorebookEntryId::new().to_string(), lorebook_id],
            )
            .expect("insert malformed entry");
        drop(connection);
        assert_eq!(
            plan_legacy_lorebooks(&path),
            Err(LegacyDatabasePreflightError::MalformedRecord {
                table: "lorebook_entries",
                field: "keywords"
            })
        );
        std::fs::remove_file(path).expect("remove legacy database");
    }

    #[test]
    fn lorebook_plan_rejects_orphan_entries() {
        let path = legacy_database(i64::from(LEGACY_DATABASE_SCHEMA_VERSION));
        let connection = Connection::open(&path).expect("open legacy database");
        connection
            .execute_batch("PRAGMA foreign_keys = OFF")
            .expect("allow corrupt legacy fixture");
        connection
            .execute(
                "INSERT INTO lorebook_entries (id,lorebook_id,content,created_at,updated_at) VALUES (?1,?2,'Orphan',1,1)",
                rusqlite::params![LorebookEntryId::new().to_string(), LorebookId::new().to_string()],
            )
            .expect("insert orphan entry");
        drop(connection);

        assert_eq!(
            plan_legacy_lorebooks(&path),
            Err(LegacyDatabasePreflightError::OrphanRecord {
                table: "lorebook_entries",
                parent_table: "lorebooks"
            })
        );
        std::fs::remove_file(path).expect("remove legacy database");
    }

    #[test]
    fn lorebook_plan_enforces_root_and_entry_bounds() {
        let path = legacy_database(i64::from(LEGACY_DATABASE_SCHEMA_VERSION));
        let connection = Connection::open(&path).expect("open legacy database");
        let lorebook_id = LorebookId::new();
        connection
            .execute(
                "INSERT INTO lorebooks (id,name,created_at,updated_at) VALUES (?1,'Book',1,1)",
                [lorebook_id.to_string()],
            )
            .expect("insert lorebook");
        assert_eq!(
            plan_legacy_lorebooks_with_limits(&connection, 0, 1, 1),
            Err(LegacyDatabasePreflightError::LimitExceeded {
                table: "lorebooks",
                limit: 0
            })
        );
        connection
            .execute(
                "INSERT INTO lorebook_entries (id,lorebook_id,content,created_at,updated_at) VALUES (?1,?2,'Entry',1,1)",
                rusqlite::params![LorebookEntryId::new().to_string(), lorebook_id.to_string()],
            )
            .expect("insert entry");
        assert_eq!(
            plan_legacy_lorebooks_with_limits(&connection, 1, 0, 1),
            Err(LegacyDatabasePreflightError::LimitExceeded {
                table: "lorebook_entries",
                limit: 0
            })
        );
        drop(connection);
        std::fs::remove_file(path).expect("remove legacy database");
    }
}
