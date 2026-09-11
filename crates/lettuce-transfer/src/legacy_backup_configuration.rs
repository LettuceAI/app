use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

use lettuce_context::{
    PromptEntryCondition, PromptEntryDraft, PromptEntryPayload, PromptEntryPosition,
    PromptEntryRole, PromptPurpose,
};
use lettuce_models::{
    CapabilityEvidence, CapabilityEvidenceSource, CapabilityStatus, ChatParameterProfile,
    CustomAuth, CustomModelList, CustomProviderConfig, CustomRoles, CustomToolChoiceMode, JsonPath,
    ModalityCapabilities, ModelCapabilities, ModelKind, ModelProfileConfig, OllamaOptions,
    OpenRouterOptions, ParameterSupport, PromptCacheRetention, PromptCaching, ProviderConfig,
    ProviderProtocol, QueryParameterName, ReasoningEffort, ReasoningMode, WireRole,
};
use lettuce_settings::{
    DynamicMemorySettings, EmbeddingSettings, GlobalSettings, HeaderName,
    LorebookGeneratorSelection, LorebookGeneratorSettings, MemoryRetrievalStrategy, MemoryRunMode,
    PureMode, SecretOwnerId, SecretPurpose, SecretRef, SecretValue,
};
use lettuce_speech::{AudioProvider, AudioProviderConfig, UserVoice};
use lettuce_types::{
    AudioProviderId, ModelProfileId, ProviderAccountId, Revision, TimestampMillis, VoiceProfileId,
};
use serde::Deserialize;
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::{
    LEGACY_MODEL_PROFILE_PLAN_LIMIT, LEGACY_PROMPT_PLAN_LIMIT, LEGACY_PROVIDER_ACCOUNT_PLAN_LIMIT,
    LegacyBackupDocumentKind, LegacyBackupInventory, LegacyModelProfileCandidate,
    LegacyPendingProviderSecret, LegacyPromptCandidate, LegacyPromptEntryCandidate,
    LegacyPromptPlan, LegacyProviderAccountCandidate, LegacyProviderAccountOrigin,
    LegacyProviderModelPlan, ProviderBackupSecret,
};

const AUDIO_PROVIDER_LIMIT: usize = 256;
const USER_VOICE_LIMIT: usize = 10_000;
const CHAT_TEMPLATE_LIMIT: usize = 10_000;
const SECRET_LIMIT: usize = 1_024;
const LEGACY_ID_NAMESPACE: Uuid = Uuid::from_u128(0x6c657474_7563_652d_6261_636b75707631);

#[derive(Debug)]
pub struct LegacyBackupConfigurationPlan {
    pub provider_models: LegacyProviderModelPlan,
    pub prompts: LegacyPromptPlan,
    pub settings: LegacyBackupSettingsCandidate,
    pub audio_providers: Vec<AudioProvider>,
    pub user_voices: Vec<UserVoice>,
    pub secrets: Vec<ProviderBackupSecret>,
    pub chat_templates: Vec<LegacyBackupChatTemplateCandidate>,
    pub notices: Vec<LegacyBackupConversionNotice>,
    pub source: LegacyBackupInventory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBackupSettingsCandidate {
    pub value: GlobalSettings,
    pub default_provider_account_id: Option<ProviderAccountId>,
    pub default_model_profile_id: Option<ModelProfileId>,
    pub default_prompt_source_id: Option<String>,
    pub dynamic_memory_model_profile_id: Option<ModelProfileId>,
    pub group_speaker_model_profile_id: Option<ModelProfileId>,
    pub lorebook_generator_model_profile_id: Option<ModelProfileId>,
    pub lorebook_generator_prompt_source_ids: LorebookGeneratorPromptSources,
    pub deprecated_system_prompt: Option<String>,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LorebookGeneratorPromptSources {
    pub planner: Option<String>,
    pub writer: Option<String>,
    pub refine: Option<String>,
    pub coherence: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBackupChatTemplateCandidate {
    pub source_id: String,
    pub character_source_id: String,
    pub name: String,
    pub scene_source_id: Option<String>,
    pub prompt_source_id: Option<String>,
    pub has_lorebook_override: bool,
    pub lorebook_source_ids: Vec<String>,
    pub messages: Vec<LegacyBackupChatTemplateMessage>,
    pub created_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBackupChatTemplateMessage {
    pub source_id: String,
    pub index: u32,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LegacyBackupConversionNoticeKind {
    Absent,
    Unsupported,
    Lossy,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LegacyBackupConversionNotice {
    pub kind: LegacyBackupConversionNoticeKind,
    pub document: LegacyBackupDocumentKind,
    pub field: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LegacyBackupConfigurationError {
    #[error("legacy backup configuration document is malformed")]
    Malformed {
        document: LegacyBackupDocumentKind,
        field: String,
    },
    #[error("legacy backup configuration exceeds its record limit")]
    LimitExceeded { document: LegacyBackupDocumentKind },
    #[error("legacy backup configuration contains an orphaned selection or ownership link")]
    Orphan {
        document: LegacyBackupDocumentKind,
        field: String,
    },
}

#[derive(Deserialize, Default)]
struct SettingsRow {
    default_provider_credential_id: Option<String>,
    default_model_id: Option<String>,
    #[serde(default)]
    app_state: Value,
    advanced_model_settings: Option<Value>,
    prompt_template_id: Option<String>,
    system_prompt: Option<String>,
    migration_version: Option<i64>,
    advanced_settings: Option<Value>,
    created_at: Option<i64>,
    updated_at: Option<i64>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct ProviderRow {
    id: String,
    provider_id: String,
    label: String,
    api_key_ref: Option<String>,
    api_key: Option<String>,
    base_url: Option<String>,
    default_model: Option<String>,
    headers: Option<String>,
    config: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct ModelRow {
    id: String,
    name: String,
    provider_id: String,
    provider_credential_id: Option<String>,
    provider_label: String,
    display_name: String,
    created_at: i64,
    model_type: Option<String>,
    input_scopes: Option<String>,
    output_scopes: Option<String>,
    advanced_model_settings: Option<String>,
    prompt_template_id: Option<String>,
    system_prompt: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct PromptRow {
    id: String,
    name: String,
    #[serde(alias = "promptType")]
    prompt_type: Option<String>,
    #[serde(default)]
    content: String,
    #[serde(default)]
    entries: Value,
    #[serde(default)]
    condense_prompt_entries: bool,
    created_at: Option<i64>,
    updated_at: Option<i64>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptEntryRow {
    id: String,
    name: String,
    role: PromptEntryRole,
    content: String,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    injection_position: PromptEntryPosition,
    #[serde(default)]
    injection_depth: u32,
    #[serde(default)]
    conditional_min_messages: Option<u32>,
    #[serde(default)]
    interval_turns: Option<u32>,
    #[serde(default)]
    system_prompt: bool,
    #[serde(default)]
    conditions: Option<PromptEntryCondition>,
    #[serde(default)]
    prompt_entry_payload: Option<PromptEntryPayload>,
}

#[derive(Deserialize)]
struct AudioProviderRow {
    id: String,
    provider_type: String,
    label: String,
    api_key: Option<String>,
    project_id: Option<String>,
    location: Option<String>,
    base_url: Option<String>,
    request_path: Option<String>,
    kokoro_variant: Option<String>,
    asset_root: Option<String>,
    created_at: i64,
    updated_at: i64,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct UserVoiceRow {
    id: String,
    provider_id: String,
    name: String,
    model_id: String,
    voice_id: String,
    prompt: Option<String>,
    created_at: i64,
    updated_at: i64,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct SecretRow {
    service: String,
    account: String,
    value: String,
    created_at: Option<i64>,
    updated_at: Option<i64>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct ChatTemplateRow {
    id: String,
    character_id: String,
    name: String,
    scene_id: Option<String>,
    prompt_template_id: Option<String>,
    lorebook_ids_override: Option<String>,
    created_at: i64,
    #[serde(default)]
    messages: Vec<ChatTemplateMessageRow>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct ChatTemplateMessageRow {
    id: String,
    idx: Option<i64>,
    role: String,
    content: String,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

struct AudioConfigurationPlan {
    providers: Vec<AudioProvider>,
    voices: Vec<UserVoice>,
    secrets: Vec<ProviderBackupSecret>,
}

pub fn plan_legacy_backup_configuration(
    source: LegacyBackupInventory,
) -> Result<LegacyBackupConfigurationPlan, LegacyBackupConfigurationError> {
    let mut notices = Vec::new();
    let settings: SettingsRow =
        optional_document(&source, LegacyBackupDocumentKind::Settings, &mut notices)?
            .unwrap_or_default();
    report_extra(
        LegacyBackupDocumentKind::Settings,
        "",
        &settings.extra,
        &mut notices,
    );
    let settings_candidate = map_settings(&settings, &mut notices)?;
    let provider_rows: Vec<ProviderRow> = array_document(
        &source,
        LegacyBackupDocumentKind::ProviderCredentials,
        LEGACY_PROVIDER_ACCOUNT_PLAN_LIMIT as usize,
        &mut notices,
    )?;
    let model_rows: Vec<ModelRow> = array_document(
        &source,
        LegacyBackupDocumentKind::Models,
        LEGACY_MODEL_PROFILE_PLAN_LIMIT as usize,
        &mut notices,
    )?;
    let prompt_rows: Vec<PromptRow> = array_document(
        &source,
        LegacyBackupDocumentKind::PromptTemplates,
        LEGACY_PROMPT_PLAN_LIMIT as usize,
        &mut notices,
    )?;
    let secret_rows: Vec<SecretRow> = array_document(
        &source,
        LegacyBackupDocumentKind::Secrets,
        SECRET_LIMIT,
        &mut notices,
    )?;
    let audio_rows: Vec<AudioProviderRow> = array_document(
        &source,
        LegacyBackupDocumentKind::AudioProviders,
        AUDIO_PROVIDER_LIMIT,
        &mut notices,
    )?;
    let voice_rows: Vec<UserVoiceRow> = array_document(
        &source,
        LegacyBackupDocumentKind::UserVoices,
        USER_VOICE_LIMIT,
        &mut notices,
    )?;
    let chat_rows: Vec<ChatTemplateRow> = array_document(
        &source,
        LegacyBackupDocumentKind::ChatTemplates,
        CHAT_TEMPLATE_LIMIT,
        &mut notices,
    )?;

    let mut provider_models =
        map_provider_models(provider_rows, model_rows, &settings_candidate, &mut notices)?;
    let prompts = map_prompts(prompt_rows, &settings_candidate, &mut notices)?;
    provider_models.default_provider_account_id = settings_candidate.default_provider_account_id;
    provider_models.default_model_profile_id = settings_candidate.default_model_profile_id;
    validate_selections(&settings_candidate, &provider_models, &prompts)?;
    let audio = map_audio(audio_rows, voice_rows, &mut notices)?;
    let audio_providers = audio.providers;
    let user_voices = audio.voices;
    let mut secrets = audio.secrets;
    secrets.extend(map_provider_secrets(
        &provider_models,
        &source,
        secret_rows,
        &mut notices,
    )?);
    for secret in &secrets {
        let (owner, pending) = match &secret.purpose {
            SecretPurpose::ProviderApiKey { owner } => {
                (*owner, LegacyPendingProviderSecret::ApiKey)
            }
            SecretPurpose::ProviderSecretHeader { owner, name } => (
                *owner,
                LegacyPendingProviderSecret::Header { name: name.clone() },
            ),
            _ => continue,
        };
        let provider = provider_models
            .provider_accounts
            .iter_mut()
            .find(|provider| provider.secret_owner_id == owner)
            .ok_or_else(|| orphan(LegacyBackupDocumentKind::Secrets, "purpose.owner"))?;
        if !provider.pending_secrets.contains(&pending) {
            provider.pending_secrets.push(pending);
            provider.pending_secrets.sort();
        }
    }
    secrets.sort_by_key(|secret| secret.reference);
    if secrets
        .windows(2)
        .any(|pair| pair[0].reference == pair[1].reference)
    {
        return Err(malformed(LegacyBackupDocumentKind::Secrets, "reference"));
    }
    let chat_templates = map_chat_templates(chat_rows, &source, &prompts, &mut notices)?;
    notices.sort();
    notices.dedup();
    Ok(LegacyBackupConfigurationPlan {
        provider_models,
        prompts,
        settings: settings_candidate,
        audio_providers,
        user_voices,
        secrets,
        chat_templates,
        notices,
        source,
    })
}

fn optional_document<T: for<'de> Deserialize<'de>>(
    source: &LegacyBackupInventory,
    kind: LegacyBackupDocumentKind,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Option<T>, LegacyBackupConfigurationError> {
    let Some(document) = source
        .documents
        .iter()
        .find(|document| document.kind == kind)
    else {
        notices.push(notice(LegacyBackupConversionNoticeKind::Absent, kind, "$"));
        return Ok(None);
    };
    serde_json::from_slice(&document.bytes)
        .map(Some)
        .map_err(|_| malformed(kind, "$"))
}

fn array_document<T: for<'de> Deserialize<'de>>(
    source: &LegacyBackupInventory,
    kind: LegacyBackupDocumentKind,
    limit: usize,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<T>, LegacyBackupConfigurationError> {
    let rows = optional_document::<Vec<T>>(source, kind, notices)?.unwrap_or_default();
    if rows.len() > limit {
        return Err(LegacyBackupConfigurationError::LimitExceeded { document: kind });
    }
    Ok(rows)
}

fn map_settings(
    row: &SettingsRow,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<LegacyBackupSettingsCandidate, LegacyBackupConfigurationError> {
    let created = row.created_at.unwrap_or(0);
    let updated = row.updated_at.or(row.created_at).unwrap_or(0);
    if created < 0 || updated < created {
        return Err(malformed(LegacyBackupDocumentKind::Settings, "timestamps"));
    }
    let app = object_or_empty(
        &row.app_state,
        LegacyBackupDocumentKind::Settings,
        "app_state",
    )?;
    let advanced_value = row.advanced_settings.as_ref().unwrap_or(&Value::Null);
    let advanced = object_or_empty(
        advanced_value,
        LegacyBackupDocumentKind::Settings,
        "advanced_settings",
    )?;
    let pure_mode = match app.get("pureModeLevel").and_then(Value::as_str) {
        Some("off") => PureMode::Off,
        Some("strict") => PureMode::Strict,
        Some("low") => {
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Lossy,
                LegacyBackupDocumentKind::Settings,
                "app_state.pureModeLevel",
            ));
            PureMode::Standard
        }
        Some("standard") => PureMode::Standard,
        None => match app.get("pureModeEnabled").and_then(Value::as_bool) {
            Some(false) => {
                notices.push(notice(
                    LegacyBackupConversionNoticeKind::Lossy,
                    LegacyBackupDocumentKind::Settings,
                    "app_state.pureModeEnabled",
                ));
                PureMode::Off
            }
            Some(true) => {
                notices.push(notice(
                    LegacyBackupConversionNoticeKind::Lossy,
                    LegacyBackupDocumentKind::Settings,
                    "app_state.pureModeEnabled",
                ));
                PureMode::Standard
            }
            None => {
                notices.push(notice(
                    LegacyBackupConversionNoticeKind::Absent,
                    LegacyBackupDocumentKind::Settings,
                    "app_state.pureModeLevel",
                ));
                PureMode::Standard
            }
        },
        Some(_) => {
            return Err(malformed(
                LegacyBackupDocumentKind::Settings,
                "app_state.pureModeLevel",
            ));
        }
    };
    let analytics_enabled = optional_bool_value(
        app,
        "analyticsEnabled",
        true,
        LegacyBackupDocumentKind::Settings,
    )?;
    let update_checks_enabled = optional_bool_value(
        advanced,
        "appUpdateChecksEnabled",
        true,
        LegacyBackupDocumentKind::Settings,
    )?;
    let dynamic_memory = map_dynamic_memory(
        advanced.get("dynamicMemory"),
        "advanced_settings.dynamicMemory",
        notices,
    )?;
    let group_dynamic_memory = advanced
        .get("groupDynamicMemory")
        .map(|value| {
            map_dynamic_memory(Some(value), "advanced_settings.groupDynamicMemory", notices)
        })
        .transpose()?;
    let default_provider_account_id = parse_optional_id(
        row.default_provider_credential_id.as_deref(),
        LegacyBackupDocumentKind::Settings,
        "default_provider_credential_id",
    )?;
    let default_model_profile_id = parse_optional_id(
        row.default_model_id.as_deref(),
        LegacyBackupDocumentKind::Settings,
        "default_model_id",
    )?;
    let generator_model = advanced_id(advanced, "lorebookGeneratorModelId")?;
    let generator_prompts = LorebookGeneratorPromptSources {
        planner: normalized_string(advanced, "lorebookGeneratorPlannerPromptTemplateId")?,
        writer: normalized_string(advanced, "lorebookGeneratorWriterPromptTemplateId")?,
        refine: normalized_string(advanced, "lorebookGeneratorRefinePromptTemplateId")?,
        coherence: normalized_string(advanced, "lorebookGeneratorCoherencePromptTemplateId")?,
    };
    let target_count = optional_u32(advanced, "lorebookGeneratorDefaultTargetCount")?;
    let max_output_tokens = optional_u32(advanced, "lorebookGeneratorMaxTokens")?;
    if target_count.is_some_and(|value| !(5..=50).contains(&value)) {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Lossy,
            LegacyBackupDocumentKind::Settings,
            "advanced_settings.lorebookGeneratorDefaultTargetCount",
        ));
    }
    if max_output_tokens.is_some_and(|value| !(256..=32_768).contains(&value)) {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Lossy,
            LegacyBackupDocumentKind::Settings,
            "advanced_settings.lorebookGeneratorMaxTokens",
        ));
    }
    let mapped_advanced = BTreeSet::from([
        "appUpdateChecksEnabled",
        "dynamicMemory",
        "groupDynamicMemory",
        "embeddingDimensions",
        "summarisationModelId",
        "groupSpeakerSelectionModelId",
        "lorebookGeneratorModelId",
        "lorebookGeneratorDefaultTargetCount",
        "lorebookGeneratorMaxTokens",
        "lorebookGeneratorPlannerPromptTemplateId",
        "lorebookGeneratorWriterPromptTemplateId",
        "lorebookGeneratorRefinePromptTemplateId",
        "lorebookGeneratorCoherencePromptTemplateId",
    ]);
    for field in advanced
        .keys()
        .filter(|field| !mapped_advanced.contains(field.as_str()))
    {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Unsupported,
            LegacyBackupDocumentKind::Settings,
            format!("advanced_settings.{field}"),
        ));
    }
    for field in app.keys().filter(|field| {
        !matches!(
            field.as_str(),
            "pureModeLevel" | "pureModeEnabled" | "analyticsEnabled"
        )
    }) {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Unsupported,
            LegacyBackupDocumentKind::Settings,
            format!("app_state.{field}"),
        ));
    }
    if row
        .advanced_model_settings
        .as_ref()
        .is_some_and(|value| !value.is_null())
    {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Unsupported,
            LegacyBackupDocumentKind::Settings,
            "advanced_model_settings",
        ));
    }
    if row.migration_version.is_some() {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Unsupported,
            LegacyBackupDocumentKind::Settings,
            "migration_version",
        ));
    }
    Ok(LegacyBackupSettingsCandidate {
        value: GlobalSettings {
            pure_mode,
            analytics_enabled,
            update_checks_enabled,
            lorebook_generator: LorebookGeneratorSettings {
                selection: LorebookGeneratorSelection::default(),
                default_target_count: target_count,
                max_output_tokens,
            },
            dynamic_memory,
            group_dynamic_memory,
            embedding: EmbeddingSettings {
                dimensions: optional_u32(advanced, "embeddingDimensions")?
                    .and_then(|value| u16::try_from(value).ok()),
            },
        },
        default_provider_account_id,
        default_model_profile_id,
        default_prompt_source_id: normalize_option(row.prompt_template_id.clone()),
        dynamic_memory_model_profile_id: advanced_id(advanced, "summarisationModelId")?,
        group_speaker_model_profile_id: advanced_id(advanced, "groupSpeakerSelectionModelId")?,
        lorebook_generator_model_profile_id: generator_model,
        lorebook_generator_prompt_source_ids: generator_prompts,
        deprecated_system_prompt: normalize_option(row.system_prompt.clone()),
        created_at: TimestampMillis::new(created),
        updated_at: TimestampMillis::new(updated),
    })
}

fn map_dynamic_memory(
    value: Option<&Value>,
    path: &str,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<DynamicMemorySettings, LegacyBackupConfigurationError> {
    let Some(value) = value else {
        return Ok(DynamicMemorySettings::default());
    };
    let object = object_or_empty(value, LegacyBackupDocumentKind::Settings, path)?;
    let mut result = DynamicMemorySettings::default();
    result.enabled = optional_bool_value(
        object,
        "enabled",
        result.enabled,
        LegacyBackupDocumentKind::Settings,
    )?;
    if let Some(interval) = optional_u32(object, "summaryMessageInterval")? {
        if interval == 0 {
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Lossy,
                LegacyBackupDocumentKind::Settings,
                format!("{path}.summaryMessageInterval"),
            ));
        }
        result.summary_message_interval = interval.max(1);
    }
    result.run_mode = match object.get("runMode").and_then(Value::as_str) {
        Some("manual") => MemoryRunMode::Manual,
        Some("askFirst") => MemoryRunMode::AskFirst,
        _ => MemoryRunMode::Auto,
    };
    result.recursive_memory_loops = optional_bool_value(
        object,
        "recursiveMemoryLoops",
        result.recursive_memory_loops,
        LegacyBackupDocumentKind::Settings,
    )?;
    result.recursive_memory_loop_hard_cap = optional_u32(object, "recursiveMemoryLoopHardCap")?
        .map_or(result.recursive_memory_loop_hard_cap, |cap| cap.max(1));
    result.decay_rate_basis_points = threshold_basis_points(
        object.get("decayRate"),
        &format!("{path}.decayRate"),
        result.decay_rate_basis_points,
        notices,
    )?;
    result.delete_confidence_basis_points = threshold_basis_points(
        object.get("deleteConfidenceDefault"),
        &format!("{path}.deleteConfidenceDefault"),
        result.delete_confidence_basis_points,
        notices,
    )?;
    result.max_hard_delete_ratio_basis_points = threshold_basis_points(
        object.get("maxHardDeleteRatioPerCycle"),
        &format!("{path}.maxHardDeleteRatioPerCycle"),
        result.max_hard_delete_ratio_basis_points,
        notices,
    )?;
    result.max_entries = optional_u32(object, "maxEntries")?.unwrap_or(result.max_entries);
    result.retrieval_limit = optional_u32(object, "retrievalLimit")?
        .map(|value| {
            u16::try_from(value).map_err(|_| {
                malformed(
                    LegacyBackupDocumentKind::Settings,
                    format!("{path}.retrievalLimit"),
                )
            })
        })
        .transpose()?
        .unwrap_or(result.retrieval_limit);
    result.hot_memory_token_budget =
        optional_u32(object, "hotMemoryTokenBudget")?.unwrap_or(result.hot_memory_token_budget);
    result.context_enrichment_enabled = optional_bool_value(
        object,
        "contextEnrichmentEnabled",
        result.context_enrichment_enabled,
        LegacyBackupDocumentKind::Settings,
    )?;
    result.min_similarity_basis_points = threshold_basis_points(
        object.get("minSimilarityThreshold"),
        &format!("{path}.minSimilarityThreshold"),
        result.min_similarity_basis_points,
        notices,
    )?;
    result.cold_threshold_basis_points = threshold_basis_points(
        object.get("coldThreshold"),
        &format!("{path}.coldThreshold"),
        result.cold_threshold_basis_points,
        notices,
    )?;
    result.retrieval_strategy = match object.get("retrievalStrategy").and_then(Value::as_str) {
        None | Some("smart") => MemoryRetrievalStrategy::Smart,
        Some("cosine") => MemoryRetrievalStrategy::Cosine,
        Some(_) => {
            return Err(malformed(
                LegacyBackupDocumentKind::Settings,
                format!("{path}.retrievalStrategy"),
            ));
        }
    };
    for field in object.keys().filter(|field| {
        !matches!(
            field.as_str(),
            "enabled"
                | "summaryMessageInterval"
                | "runMode"
                | "recursiveMemoryLoops"
                | "recursiveMemoryLoopHardCap"
                | "decayRate"
                | "deleteConfidenceDefault"
                | "maxHardDeleteRatioPerCycle"
                | "maxEntries"
                | "retrievalLimit"
                | "hotMemoryTokenBudget"
                | "contextEnrichmentEnabled"
                | "minSimilarityThreshold"
                | "coldThreshold"
                | "retrievalStrategy"
        )
    }) {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Unsupported,
            LegacyBackupDocumentKind::Settings,
            format!("{path}.{field}"),
        ));
    }
    Ok(result)
}

fn threshold_basis_points(
    value: Option<&Value>,
    field: &str,
    default: u16,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<u16, LegacyBackupConfigurationError> {
    let Some(value) = value else {
        return Ok(default);
    };
    let value = value
        .as_f64()
        .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
        .ok_or_else(|| malformed(LegacyBackupDocumentKind::Settings, field))?;
    let scaled = value * 10_000.0;
    if (scaled - scaled.round()).abs() > 1e-9 {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Lossy,
            LegacyBackupDocumentKind::Settings,
            field,
        ));
    }
    Ok(scaled.round() as u16)
}

fn map_provider_models(
    provider_rows: Vec<ProviderRow>,
    model_rows: Vec<ModelRow>,
    settings: &LegacyBackupSettingsCandidate,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<LegacyProviderModelPlan, LegacyBackupConfigurationError> {
    let mut providers = Vec::with_capacity(provider_rows.len());
    let mut source_provider_ids = BTreeMap::new();
    for (index, row) in provider_rows.into_iter().enumerate() {
        report_extra(
            LegacyBackupDocumentKind::ProviderCredentials,
            &format!("[{index}]."),
            &row.extra,
            notices,
        );
        require_nonblank(&row.id, LegacyBackupDocumentKind::ProviderCredentials, "id")?;
        require_nonblank(
            &row.provider_id,
            LegacyBackupDocumentKind::ProviderCredentials,
            "provider_id",
        )?;
        require_nonblank(
            &row.label,
            LegacyBackupDocumentKind::ProviderCredentials,
            "label",
        )?;
        let id = canonical_provider_id(
            &row.id,
            notices,
            LegacyBackupDocumentKind::ProviderCredentials,
            &format!("[{index}].id"),
        );
        if source_provider_ids.insert(row.id.clone(), id).is_some()
            || providers
                .iter()
                .any(|provider: &LegacyProviderAccountCandidate| provider.id == id)
        {
            return Err(malformed(
                LegacyBackupDocumentKind::ProviderCredentials,
                "id",
            ));
        }
        let protocol = legacy_provider_protocol(&row.provider_id).ok_or_else(|| {
            malformed(
                LegacyBackupDocumentKind::ProviderCredentials,
                format!("[{index}].provider_id"),
            )
        })?;
        let config_value = parse_string_object(
            row.config.as_deref(),
            LegacyBackupDocumentKind::ProviderCredentials,
            &format!("[{index}].config"),
        )?;
        let streaming_enabled = optional_bool_value(
            &config_value,
            "streamingEnabled",
            true,
            LegacyBackupDocumentKind::ProviderCredentials,
        )?;
        let allow_invalid_tls = optional_bool_value(
            &config_value,
            "allowInvalidTls",
            false,
            LegacyBackupDocumentKind::ProviderCredentials,
        )?;
        let (config, mapped) = legacy_provider_config(&row.provider_id, &config_value)?;
        let deferred_config_fields = config_value
            .keys()
            .filter(|field| !mapped.contains(&field.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        for field in &deferred_config_fields {
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Unsupported,
                LegacyBackupDocumentKind::ProviderCredentials,
                format!("[{index}].config.{field}"),
            ));
        }
        let mut pending_secrets = Vec::new();
        if row
            .api_key
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
        {
            pending_secrets.push(LegacyPendingProviderSecret::ApiKey);
        }
        let headers = parse_headers(row.headers.as_deref(), index)?;
        pending_secrets.extend(
            headers
                .keys()
                .cloned()
                .map(|name| LegacyPendingProviderSecret::Header { name }),
        );
        if row
            .api_key_ref
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
        {
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Unsupported,
                LegacyBackupDocumentKind::ProviderCredentials,
                format!("[{index}].api_key_ref"),
            ));
        }
        providers.push(LegacyProviderAccountCandidate {
            id,
            origin: LegacyProviderAccountOrigin::Stored,
            secret_owner_id: SecretOwnerId::from_uuid(id.as_uuid()),
            provider_kind: row.provider_id,
            protocol,
            label: row.label,
            endpoint: normalize_option(row.base_url),
            enabled: true,
            streaming_enabled,
            allow_invalid_tls,
            default_model: normalize_option(row.default_model),
            config,
            pending_secrets,
            deferred_config_fields,
            created_at: settings.created_at,
            updated_at: settings.updated_at,
        });
    }
    let has_llama = model_rows
        .iter()
        .any(|row| row.provider_id.eq_ignore_ascii_case("llamacpp"));
    if has_llama {
        let id =
            ProviderAccountId::from_uuid(Uuid::new_v5(&LEGACY_ID_NAMESPACE, b"builtin:llamacpp"));
        providers.push(LegacyProviderAccountCandidate {
            id,
            origin: LegacyProviderAccountOrigin::BuiltInLlamaCpp,
            secret_owner_id: SecretOwnerId::from_uuid(id.as_uuid()),
            provider_kind: "llamacpp".into(),
            protocol: ProviderProtocol::LlamaCpp,
            label: "llama.cpp (Local)".into(),
            endpoint: None,
            enabled: true,
            streaming_enabled: true,
            allow_invalid_tls: false,
            default_model: None,
            config: ProviderConfig::Standard,
            pending_secrets: Vec::new(),
            deferred_config_fields: Vec::new(),
            created_at: settings.created_at,
            updated_at: settings.updated_at,
        });
    }
    let mut models = Vec::with_capacity(model_rows.len());
    for (index, row) in model_rows.into_iter().enumerate() {
        report_extra(
            LegacyBackupDocumentKind::Models,
            &format!("[{index}]."),
            &row.extra,
            notices,
        );
        require_nonblank(&row.id, LegacyBackupDocumentKind::Models, "id")?;
        require_nonblank(&row.name, LegacyBackupDocumentKind::Models, "name")?;
        require_nonblank(
            &row.provider_id,
            LegacyBackupDocumentKind::Models,
            "provider_id",
        )?;
        require_nonblank(
            &row.display_name,
            LegacyBackupDocumentKind::Models,
            "display_name",
        )?;
        let id = canonical_model_id(&row.id, notices, &format!("[{index}].id"));
        if models
            .iter()
            .any(|model: &LegacyModelProfileCandidate| model.id == id)
        {
            return Err(malformed(LegacyBackupDocumentKind::Models, "id"));
        }
        let explicit = row
            .provider_credential_id
            .as_ref()
            .and_then(|value| source_provider_ids.get(value))
            .copied();
        let provider_id = resolve_provider(
            &providers,
            &row.provider_id,
            explicit,
            &row.provider_label,
            &row.name,
            settings.default_provider_account_id,
        )?;
        let input = legacy_scopes(
            row.input_scopes.as_deref(),
            row.model_type.as_deref(),
            true,
            index,
            notices,
        )?;
        let output = legacy_scopes(
            row.output_scopes.as_deref(),
            row.model_type.as_deref(),
            false,
            index,
            notices,
        )?;
        let advanced = parse_string_object(
            row.advanced_model_settings.as_deref(),
            LegacyBackupDocumentKind::Models,
            &format!("[{index}].advanced_model_settings"),
        )?;
        let (chat_parameters, mapped) = legacy_chat_parameters(&row.provider_id, &advanced)?;
        let deferred = advanced
            .keys()
            .filter(|field| !mapped.contains(&field.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        for field in &deferred {
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Unsupported,
                LegacyBackupDocumentKind::Models,
                format!("[{index}].advanced_model_settings.{field}"),
            ));
        }
        let account = providers
            .iter()
            .find(|provider| provider.id == provider_id)
            .ok_or_else(|| {
                orphan(
                    LegacyBackupDocumentKind::Models,
                    format!("[{index}].provider_credential_id"),
                )
            })?;
        let config = ModelProfileConfig {
            chat_parameters,
            lorebook_generator_parameters: Default::default(),
            capabilities: ModelCapabilities {
                format_version: lettuce_models::MODEL_CAPABILITIES_FORMAT_VERSION,
                evidence: CapabilityEvidence {
                    source: CapabilityEvidenceSource::UserOverride,
                    source_version: 1,
                    observed_at: TimestampMillis::new(row.created_at.max(0)),
                },
                input_modalities: input,
                output_modalities: output,
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
            },
        };
        config.chat_parameters.validate().map_err(|_| {
            malformed(
                LegacyBackupDocumentKind::Models,
                format!("[{index}].advanced_model_settings"),
            )
        })?;
        config.capabilities.validate().map_err(|_| {
            malformed(
                LegacyBackupDocumentKind::Models,
                format!("[{index}].capabilities"),
            )
        })?;
        let kind = match row.model_type.as_deref().unwrap_or("chat") {
            "chat" | "multimodel" => ModelKind::Chat,
            "imagegeneration" => ModelKind::Image,
            _ => {
                return Err(malformed(
                    LegacyBackupDocumentKind::Models,
                    format!("[{index}].model_type"),
                ));
            }
        };
        if row.created_at < 0 {
            return Err(malformed(
                LegacyBackupDocumentKind::Models,
                format!("[{index}].created_at"),
            ));
        }
        models.push(LegacyModelProfileCandidate {
            id,
            provider_account_id: provider_id,
            source_provider_kind: row.provider_id,
            source_provider_label: row.provider_label,
            external_model_id: row.name,
            display_name: row.display_name,
            kind,
            config,
            prompt_template_id: normalize_option(row.prompt_template_id),
            deprecated_system_prompt: normalize_option(row.system_prompt),
            deferred_advanced_fields: deferred,
            created_at: TimestampMillis::new(row.created_at),
        });
    }
    providers.sort_by(|left, right| {
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
    models.sort_by_key(|model| (model.created_at, model.id));
    Ok(LegacyProviderModelPlan {
        provider_accounts: providers,
        model_profiles: models,
        default_provider_account_id: None,
        default_model_profile_id: None,
    })
}

fn map_prompts(
    rows: Vec<PromptRow>,
    settings: &LegacyBackupSettingsCandidate,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<LegacyPromptPlan, LegacyBackupConfigurationError> {
    let mut prompts = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        report_extra(
            LegacyBackupDocumentKind::PromptTemplates,
            &format!("[{index}]."),
            &row.extra,
            notices,
        );
        require_nonblank(&row.id, LegacyBackupDocumentKind::PromptTemplates, "id")?;
        require_nonblank(&row.name, LegacyBackupDocumentKind::PromptTemplates, "name")?;
        if prompts
            .iter()
            .any(|prompt: &LegacyPromptCandidate| prompt.source_id == row.id)
        {
            return Err(malformed(LegacyBackupDocumentKind::PromptTemplates, "id"));
        }
        let mut purpose: PromptPurpose = serde_json::from_value(Value::String(
            row.prompt_type.unwrap_or_else(|| "undefined".into()),
        ))
        .map_err(|_| {
            malformed(
                LegacyBackupDocumentKind::PromptTemplates,
                format!("[{index}].prompt_type"),
            )
        })?;
        if purpose == PromptPurpose::Undefined {
            purpose = PromptPurpose::DirectChat;
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Lossy,
                LegacyBackupDocumentKind::PromptTemplates,
                format!("[{index}].prompt_type"),
            ));
        }
        let entry_value = match row.entries {
            Value::String(value) => serde_json::from_str(&value).map_err(|_| {
                malformed(
                    LegacyBackupDocumentKind::PromptTemplates,
                    format!("[{index}].entries"),
                )
            })?,
            value => value,
        };
        let entry_rows: Vec<PromptEntryRow> =
            serde_json::from_value(entry_value).map_err(|_| {
                malformed(
                    LegacyBackupDocumentKind::PromptTemplates,
                    format!("[{index}].entries"),
                )
            })?;
        let mut entries = entry_rows
            .into_iter()
            .map(|entry| {
                let source_id = entry.id;
                let draft = PromptEntryDraft {
                    built_in_entry_key: None,
                    name: entry.name,
                    role: entry.role,
                    content: entry.content,
                    enabled: entry.enabled,
                    injection_position: entry.injection_position,
                    depth: entry.injection_depth,
                    conditional_min_messages: entry.conditional_min_messages,
                    interval_turns: entry.interval_turns,
                    system_prompt: entry.system_prompt,
                    conditions: entry.conditions,
                    payload: entry.prompt_entry_payload,
                };
                if source_id.trim().is_empty() || draft.validate().is_err() {
                    return Err(malformed(
                        LegacyBackupDocumentKind::PromptTemplates,
                        format!("[{index}].entries"),
                    ));
                }
                Ok(LegacyPromptEntryCandidate { source_id, draft })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if entries
            .iter()
            .map(|entry| &entry.source_id)
            .collect::<BTreeSet<_>>()
            .len()
            != entries.len()
        {
            return Err(malformed(
                LegacyBackupDocumentKind::PromptTemplates,
                format!("[{index}].entries"),
            ));
        }
        if entries.is_empty() && !row.content.trim().is_empty() {
            entries.push(LegacyPromptEntryCandidate {
                source_id: "entry_system".into(),
                draft: PromptEntryDraft {
                    built_in_entry_key: None,
                    name: "System Prompt".into(),
                    role: PromptEntryRole::System,
                    content: row.content,
                    enabled: true,
                    injection_position: PromptEntryPosition::Relative,
                    depth: 0,
                    conditional_min_messages: None,
                    interval_turns: None,
                    system_prompt: true,
                    conditions: None,
                    payload: None,
                },
            });
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Lossy,
                LegacyBackupDocumentKind::PromptTemplates,
                format!("[{index}].content"),
            ));
        } else if !entries.is_empty() && !row.content.trim().is_empty() {
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Unsupported,
                LegacyBackupDocumentKind::PromptTemplates,
                format!("[{index}].content"),
            ));
        }
        let created = row.created_at.unwrap_or(0);
        let updated = row.updated_at.or(row.created_at).unwrap_or(0);
        if created < 0 || updated < created {
            return Err(malformed(
                LegacyBackupDocumentKind::PromptTemplates,
                format!("[{index}].timestamps"),
            ));
        }
        prompts.push(LegacyPromptCandidate {
            source_id: row.id,
            name: row.name,
            purpose,
            entries,
            condense: row.condense_prompt_entries,
            created_at: TimestampMillis::new(created),
            updated_at: TimestampMillis::new(updated),
        });
    }
    prompts.sort_by(|left, right| left.source_id.cmp(&right.source_id));
    Ok(LegacyPromptPlan {
        prompts,
        default_prompt_source_id: settings.default_prompt_source_id.clone(),
        deprecated_system_prompt: settings.deprecated_system_prompt.clone(),
    })
}

fn map_audio(
    rows: Vec<AudioProviderRow>,
    voices: Vec<UserVoiceRow>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<AudioConfigurationPlan, LegacyBackupConfigurationError> {
    let mut providers = Vec::with_capacity(rows.len());
    let mut source_ids = BTreeMap::new();
    let mut secrets = Vec::new();
    for (index, row) in rows.into_iter().enumerate() {
        report_extra(
            LegacyBackupDocumentKind::AudioProviders,
            &format!("[{index}]."),
            &row.extra,
            notices,
        );
        require_nonblank(&row.id, LegacyBackupDocumentKind::AudioProviders, "id")?;
        require_nonblank(
            &row.label,
            LegacyBackupDocumentKind::AudioProviders,
            "label",
        )?;
        if row.created_at < 0 || row.updated_at < row.created_at {
            return Err(malformed(
                LegacyBackupDocumentKind::AudioProviders,
                format!("[{index}].timestamps"),
            ));
        }
        report_unused_audio_fields(&row, index, notices);
        let id = canonical_audio_provider_id(&row.id, notices, &format!("[{index}].id"));
        if source_ids.insert(row.id, id).is_some()
            || providers
                .iter()
                .any(|provider: &AudioProvider| provider.id == id)
        {
            return Err(malformed(LegacyBackupDocumentKind::AudioProviders, "id"));
        }
        let owner = SecretOwnerId::from_uuid(id.as_uuid());
        let config = match row.provider_type.as_str() {
            "gemini_tts" => AudioProviderConfig::Gemini {
                project_id: normalize_option(row.project_id),
                location: normalize_option(row.location).unwrap_or_else(|| "us-central1".into()),
            },
            "elevenlabs" => AudioProviderConfig::Elevenlabs,
            "fish_tts" => AudioProviderConfig::FishTts,
            "fish_speech" => AudioProviderConfig::FishSpeech {
                base_url: normalize_option(row.base_url),
                request_path: normalize_option(row.request_path),
            },
            "openai_tts" => AudioProviderConfig::OpenAiCompatible {
                base_url: normalize_option(row.base_url),
                request_path: normalize_option(row.request_path),
            },
            "kokoro" => AudioProviderConfig::Kokoro {
                variant: normalize_option(row.kokoro_variant),
            },
            _ => {
                return Err(malformed(
                    LegacyBackupDocumentKind::AudioProviders,
                    format!("[{index}].provider_type"),
                ));
            }
        };
        if row
            .asset_root
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
        {
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Unsupported,
                LegacyBackupDocumentKind::AudioProviders,
                format!("[{index}].asset_root"),
            ));
        }
        let api_key = normalize_option(row.api_key);
        if matches!(config, AudioProviderConfig::Kokoro { .. }) && api_key.is_some() {
            return Err(malformed(
                LegacyBackupDocumentKind::AudioProviders,
                format!("[{index}].api_key"),
            ));
        }
        let reference = api_key
            .as_ref()
            .map(|_| deterministic_secret_ref(&format!("audio:{}:api", id)));
        if let (Some(value), Some(reference)) = (api_key, reference) {
            secrets.push(ProviderBackupSecret {
                reference,
                purpose: SecretPurpose::AudioApiKey { owner },
                generation: 1,
                value: SecretValue::new(value).map_err(|_| {
                    malformed(
                        LegacyBackupDocumentKind::AudioProviders,
                        format!("[{index}].api_key"),
                    )
                })?,
            });
        }
        let provider = AudioProvider {
            id,
            secret_owner_id: owner,
            label: row.label,
            api_key_ref: reference,
            config,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(row.created_at),
            updated_at: TimestampMillis::new(row.updated_at),
        };
        provider.validate().map_err(|_| {
            malformed(
                LegacyBackupDocumentKind::AudioProviders,
                format!("[{index}]"),
            )
        })?;
        providers.push(provider);
    }
    let mut mapped_voices = Vec::with_capacity(voices.len());
    for (index, row) in voices.into_iter().enumerate() {
        report_extra(
            LegacyBackupDocumentKind::UserVoices,
            &format!("[{index}]."),
            &row.extra,
            notices,
        );
        let provider_id = source_ids.get(&row.provider_id).copied().ok_or_else(|| {
            orphan(
                LegacyBackupDocumentKind::UserVoices,
                format!("[{index}].provider_id"),
            )
        })?;
        if row.created_at < 0 || row.updated_at < row.created_at {
            return Err(malformed(
                LegacyBackupDocumentKind::UserVoices,
                format!("[{index}].timestamps"),
            ));
        }
        let id = canonical_voice_id(&row.id, notices, &format!("[{index}].id"));
        if mapped_voices.iter().any(|voice: &UserVoice| voice.id == id) {
            return Err(malformed(LegacyBackupDocumentKind::UserVoices, "id"));
        }
        let voice = UserVoice {
            id,
            provider_id,
            name: row.name,
            model_id: row.model_id,
            voice_id: row.voice_id,
            prompt: normalize_option(row.prompt),
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(row.created_at),
            updated_at: TimestampMillis::new(row.updated_at),
        };
        voice
            .validate()
            .map_err(|_| malformed(LegacyBackupDocumentKind::UserVoices, format!("[{index}]")))?;
        mapped_voices.push(voice);
    }
    providers.sort_by_key(|provider| provider.id);
    mapped_voices.sort_by_key(|voice| voice.id);
    Ok(AudioConfigurationPlan {
        providers,
        voices: mapped_voices,
        secrets,
    })
}

fn map_provider_secrets(
    providers: &LegacyProviderModelPlan,
    source: &LegacyBackupInventory,
    rows: Vec<SecretRow>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<ProviderBackupSecret>, LegacyBackupConfigurationError> {
    let provider_document: Vec<ProviderRow> = source
        .documents
        .iter()
        .find(|document| document.kind == LegacyBackupDocumentKind::ProviderCredentials)
        .map(|document| {
            serde_json::from_slice(&document.bytes)
                .map_err(|_| malformed(LegacyBackupDocumentKind::ProviderCredentials, "$"))
        })
        .transpose()?
        .unwrap_or_default();
    let mut legacy = BTreeMap::new();
    for (index, row) in rows.into_iter().enumerate() {
        report_extra(
            LegacyBackupDocumentKind::Secrets,
            &format!("[{index}]."),
            &row.extra,
            notices,
        );
        require_nonblank(&row.service, LegacyBackupDocumentKind::Secrets, "service")?;
        require_nonblank(&row.account, LegacyBackupDocumentKind::Secrets, "account")?;
        if row
            .created_at
            .zip(row.updated_at)
            .is_some_and(|(created, updated)| created < 0 || updated < created)
        {
            return Err(malformed(
                LegacyBackupDocumentKind::Secrets,
                format!("[{index}].timestamps"),
            ));
        }
        if row.created_at.is_some() {
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Unsupported,
                LegacyBackupDocumentKind::Secrets,
                format!("[{index}].created_at"),
            ));
        }
        if row.updated_at.is_some() {
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Unsupported,
                LegacyBackupDocumentKind::Secrets,
                format!("[{index}].updated_at"),
            ));
        }
        if legacy
            .insert((row.service, row.account), (index, row.value))
            .is_some()
        {
            return Err(malformed(LegacyBackupDocumentKind::Secrets, "identity"));
        }
    }
    let mut result = Vec::new();
    for (index, row) in provider_document.into_iter().enumerate() {
        let canonical_id = ProviderAccountId::from_str(&row.id).unwrap_or_else(|_| {
            ProviderAccountId::from_uuid(Uuid::new_v5(
                &LEGACY_ID_NAMESPACE,
                format!("provider:{}", row.id).as_bytes(),
            ))
        });
        let Some(id) = providers
            .provider_accounts
            .iter()
            .find(|provider| {
                provider.origin == LegacyProviderAccountOrigin::Stored
                    && provider.id == canonical_id
            })
            .map(|provider| provider.id)
        else {
            return Err(orphan(
                LegacyBackupDocumentKind::ProviderCredentials,
                format!("[{index}].id"),
            ));
        };
        let provider = providers
            .provider_accounts
            .iter()
            .find(|provider| provider.id == id)
            .expect("provider found");
        let fallback_key = (
            "lettuceai:apiKey".to_owned(),
            format!("{}:{}", row.provider_id, row.id),
        );
        let fallback = legacy.remove(&fallback_key);
        let inline = normalize_option(row.api_key);
        let api = match (inline, fallback) {
            (Some(inline), Some((secret_index, fallback))) => {
                if inline != fallback {
                    notices.push(notice(
                        LegacyBackupConversionNoticeKind::Unsupported,
                        LegacyBackupDocumentKind::Secrets,
                        format!("[{secret_index}].value"),
                    ));
                }
                Some(inline)
            }
            (Some(inline), None) => Some(inline),
            (None, Some((_, fallback))) => {
                notices.push(notice(
                    LegacyBackupConversionNoticeKind::Lossy,
                    LegacyBackupDocumentKind::ProviderCredentials,
                    format!("[{index}].api_key"),
                ));
                Some(fallback)
            }
            (None, None) => None,
        };
        if let Some(value) = api {
            let reference = deterministic_secret_ref(&format!("provider:{}:api", id));
            result.push(ProviderBackupSecret {
                reference,
                purpose: SecretPurpose::ProviderApiKey {
                    owner: provider.secret_owner_id,
                },
                generation: 1,
                value: SecretValue::new(value).map_err(|_| {
                    malformed(
                        LegacyBackupDocumentKind::ProviderCredentials,
                        format!("[{index}].api_key"),
                    )
                })?,
            });
        }
        for (name, value) in parse_headers(row.headers.as_deref(), index)? {
            let reference = deterministic_secret_ref(&format!(
                "provider:{}:header:{}",
                id,
                name.as_str().to_ascii_lowercase()
            ));
            result.push(ProviderBackupSecret {
                reference,
                purpose: SecretPurpose::ProviderSecretHeader {
                    owner: provider.secret_owner_id,
                    name,
                },
                generation: 1,
                value: SecretValue::new(value).map_err(|_| {
                    malformed(
                        LegacyBackupDocumentKind::ProviderCredentials,
                        format!("[{index}].headers"),
                    )
                })?,
            });
        }
    }
    for (_, (index, _)) in legacy {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Unsupported,
            LegacyBackupDocumentKind::Secrets,
            format!("[{index}]"),
        ));
    }
    result.sort_by_key(|secret| secret.reference);
    Ok(result)
}

fn map_chat_templates(
    rows: Vec<ChatTemplateRow>,
    source: &LegacyBackupInventory,
    prompts: &LegacyPromptPlan,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupChatTemplateCandidate>, LegacyBackupConfigurationError> {
    let characters = source_ids(source, LegacyBackupDocumentKind::Characters)?;
    let character_scenes = character_scene_ids(source)?;
    let lorebooks = source_ids(source, LegacyBackupDocumentKind::Lorebooks)?;
    let prompt_ids = prompts
        .prompts
        .iter()
        .map(|prompt| prompt.source_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut result = Vec::new();
    for (index, row) in rows.into_iter().enumerate() {
        report_extra(
            LegacyBackupDocumentKind::ChatTemplates,
            &format!("[{index}]."),
            &row.extra,
            notices,
        );
        require_nonblank(&row.id, LegacyBackupDocumentKind::ChatTemplates, "id")?;
        require_nonblank(
            &row.character_id,
            LegacyBackupDocumentKind::ChatTemplates,
            "character_id",
        )?;
        require_nonblank(&row.name, LegacyBackupDocumentKind::ChatTemplates, "name")?;
        if !characters.is_empty() && !characters.contains(&row.character_id) {
            return Err(orphan(
                LegacyBackupDocumentKind::ChatTemplates,
                format!("[{index}].character_id"),
            ));
        }
        if let Some(scene_id) = row
            .scene_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            if !characters.is_empty()
                && !character_scenes
                    .get(&row.character_id)
                    .is_some_and(|scenes| scenes.contains(scene_id))
            {
                return Err(orphan(
                    LegacyBackupDocumentKind::ChatTemplates,
                    format!("[{index}].scene_id"),
                ));
            }
        }
        if let Some(prompt) = row
            .prompt_template_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            if !prompt_ids.contains(prompt) {
                return Err(orphan(
                    LegacyBackupDocumentKind::ChatTemplates,
                    format!("[{index}].prompt_template_id"),
                ));
            }
        }
        let has_lorebook_override = row.lorebook_ids_override.is_some();
        let lorebook_ids: Vec<String> = row
            .lorebook_ids_override
            .as_deref()
            .map(|value| {
                serde_json::from_str(value).map_err(|_| {
                    malformed(
                        LegacyBackupDocumentKind::ChatTemplates,
                        format!("[{index}].lorebook_ids_override"),
                    )
                })
            })
            .transpose()?
            .unwrap_or_default();
        if !lorebooks.is_empty() && lorebook_ids.iter().any(|id| !lorebooks.contains(id)) {
            return Err(orphan(
                LegacyBackupDocumentKind::ChatTemplates,
                format!("[{index}].lorebook_ids_override"),
            ));
        }
        let mut messages = Vec::with_capacity(row.messages.len());
        for (fallback, message) in row.messages.into_iter().enumerate() {
            report_extra(
                LegacyBackupDocumentKind::ChatTemplates,
                &format!("[{index}].messages[{fallback}]."),
                &message.extra,
                notices,
            );
            require_nonblank(
                &message.id,
                LegacyBackupDocumentKind::ChatTemplates,
                "messages.id",
            )?;
            require_nonblank(
                &message.role,
                LegacyBackupDocumentKind::ChatTemplates,
                "messages.role",
            )?;
            let raw_index = message.idx.unwrap_or(fallback as i64);
            let mapped_index = u32::try_from(raw_index).map_err(|_| {
                malformed(
                    LegacyBackupDocumentKind::ChatTemplates,
                    format!("[{index}].messages[{fallback}].idx"),
                )
            })?;
            if message.idx.is_none() {
                notices.push(notice(
                    LegacyBackupConversionNoticeKind::Lossy,
                    LegacyBackupDocumentKind::ChatTemplates,
                    format!("[{index}].messages[{fallback}].idx"),
                ));
            }
            messages.push(LegacyBackupChatTemplateMessage {
                source_id: message.id,
                index: mapped_index,
                role: message.role,
                content: message.content,
            });
        }
        if messages
            .iter()
            .map(|message| &message.source_id)
            .collect::<BTreeSet<_>>()
            .len()
            != messages.len()
        {
            return Err(malformed(
                LegacyBackupDocumentKind::ChatTemplates,
                format!("[{index}].messages.id"),
            ));
        }
        if messages
            .iter()
            .map(|message| message.index)
            .collect::<BTreeSet<_>>()
            .len()
            != messages.len()
        {
            return Err(malformed(
                LegacyBackupDocumentKind::ChatTemplates,
                format!("[{index}].messages.idx"),
            ));
        }
        messages.sort_by_key(|message| (message.index, message.source_id.clone()));
        if result
            .iter()
            .any(|template: &LegacyBackupChatTemplateCandidate| template.source_id == row.id)
        {
            return Err(malformed(LegacyBackupDocumentKind::ChatTemplates, "id"));
        }
        if characters.is_empty() {
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Absent,
                LegacyBackupDocumentKind::Characters,
                format!("chat_templates[{index}].character_id"),
            ));
        }
        if !lorebook_ids.is_empty() && lorebooks.is_empty() {
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Absent,
                LegacyBackupDocumentKind::Lorebooks,
                format!("chat_templates[{index}].lorebook_ids_override"),
            ));
        }
        result.push(LegacyBackupChatTemplateCandidate {
            source_id: row.id,
            character_source_id: row.character_id,
            name: row.name,
            scene_source_id: normalize_option(row.scene_id),
            prompt_source_id: normalize_option(row.prompt_template_id),
            has_lorebook_override,
            lorebook_source_ids: lorebook_ids,
            messages,
            created_at: TimestampMillis::new(row.created_at),
        });
    }
    result.sort_by(|left, right| left.source_id.cmp(&right.source_id));
    Ok(result)
}

fn validate_selections(
    settings: &LegacyBackupSettingsCandidate,
    providers: &LegacyProviderModelPlan,
    prompts: &LegacyPromptPlan,
) -> Result<(), LegacyBackupConfigurationError> {
    let provider_ids = providers
        .provider_accounts
        .iter()
        .map(|provider| provider.id)
        .collect::<BTreeSet<_>>();
    let model_ids = providers
        .model_profiles
        .iter()
        .map(|model| model.id)
        .collect::<BTreeSet<_>>();
    let prompt_ids = prompts
        .prompts
        .iter()
        .map(|prompt| prompt.source_id.as_str())
        .collect::<BTreeSet<_>>();
    if settings
        .default_provider_account_id
        .is_some_and(|id| !provider_ids.contains(&id))
    {
        return Err(orphan(
            LegacyBackupDocumentKind::Settings,
            "default_provider_credential_id",
        ));
    }
    for (field, value) in [
        ("default_model_id", settings.default_model_profile_id),
        (
            "advanced_settings.summarisationModelId",
            settings.dynamic_memory_model_profile_id,
        ),
        (
            "advanced_settings.groupSpeakerSelectionModelId",
            settings.group_speaker_model_profile_id,
        ),
        (
            "advanced_settings.lorebookGeneratorModelId",
            settings.lorebook_generator_model_profile_id,
        ),
    ] {
        if value.is_some_and(|id| !model_ids.contains(&id)) {
            return Err(orphan(LegacyBackupDocumentKind::Settings, field));
        }
    }
    if settings
        .default_prompt_source_id
        .as_deref()
        .is_some_and(|id| !prompt_ids.contains(id))
    {
        return Err(orphan(
            LegacyBackupDocumentKind::Settings,
            "prompt_template_id",
        ));
    }
    for (field, value) in [
        (
            "planner",
            &settings.lorebook_generator_prompt_source_ids.planner,
        ),
        (
            "writer",
            &settings.lorebook_generator_prompt_source_ids.writer,
        ),
        (
            "refine",
            &settings.lorebook_generator_prompt_source_ids.refine,
        ),
        (
            "coherence",
            &settings.lorebook_generator_prompt_source_ids.coherence,
        ),
    ] {
        if value.as_deref().is_some_and(|id| !prompt_ids.contains(id)) {
            return Err(orphan(
                LegacyBackupDocumentKind::Settings,
                format!("advanced_settings.lorebookGenerator.{field}"),
            ));
        }
    }
    for (index, model) in providers.model_profiles.iter().enumerate() {
        if model
            .prompt_template_id
            .as_deref()
            .is_some_and(|id| !prompt_ids.contains(id))
        {
            return Err(orphan(
                LegacyBackupDocumentKind::Models,
                format!("[{index}].prompt_template_id"),
            ));
        }
    }
    Ok(())
}

fn source_ids(
    source: &LegacyBackupInventory,
    kind: LegacyBackupDocumentKind,
) -> Result<BTreeSet<String>, LegacyBackupConfigurationError> {
    let Some(document) = source
        .documents
        .iter()
        .find(|document| document.kind == kind)
    else {
        return Ok(BTreeSet::new());
    };
    let rows: Vec<Value> =
        serde_json::from_slice(&document.bytes).map_err(|_| malformed(kind, "$"))?;
    let mut result = BTreeSet::new();
    for row in rows {
        let id = row
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.trim().is_empty())
                .map(str::to_owned)
                .ok_or_else(|| malformed(kind, "id"))?;
        if !result.insert(id) {
            return Err(malformed(kind, "id"));
        }
    }
    Ok(result)
}

fn character_scene_ids(
    source: &LegacyBackupInventory,
) -> Result<BTreeMap<String, BTreeSet<String>>, LegacyBackupConfigurationError> {
    let Some(document) = source
        .documents
        .iter()
        .find(|document| document.kind == LegacyBackupDocumentKind::Characters)
    else {
        return Ok(BTreeMap::new());
    };
    let rows: Vec<Value> = serde_json::from_slice(&document.bytes)
        .map_err(|_| malformed(LegacyBackupDocumentKind::Characters, "$"))?;
    let mut result = BTreeMap::new();
    for row in rows {
        let character_id = row
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| malformed(LegacyBackupDocumentKind::Characters, "id"))?;
        let scenes = row
            .get("scenes")
            .and_then(Value::as_array)
            .map(|scenes| {
                scenes
                    .iter()
                    .map(|scene| {
                        scene
                            .get("id")
                            .and_then(Value::as_str)
                            .filter(|value| !value.trim().is_empty())
                            .map(str::to_owned)
                            .ok_or_else(|| {
                                malformed(LegacyBackupDocumentKind::Characters, "scenes.id")
                            })
                    })
                    .collect::<Result<BTreeSet<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();
        if result.insert(character_id.to_owned(), scenes).is_some() {
            return Err(malformed(LegacyBackupDocumentKind::Characters, "id"));
        }
    }
    Ok(result)
}

fn report_unused_audio_fields(
    row: &AudioProviderRow,
    index: usize,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) {
    let unused: &[(&str, bool)] = match row.provider_type.as_str() {
        "gemini_tts" => &[
            ("base_url", row.base_url.is_some()),
            ("request_path", row.request_path.is_some()),
            ("kokoro_variant", row.kokoro_variant.is_some()),
        ],
        "fish_speech" | "openai_tts" => &[
            ("project_id", row.project_id.is_some()),
            ("location", row.location.is_some()),
            ("kokoro_variant", row.kokoro_variant.is_some()),
        ],
        "kokoro" => &[
            ("project_id", row.project_id.is_some()),
            ("location", row.location.is_some()),
            ("base_url", row.base_url.is_some()),
            ("request_path", row.request_path.is_some()),
        ],
        "elevenlabs" | "fish_tts" => &[
            ("project_id", row.project_id.is_some()),
            ("location", row.location.is_some()),
            ("base_url", row.base_url.is_some()),
            ("request_path", row.request_path.is_some()),
            ("kokoro_variant", row.kokoro_variant.is_some()),
        ],
        _ => &[],
    };
    for (field, _) in unused.iter().copied().filter(|(_, present)| *present) {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Unsupported,
            LegacyBackupDocumentKind::AudioProviders,
            format!("[{index}].{field}"),
        ));
    }
}

fn parse_headers(
    value: Option<&str>,
    index: usize,
) -> Result<BTreeMap<HeaderName, String>, LegacyBackupConfigurationError> {
    let Some(value) = value else {
        return Ok(BTreeMap::new());
    };
    let object: BTreeMap<String, Value> = serde_json::from_str(value).map_err(|_| {
        malformed(
            LegacyBackupDocumentKind::ProviderCredentials,
            format!("[{index}].headers"),
        )
    })?;
    if object.len() > 16 {
        return Err(malformed(
            LegacyBackupDocumentKind::ProviderCredentials,
            format!("[{index}].headers"),
        ));
    }
    let mut result = BTreeMap::new();
    for (name, value) in object {
        let value = value
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                malformed(
                    LegacyBackupDocumentKind::ProviderCredentials,
                    format!("[{index}].headers"),
                )
            })?;
        let name = HeaderName::new(name).map_err(|_| {
            malformed(
                LegacyBackupDocumentKind::ProviderCredentials,
                format!("[{index}].headers"),
            )
        })?;
        if result
            .keys()
            .any(|existing: &HeaderName| existing.as_str().eq_ignore_ascii_case(name.as_str()))
        {
            return Err(malformed(
                LegacyBackupDocumentKind::ProviderCredentials,
                format!("[{index}].headers"),
            ));
        }
        result.insert(name, value.to_owned());
    }
    Ok(result)
}

fn resolve_provider(
    providers: &[LegacyProviderAccountCandidate],
    kind: &str,
    explicit: Option<ProviderAccountId>,
    label: &str,
    model: &str,
    default: Option<ProviderAccountId>,
) -> Result<ProviderAccountId, LegacyBackupConfigurationError> {
    if kind.eq_ignore_ascii_case("llamacpp") {
        return providers
            .iter()
            .find(|provider| provider.origin == LegacyProviderAccountOrigin::BuiltInLlamaCpp)
            .map(|provider| provider.id)
            .ok_or_else(|| orphan(LegacyBackupDocumentKind::Models, "provider_id"));
    }
    if let Some(provider) = explicit.and_then(|id| {
        providers
            .iter()
            .find(|provider| provider.id == id && provider.provider_kind == kind)
    }) {
        return Ok(provider.id);
    }
    let candidates = providers
        .iter()
        .filter(|provider| provider.provider_kind == kind)
        .collect::<Vec<_>>();
    if let Some(provider) = default.and_then(|id| {
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
        .find(|provider| provider.label == label)
    {
        return Ok(provider.id);
    }
    if let Some(provider) = candidates
        .iter()
        .copied()
        .find(|provider| provider.default_model.as_deref() == Some(model))
    {
        return Ok(provider.id);
    }
    Err(orphan(
        LegacyBackupDocumentKind::Models,
        "provider_credential_id",
    ))
}

fn legacy_scopes(
    value: Option<&str>,
    model_type: Option<&str>,
    input: bool,
    index: usize,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<ModalityCapabilities, LegacyBackupConfigurationError> {
    let fallback = if input && model_type == Some("multimodel")
        || !input && model_type == Some("imagegeneration")
    {
        vec!["text", "image"]
    } else {
        vec!["text"]
    };
    let values: Vec<String> = match value {
        Some(value) => serde_json::from_str(value).map_err(|_| {
            malformed(
                LegacyBackupDocumentKind::Models,
                format!(
                    "[{index}].{}_scopes",
                    if input { "input" } else { "output" }
                ),
            )
        })?,
        None => {
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Lossy,
                LegacyBackupDocumentKind::Models,
                format!(
                    "[{index}].{}_scopes",
                    if input { "input" } else { "output" }
                ),
            ));
            fallback.into_iter().map(str::to_owned).collect()
        }
    };
    let mut result = ModalityCapabilities {
        text: CapabilityStatus::Unsupported,
        image: CapabilityStatus::Unsupported,
        audio: CapabilityStatus::Unsupported,
    };
    for value in values {
        match value.as_str() {
            "text" => result.text = CapabilityStatus::Supported,
            "image" => result.image = CapabilityStatus::Supported,
            "audio" => result.audio = CapabilityStatus::Supported,
            _ => {
                return Err(malformed(
                    LegacyBackupDocumentKind::Models,
                    format!("[{index}].scopes"),
                ));
            }
        }
    }
    if matches!(
        (result.text, result.image, result.audio),
        (
            CapabilityStatus::Unsupported,
            CapabilityStatus::Unsupported,
            CapabilityStatus::Unsupported
        )
    ) {
        result.text = CapabilityStatus::Supported;
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Lossy,
            LegacyBackupDocumentKind::Models,
            format!("[{index}].scopes"),
        ));
    }
    Ok(result)
}

fn legacy_provider_protocol(kind: &str) -> Option<ProviderProtocol> {
    Some(match kind.to_ascii_lowercase().as_str() {
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
    })
}

fn legacy_provider_config(
    kind: &str,
    object: &Map<String, Value>,
) -> Result<(ProviderConfig, Vec<&'static str>), LegacyBackupConfigurationError> {
    let mut mapped = vec!["streamingEnabled", "allowInvalidTls"];
    if !kind.eq_ignore_ascii_case("custom") && !kind.eq_ignore_ascii_case("custom-anthropic") {
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
    let string = |key: &str| {
        object
            .get(key)
            .map(|value| {
                value.as_str().map(str::to_owned).ok_or_else(|| {
                    malformed(LegacyBackupDocumentKind::ProviderCredentials, "config")
                })
            })
            .transpose()
    };
    let role = |key: &str| {
        string(key)?
            .filter(|value| !value.is_empty())
            .map(WireRole::new)
            .transpose()
            .map_err(|_| malformed(LegacyBackupDocumentKind::ProviderCredentials, "config"))
    };
    let chat_path = string("chatEndpoint")?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            if kind.eq_ignore_ascii_case("custom-anthropic") {
                "/v1/messages".into()
            } else {
                "/chat/completions".into()
            }
        });
    let fetch = optional_bool_value(
        object,
        "fetchModelsEnabled",
        false,
        LegacyBackupDocumentKind::ProviderCredentials,
    )?;
    let models_path = if fetch {
        Some(
            string("modelsEndpoint")?
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    malformed(LegacyBackupDocumentKind::ProviderCredentials, "config")
                })?,
        )
    } else {
        None
    };
    let path = |key: &str, default: &str| {
        JsonPath::new(
            string(key)?
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| default.into()),
        )
        .map_err(|_| malformed(LegacyBackupDocumentKind::ProviderCredentials, "config"))
    };
    let optional_path = |key: &str, default: Option<&str>| {
        string(key)?
            .or_else(|| default.map(str::to_owned))
            .filter(|value| !value.is_empty())
            .map(JsonPath::new)
            .transpose()
            .map_err(|_| malformed(LegacyBackupDocumentKind::ProviderCredentials, "config"))
    };
    let auth = match string("authMode")?
        .unwrap_or_else(|| "header".into())
        .to_ascii_lowercase()
        .as_str()
    {
        "bearer" => CustomAuth::Bearer,
        "header" => CustomAuth::Header {
            name: HeaderName::new(string("authHeaderName")?.unwrap_or_else(|| "x-api-key".into()))
                .map_err(|_| malformed(LegacyBackupDocumentKind::ProviderCredentials, "config"))?,
        },
        "query" => CustomAuth::Query {
            name: QueryParameterName::new(
                string("authQueryParamName")?.unwrap_or_else(|| "api_key".into()),
            )
            .map_err(|_| malformed(LegacyBackupDocumentKind::ProviderCredentials, "config"))?,
        },
        "none" => CustomAuth::None,
        _ => {
            return Err(malformed(
                LegacyBackupDocumentKind::ProviderCredentials,
                "config",
            ));
        }
    };
    let tool_choice_mode = match string("toolChoiceMode")?
        .unwrap_or_else(|| "auto".into())
        .to_ascii_lowercase()
        .as_str()
    {
        "auto" => CustomToolChoiceMode::Auto,
        "required" => CustomToolChoiceMode::Required,
        "none" => CustomToolChoiceMode::None,
        "omit" => CustomToolChoiceMode::Omit,
        "passthrough" => CustomToolChoiceMode::Passthrough,
        _ => {
            return Err(malformed(
                LegacyBackupDocumentKind::ProviderCredentials,
                "config",
            ));
        }
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
            streaming: optional_bool_value(
                object,
                "supportsStream",
                true,
                LegacyBackupDocumentKind::ProviderCredentials,
            )?,
            auth,
            roles: CustomRoles {
                system: role("systemRole")?,
                user: role("userRole")?,
                assistant: role("assistantRole")?,
            },
            merge_same_role_messages: optional_bool_value(
                object,
                "mergeSameRoleMessages",
                true,
                LegacyBackupDocumentKind::ProviderCredentials,
            )?,
            send_chat_template_kwargs: optional_bool_value(
                object,
                "sendChatTemplateKwargs",
                false,
                LegacyBackupDocumentKind::ProviderCredentials,
            )?,
            tool_choice_mode,
        }),
        mapped,
    ))
}

fn legacy_chat_parameters(
    kind: &str,
    object: &Map<String, Value>,
) -> Result<(ChatParameterProfile, Vec<&'static str>), LegacyBackupConfigurationError> {
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
    let f64_value = |key: &str| {
        object
            .get(key)
            .filter(|value| !value.is_null())
            .map(|value| {
                value.as_f64().ok_or_else(|| {
                    malformed(LegacyBackupDocumentKind::Models, "advanced_model_settings")
                })
            })
            .transpose()
    };
    let u32_value = |key: &str| {
        object
            .get(key)
            .filter(|value| !value.is_null())
            .map(|value| {
                value
                    .as_u64()
                    .and_then(|value| u32::try_from(value).ok())
                    .ok_or_else(|| {
                        malformed(LegacyBackupDocumentKind::Models, "advanced_model_settings")
                    })
            })
            .transpose()
    };
    let bool_value = |key: &str| {
        object
            .get(key)
            .filter(|value| !value.is_null())
            .map(|value| {
                value.as_bool().ok_or_else(|| {
                    malformed(LegacyBackupDocumentKind::Models, "advanced_model_settings")
                })
            })
            .transpose()
    };
    let string_value = |key: &str| {
        object
            .get(key)
            .filter(|value| !value.is_null())
            .map(|value| {
                value.as_str().map(str::to_owned).ok_or_else(|| {
                    malformed(LegacyBackupDocumentKind::Models, "advanced_model_settings")
                })
            })
            .transpose()
    };
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
                _ => Err(malformed(
                    LegacyBackupDocumentKind::Models,
                    "advanced_model_settings",
                )),
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
            let default = if kind == "openai" {
                "in_memory"
            } else {
                "5min"
            };
            let retention = match string_value("promptCachingTtl")?
                .unwrap_or_else(|| default.into())
                .as_str()
            {
                "in_memory" => PromptCacheRetention::InMemory,
                "5min" => PromptCacheRetention::FiveMinutes,
                "1h" => PromptCacheRetention::OneHour,
                "24h" => PromptCacheRetention::TwentyFourHours,
                _ => {
                    return Err(malformed(
                        LegacyBackupDocumentKind::Models,
                        "advanced_model_settings",
                    ));
                }
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
                .ok_or_else(|| {
                    malformed(LegacyBackupDocumentKind::Models, "advanced_model_settings")
                })?
                .iter()
                .map(|value| {
                    value.as_str().map(str::to_owned).ok_or_else(|| {
                        malformed(LegacyBackupDocumentKind::Models, "advanced_model_settings")
                    })
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
                .ok_or_else(|| {
                    malformed(LegacyBackupDocumentKind::Models, "advanced_model_settings")
                })
        })
        .transpose()?;
    Ok((
        ChatParameterProfile {
            temperature: f64_value("temperature")?,
            top_p: f64_value("topP")?,
            top_k: u32_value("topK")?,
            max_output_tokens: u32_value("maxOutputTokens")?
                .or(u32_value("ollamaNumPredict")?)
                .filter(|value| *value != 0),
            context_length: u32_value("contextLength")?
                .or(u32_value("ollamaNumCtx")?)
                .filter(|value| *value != 0),
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

fn canonical_provider_id(
    value: &str,
    notices: &mut Vec<LegacyBackupConversionNotice>,
    document: LegacyBackupDocumentKind,
    field: &str,
) -> ProviderAccountId {
    ProviderAccountId::from_str(value).unwrap_or_else(|_| {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Lossy,
            document,
            field,
        ));
        ProviderAccountId::from_uuid(Uuid::new_v5(
            &LEGACY_ID_NAMESPACE,
            format!("provider:{value}").as_bytes(),
        ))
    })
}
pub(crate) fn canonical_model_id(
    value: &str,
    notices: &mut Vec<LegacyBackupConversionNotice>,
    field: &str,
) -> ModelProfileId {
    ModelProfileId::from_str(value).unwrap_or_else(|_| {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Lossy,
            LegacyBackupDocumentKind::Models,
            field,
        ));
        ModelProfileId::from_uuid(Uuid::new_v5(
            &LEGACY_ID_NAMESPACE,
            format!("model:{value}").as_bytes(),
        ))
    })
}
fn canonical_audio_provider_id(
    value: &str,
    notices: &mut Vec<LegacyBackupConversionNotice>,
    field: &str,
) -> AudioProviderId {
    AudioProviderId::from_str(value).unwrap_or_else(|_| {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Lossy,
            LegacyBackupDocumentKind::AudioProviders,
            field,
        ));
        AudioProviderId::from_uuid(Uuid::new_v5(
            &LEGACY_ID_NAMESPACE,
            format!("audio:{value}").as_bytes(),
        ))
    })
}
fn canonical_voice_id(
    value: &str,
    notices: &mut Vec<LegacyBackupConversionNotice>,
    field: &str,
) -> VoiceProfileId {
    VoiceProfileId::from_str(value).unwrap_or_else(|_| {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Lossy,
            LegacyBackupDocumentKind::UserVoices,
            field,
        ));
        VoiceProfileId::from_uuid(Uuid::new_v5(
            &LEGACY_ID_NAMESPACE,
            format!("voice:{value}").as_bytes(),
        ))
    })
}
fn deterministic_secret_ref(value: &str) -> SecretRef {
    SecretRef::from_uuid(Uuid::new_v5(&LEGACY_ID_NAMESPACE, value.as_bytes()))
}

fn parse_optional_id<T: FromStr>(
    value: Option<&str>,
    document: LegacyBackupDocumentKind,
    field: &str,
) -> Result<Option<T>, LegacyBackupConfigurationError> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| T::from_str(value).map_err(|_| malformed(document, field)))
        .transpose()
}
fn advanced_id(
    object: &Map<String, Value>,
    key: &str,
) -> Result<Option<ModelProfileId>, LegacyBackupConfigurationError> {
    normalized_string(object, key)?
        .map(|value| {
            ModelProfileId::from_str(&value).map_err(|_| {
                malformed(
                    LegacyBackupDocumentKind::Settings,
                    format!("advanced_settings.{key}"),
                )
            })
        })
        .transpose()
}
fn normalized_string(
    object: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, LegacyBackupConfigurationError> {
    object
        .get(key)
        .filter(|value| !value.is_null())
        .map(|value| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                malformed(
                    LegacyBackupDocumentKind::Settings,
                    format!("advanced_settings.{key}"),
                )
            })
        })
        .transpose()
        .map(|value| value.and_then(|value| normalize_option(Some(value))))
}
fn optional_u32(
    object: &Map<String, Value>,
    key: &str,
) -> Result<Option<u32>, LegacyBackupConfigurationError> {
    object
        .get(key)
        .filter(|value| !value.is_null())
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| {
                    malformed(
                        LegacyBackupDocumentKind::Settings,
                        format!("advanced_settings.{key}"),
                    )
                })
        })
        .transpose()
}
fn optional_bool_value(
    object: &Map<String, Value>,
    key: &str,
    default: bool,
    document: LegacyBackupDocumentKind,
) -> Result<bool, LegacyBackupConfigurationError> {
    object
        .get(key)
        .map(|value| value.as_bool().ok_or_else(|| malformed(document, key)))
        .transpose()
        .map(|value| value.unwrap_or(default))
}
fn parse_string_object(
    value: Option<&str>,
    document: LegacyBackupDocumentKind,
    field: &str,
) -> Result<Map<String, Value>, LegacyBackupConfigurationError> {
    match value {
        None => Ok(Map::new()),
        Some(value) => serde_json::from_str::<Value>(value)
            .map_err(|_| malformed(document, field))?
            .as_object()
            .cloned()
            .ok_or_else(|| malformed(document, field)),
    }
}
fn object_or_empty<'a>(
    value: &'a Value,
    document: LegacyBackupDocumentKind,
    field: &str,
) -> Result<&'a Map<String, Value>, LegacyBackupConfigurationError> {
    if value.is_null() {
        static EMPTY: std::sync::OnceLock<Map<String, Value>> = std::sync::OnceLock::new();
        Ok(EMPTY.get_or_init(Map::new))
    } else {
        value.as_object().ok_or_else(|| malformed(document, field))
    }
}
fn normalize_option(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
fn require_nonblank(
    value: &str,
    document: LegacyBackupDocumentKind,
    field: &str,
) -> Result<(), LegacyBackupConfigurationError> {
    if value.trim().is_empty() {
        Err(malformed(document, field))
    } else {
        Ok(())
    }
}
fn report_extra(
    document: LegacyBackupDocumentKind,
    prefix: &str,
    extra: &BTreeMap<String, Value>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) {
    notices.extend(extra.keys().map(|field| {
        notice(
            LegacyBackupConversionNoticeKind::Unsupported,
            document,
            format!("{prefix}{field}"),
        )
    }));
}
fn notice(
    kind: LegacyBackupConversionNoticeKind,
    document: LegacyBackupDocumentKind,
    field: impl Into<String>,
) -> LegacyBackupConversionNotice {
    LegacyBackupConversionNotice {
        kind,
        document,
        field: field.into(),
    }
}
fn malformed(
    document: LegacyBackupDocumentKind,
    field: impl Into<String>,
) -> LegacyBackupConfigurationError {
    LegacyBackupConfigurationError::Malformed {
        document,
        field: field.into(),
    }
}
fn orphan(
    document: LegacyBackupDocumentKind,
    field: impl Into<String>,
) -> LegacyBackupConfigurationError {
    LegacyBackupConfigurationError::Orphan {
        document,
        field: field.into(),
    }
}

#[cfg(test)]
mod tests {
    use lettuce_types::ContentHash;
    use serde_json::json;
    use zeroize::Zeroizing;

    use super::*;
    use crate::{LegacyBackupDocument, LegacyBackupMedia};

    fn document(kind: LegacyBackupDocumentKind, value: Value) -> LegacyBackupDocument {
        LegacyBackupDocument {
            kind,
            bytes: Zeroizing::new(serde_json::to_vec(&value).expect("serialize legacy document")),
        }
    }

    fn inventory(documents: Vec<LegacyBackupDocument>) -> LegacyBackupInventory {
        LegacyBackupInventory {
            version: 1,
            created_at: 50,
            app_version: "0.9.0".into(),
            source_hash: ContentHash::parse("11".repeat(32)).expect("source hash"),
            documents,
            media: Vec::<LegacyBackupMedia>::new(),
        }
    }

    #[test]
    fn legacy_configuration_maps_graph_secrets_speech_and_retains_source() {
        let provider_id = ProviderAccountId::new();
        let model_id = ModelProfileId::new();
        let audio_id = AudioProviderId::new();
        let voice_id = VoiceProfileId::new();
        let documents = vec![
            document(
                LegacyBackupDocumentKind::Settings,
                json!({
                    "default_provider_credential_id": provider_id,
                    "default_model_id": model_id,
                    "app_state": {"pureModeEnabled": false, "analyticsEnabled": false, "theme": "dark"},
                    "advanced_model_settings": {"temperature": 0.2},
                    "prompt_template_id": "prompt-main",
                    "system_prompt": "Old global prompt",
                    "migration_version": 92,
                    "advanced_settings": {
                        "appUpdateChecksEnabled": false,
                        "embeddingDimensions": 512,
                        "summarisationModelId": model_id,
                        "groupSpeakerSelectionModelId": model_id,
                        "lorebookGeneratorModelId": model_id,
                        "lorebookGeneratorDefaultTargetCount": 14,
                        "lorebookGeneratorMaxTokens": 2048,
                        "lorebookGeneratorPlannerPromptTemplateId": "prompt-main",
                        "dynamicMemory": {
                            "maxEntries": 60,
                            "minSimilarityThreshold": 0.42,
                            "retrievalLimit": 7,
                            "retrievalStrategy": "cosine",
                            "hotMemoryTokenBudget": 2400,
                            "coldThreshold": 0.25,
                            "contextEnrichmentEnabled": false,
                            "decayRate": 0.1,
                            "enabled": true,
                            "summaryMessageInterval": 12,
                            "runMode": "askFirst",
                            "recursiveMemoryLoops": true,
                            "recursiveMemoryLoopHardCap": 6,
                            "deleteConfidenceDefault": 0.7,
                            "maxHardDeleteRatioPerCycle": 0.25,
                            "unknownKnob": 1
                        }
                    },
                    "created_at": 10,
                    "updated_at": 20
                }),
            ),
            document(
                LegacyBackupDocumentKind::ProviderCredentials,
                json!([{
                    "id": provider_id,
                    "provider_id": "openrouter",
                    "label": "Router",
                    "api_key_ref": "old-reference",
                    "api_key": null,
                    "base_url": "https://openrouter.ai/api/v1",
                    "default_model": "vendor/model",
                    "headers": "{\"X-Client\":\"header-secret\"}",
                    "config": "{\"streamingEnabled\":false,\"allowInvalidTls\":false,\"legacyFlag\":true}"
                }]),
            ),
            document(
                LegacyBackupDocumentKind::Models,
                json!([{
                    "id": model_id,
                    "name": "vendor/model",
                    "provider_id": "openrouter",
                    "provider_credential_id": provider_id,
                    "provider_label": "Router",
                    "display_name": "Model",
                    "created_at": 15,
                    "model_type": "multimodel",
                    "input_scopes": null,
                    "output_scopes": "[\"text\"]",
                    "advanced_model_settings": "{\"temperature\":0.7,\"legacyGpuLayers\":12}",
                    "prompt_template_id": "prompt-main",
                    "system_prompt": "Old model prompt"
                }]),
            ),
            document(
                LegacyBackupDocumentKind::PromptTemplates,
                json!([{
                    "id": "prompt-main",
                    "name": "Main",
                    "prompt_type": "undefined",
                    "content": "System content",
                    "entries": [],
                    "condense_prompt_entries": true,
                    "created_at": 11,
                    "updated_at": 12
                }]),
            ),
            document(
                LegacyBackupDocumentKind::Secrets,
                json!([{
                    "service": "lettuceai:apiKey",
                    "account": format!("openrouter:{provider_id}"),
                    "value": "provider-secret",
                    "created_at": 8,
                    "updated_at": 9
                }]),
            ),
            document(
                LegacyBackupDocumentKind::AudioProviders,
                json!([
                    {
                        "id": audio_id,
                        "provider_type": "elevenlabs",
                        "label": "Voice cloud",
                        "api_key": "audio-secret",
                        "project_id": null,
                        "location": null,
                        "base_url": null,
                        "request_path": null,
                        "kokoro_variant": null,
                        "asset_root": null,
                        "created_at": 16,
                        "updated_at": 17
                    },
                    {
                        "id": "system-kokoro",
                        "provider_type": "kokoro",
                        "label": "Kokoro (Local)",
                        "api_key": null,
                        "project_id": null,
                        "location": null,
                        "base_url": null,
                        "request_path": null,
                        "kokoro_variant": "v1.0",
                        "asset_root": "/machine-specific/models",
                        "created_at": 16,
                        "updated_at": 17
                    }
                ]),
            ),
            document(
                LegacyBackupDocumentKind::UserVoices,
                json!([{
                    "id": voice_id,
                    "provider_id": audio_id,
                    "name": "Narrator",
                    "model_id": "eleven_multilingual_v2",
                    "voice_id": "voice-remote",
                    "prompt": "Calm",
                    "created_at": 18,
                    "updated_at": 19
                }]),
            ),
            document(
                LegacyBackupDocumentKind::Characters,
                json!([{"id": "character-one", "scenes": [{"id": "scene-one"}]}]),
            ),
            document(
                LegacyBackupDocumentKind::Lorebooks,
                json!([{"id": "lorebook-one"}]),
            ),
            document(
                LegacyBackupDocumentKind::ChatTemplates,
                json!([{
                    "id": "chat-template-one",
                    "character_id": "character-one",
                    "name": "Opening",
                    "scene_id": "scene-one",
                    "prompt_template_id": "prompt-main",
                    "lorebook_ids_override": "[\"lorebook-one\"]",
                    "created_at": 21,
                    "messages": [{"id": "message-one", "idx": 0, "role": "user", "content": "Hello"}]
                }]),
            ),
        ];

        let plan =
            plan_legacy_backup_configuration(inventory(documents)).expect("configuration plan");

        assert_eq!(plan.source.documents.len(), 10);
        assert_eq!(plan.provider_models.provider_accounts.len(), 1);
        assert_eq!(plan.provider_models.model_profiles.len(), 1);
        assert_eq!(
            plan.provider_models.default_model_profile_id,
            Some(model_id)
        );
        assert_eq!(plan.prompts.prompts[0].purpose, PromptPurpose::DirectChat);
        assert_eq!(
            plan.prompts.prompts[0].entries[0].draft.content,
            "System content"
        );
        assert_eq!(plan.settings.value.pure_mode, PureMode::Off);
        assert!(!plan.settings.value.analytics_enabled);
        assert!(!plan.settings.value.update_checks_enabled);
        assert_eq!(plan.settings.value.dynamic_memory.max_entries, 60);
        assert_eq!(
            plan.settings
                .value
                .dynamic_memory
                .min_similarity_basis_points,
            4200
        );
        assert_eq!(plan.settings.value.dynamic_memory.retrieval_limit, 7);
        assert_eq!(
            plan.settings.value.dynamic_memory.retrieval_strategy,
            MemoryRetrievalStrategy::Cosine
        );
        assert_eq!(plan.audio_providers.len(), 2);
        assert_eq!(plan.user_voices[0].id, voice_id);
        assert_eq!(
            plan.chat_templates[0].scene_source_id.as_deref(),
            Some("scene-one")
        );
        assert_eq!(plan.secrets.len(), 3);
        assert!(
            plan.secrets
                .iter()
                .any(|secret| secret.value.with(|value| value == "provider-secret"))
        );
        assert!(
            plan.secrets
                .iter()
                .any(|secret| secret.value.with(|value| value == "header-secret"))
        );
        assert!(
            plan.secrets
                .iter()
                .any(|secret| secret.value.with(|value| value == "audio-secret"))
        );
        let debug = format!("{plan:?}");
        assert!(!debug.contains("provider-secret"));
        assert!(!debug.contains("header-secret"));
        assert!(!debug.contains("audio-secret"));
        assert_eq!(plan.settings.value.embedding.dimensions, Some(512));
        let memory = &plan.settings.value.dynamic_memory;
        assert!(memory.enabled);
        assert_eq!(memory.summary_message_interval, 12);
        assert_eq!(memory.run_mode, MemoryRunMode::AskFirst);
        assert!(memory.recursive_memory_loops);
        assert_eq!(memory.recursive_memory_loop_hard_cap, 6);
        assert_eq!(memory.decay_rate_basis_points, 1_000);
        assert_eq!(memory.delete_confidence_basis_points, 7_000);
        assert_eq!(memory.max_hard_delete_ratio_basis_points, 2_500);
        assert!(!plan.notices.iter().any(|notice| notice.field
            == "advanced_settings.dynamicMemory.decayRate"));
        assert!(plan.notices.iter().any(|notice| notice.kind
            == LegacyBackupConversionNoticeKind::Unsupported
            && notice.field == "advanced_settings.dynamicMemory.unknownKnob"));
        assert!(plan.notices.iter().any(|notice| notice.kind
            == LegacyBackupConversionNoticeKind::Lossy
            && notice.document == LegacyBackupDocumentKind::AudioProviders
            && notice.field == "[1].id"));
    }

    #[test]
    fn legacy_configuration_reports_absent_documents_and_rejects_orphan_voice() {
        let empty =
            plan_legacy_backup_configuration(inventory(Vec::new())).expect("empty optional plan");
        let absent_documents = empty
            .notices
            .iter()
            .filter(|notice| notice.kind == LegacyBackupConversionNoticeKind::Absent)
            .map(|notice| notice.document)
            .collect::<BTreeSet<_>>();
        assert_eq!(absent_documents.len(), 8);

        let audio_id = AudioProviderId::new();
        let error = plan_legacy_backup_configuration(inventory(vec![
            document(
                LegacyBackupDocumentKind::AudioProviders,
                json!([{
                    "id": audio_id,
                    "provider_type": "fish_tts",
                    "label": "Fish",
                    "api_key": "secret",
                    "project_id": null,
                    "location": null,
                    "base_url": null,
                    "request_path": null,
                    "kokoro_variant": null,
                    "asset_root": null,
                    "created_at": 1,
                    "updated_at": 1
                }]),
            ),
            document(
                LegacyBackupDocumentKind::UserVoices,
                json!([{
                    "id": VoiceProfileId::new(),
                    "provider_id": AudioProviderId::new(),
                    "name": "Orphan",
                    "model_id": "model",
                    "voice_id": "voice",
                    "prompt": null,
                    "created_at": 1,
                    "updated_at": 1
                }]),
            ),
        ]))
        .expect_err("orphan voice");
        assert!(matches!(
            error,
            LegacyBackupConfigurationError::Orphan {
                document: LegacyBackupDocumentKind::UserVoices,
                ..
            }
        ));
    }
}
