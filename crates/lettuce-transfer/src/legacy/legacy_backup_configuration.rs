use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

use lettuce_context::{
    PromptEntryCondition, PromptEntryDraft, PromptEntryPayload, PromptEntryPosition,
    PromptEntryRole, PromptPurpose,
};
use lettuce_models::{
    CapabilityEvidence, CapabilityEvidenceSource, CapabilityStatus, CustomAuth, CustomModelList,
    CustomProviderConfig, CustomRoles, CustomToolChoiceMode, JsonPath, ModalityCapabilities,
    ModelCapabilities, ModelKind, ModelProfileConfig, ParameterSupport, ProviderConfig,
    ProviderProtocol, QueryParameterName, WireRole,
};
use lettuce_settings::{
    CompanionSoulWriterSettings, CreationHelperSettings, CreationHelperToolFallback,
    DeviceSettings, DynamicMemorySettings, EmbeddingSettings, GlobalSettings, HeaderName,
    HelpMeReplySettings, HelpMeReplyStyle, ImageGenerationSettings, LorebookEntryGeneratorSettings,
    LorebookGeneratorSelection, LorebookGeneratorSettings, MemoryRetrievalStrategy, MemoryRunMode,
    MemoryStructuredFallbackFormat, PureMode, SceneGenerationMode, SecretOwnerId, SecretPurpose,
    SecretRef, SecretValue, UiPreferences,
};
use lettuce_speech::{AudioProvider, AudioProviderConfig, UserVoice};
use lettuce_types::{
    AudioProviderId, ModelProfileId, ProviderAccountId, Revision, TimestampMillis, VoiceProfileId,
};
use serde::Deserialize;
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::{
    LegacyBackupDocumentKind, LegacyBackupInventory, LegacyModelProfileCandidate,
    LegacyPendingProviderSecret, LegacyPromptCandidate, LegacyPromptEntryCandidate,
    LegacyPromptPlan, LegacyProviderAccountCandidate, LegacyProviderAccountOrigin,
    LegacyProviderModelPlan, ProviderBackupSecret,
};

pub const LEGACY_ID_NAMESPACE: Uuid = Uuid::from_u128(0x6c657474_7563_652d_6261_636b75707631);

/// Destination ids of one legacy source. Legacy ids and ids derived from legacy
/// strings are bound to the source fingerprint, so a different legacy source
/// that reuses the same ids imports next to the existing data while a replay of
/// the same source derives the same ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyIdScope(Uuid);

impl LegacyIdScope {
    #[must_use]
    pub fn new(source_fingerprint: &lettuce_types::ContentHash) -> Self {
        Self(Uuid::new_v5(
            &LEGACY_ID_NAMESPACE,
            source_fingerprint.as_str().as_bytes(),
        ))
    }

    #[must_use]
    pub fn uuid(self, legacy: Uuid) -> Uuid {
        Uuid::new_v5(&self.0, legacy.as_bytes())
    }

    /// A legacy string id: a UUID keeps its identity within the scope, any
    /// other string is hashed first.
    #[must_use]
    pub fn source(self, value: &str) -> Uuid {
        self.uuid(
            Uuid::parse_str(value)
                .unwrap_or_else(|_| Uuid::new_v5(&LEGACY_ID_NAMESPACE, value.as_bytes())),
        )
    }

    #[must_use]
    pub fn derived(self, value: &str, suffix: &str) -> Uuid {
        Uuid::new_v5(&self.0, format!("{value}:{suffix}").as_bytes())
    }
}

#[derive(Debug)]
pub struct LegacyBackupConfigurationPlan {
    /// The legacy schema version the source recorded in
    /// `settings.migration_version`, or the importer's layout version when it
    /// recorded none.
    pub source_schema_version: u32,
    pub provider_models: LegacyProviderModelPlan,
    pub prompts: LegacyPromptPlan,
    pub settings: LegacyBackupSettingsCandidate,
    pub audio_providers: Vec<AudioProvider>,
    pub user_voices: Vec<UserVoice>,
    pub secrets: Vec<ProviderBackupSecret>,
    pub chat_templates: Vec<LegacyBackupChatTemplateCandidate>,
    pub skipped: Vec<crate::LegacyImportSkip>,
    pub notices: Vec<LegacyBackupConversionNotice>,
    pub source: LegacyBackupInventory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBackupSettingsCandidate {
    pub value: GlobalSettings,
    /// The app-wide model settings (legacy `settings.advanced_model_settings`).
    pub model_settings: lettuce_models::ModelSettingsLayer,
    pub default_provider_account_id: Option<ProviderAccountId>,
    pub default_model_profile_id: Option<ModelProfileId>,
    pub default_prompt_source_id: Option<String>,
    pub dynamic_memory_model_profile_id: Option<ModelProfileId>,
    pub group_speaker_model_profile_id: Option<ModelProfileId>,
    pub lorebook_generator_model_profile_id: Option<ModelProfileId>,
    pub lorebook_generator_prompt_source_ids: LorebookGeneratorPromptSources,
    pub dynamic_memory_prompt_source_ids: DynamicMemoryPromptSources,
    pub help_me_reply_model_profile_id: Option<ModelProfileId>,
    pub help_me_reply_prompt_source_ids: HelpMeReplyPromptSources,
    pub image_model_profile_ids: ImageModelSources,
    /// This install's legacy onboarding, hint and last-seen-version state,
    /// verbatim under its legacy keys.
    pub device_ui_state: Map<String, Value>,
    /// This install's legacy per-day app usage (`appActiveUsageByDayMs`).
    pub app_usage_days: Vec<lettuce_usage::AppUsageDay>,
    /// This install's legacy trusted certificates, embedding preferences and
    /// models folder.
    pub device_settings: DeviceSettings,
    pub feature_model_profile_ids: FeatureModelSources,
    pub feature_prompt_source_ids: FeaturePromptSources,
    pub deprecated_system_prompt: Option<String>,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

/// Legacy `avatarGenerationModelId`, `sceneGenerationModelId`,
/// `sceneWriterModelId` and `creationHelperImageModelId`, as source ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ImageModelSources {
    pub avatar: Option<ModelProfileId>,
    pub scene: Option<ModelProfileId>,
    pub scene_writer: Option<ModelProfileId>,
    pub creation_helper: Option<ModelProfileId>,
}

/// Legacy `dynamicMemorySummarizerPromptTemplateId` /
/// `dynamicMemoryManagerPromptTemplateId`, retained as source template ids
/// like the lorebook generator prompts.
/// Legacy `helpMeReplyRoleplayPromptTemplateId` /
/// `helpMeReplyConversationalPromptTemplateId`, retained as source ids.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HelpMeReplyPromptSources {
    pub roleplay: Option<String>,
    pub conversational: Option<String>,
}

/// Legacy `creationHelperModelId`, `lorebookEntryGeneratorModelId`,
/// `companionSoulWriterModelId` and `companionSoulWriterFallbackModelId`, as
/// source ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FeatureModelSources {
    pub creation_helper: Option<ModelProfileId>,
    pub lorebook_entry: Option<ModelProfileId>,
    pub soul_writer: Option<ModelProfileId>,
    pub soul_writer_fallback: Option<ModelProfileId>,
}

/// Legacy `lorebookEntryGeneratorPromptTemplateId`,
/// `lorebookKeywordGeneratorPromptTemplateId` and
/// `companionSoulWriterPromptTemplateId`, retained as source ids.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FeaturePromptSources {
    pub lorebook_entry: Option<String>,
    pub lorebook_keyword: Option<String>,
    pub soul_writer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DynamicMemoryPromptSources {
    pub summarizer: Option<String>,
    pub manager: Option<String>,
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
    prompt_type: Option<Value>,
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
    let mut skipped = Vec::new();
    let settings_candidate = map_settings(&settings, &mut notices, &mut skipped)?;
    let provider_rows: Vec<ProviderRow> = array_document(
        &source,
        LegacyBackupDocumentKind::ProviderCredentials,
        &mut notices,
    )?;
    let model_rows: Vec<ModelRow> =
        array_document(&source, LegacyBackupDocumentKind::Models, &mut notices)?;
    let prompt_rows: Vec<PromptRow> = array_document(
        &source,
        LegacyBackupDocumentKind::PromptTemplates,
        &mut notices,
    )?;
    let secret_rows: Vec<SecretRow> =
        array_document(&source, LegacyBackupDocumentKind::Secrets, &mut notices)?;
    let audio_rows: Vec<AudioProviderRow> = array_document(
        &source,
        LegacyBackupDocumentKind::AudioProviders,
        &mut notices,
    )?;
    let voice_rows: Vec<UserVoiceRow> =
        array_document(&source, LegacyBackupDocumentKind::UserVoices, &mut notices)?;
    let chat_rows: Vec<ChatTemplateRow> = array_document(
        &source,
        LegacyBackupDocumentKind::ChatTemplates,
        &mut notices,
    )?;

    let mut provider_models =
        map_provider_models(provider_rows, model_rows, &settings_candidate, &mut notices)?;
    let prompts = map_prompts(prompt_rows, &settings_candidate, &mut notices)?;
    let mut prompts = prompts;
    let mut settings_candidate = settings_candidate;
    reconcile_selections(&mut settings_candidate, &mut provider_models, &mut prompts);
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
    secrets.extend(map_app_secrets(&source, &mut notices));
    for secret in &secrets {
        let (owner, pending) = match &secret.purpose {
            SecretPurpose::ProviderApiKey { owner } => {
                (*owner, LegacyPendingProviderSecret::ApiKey)
            }
            SecretPurpose::ProviderSecretHeader { owner, name } => (
                *owner,
                LegacyPendingProviderSecret::Header { name: name.clone() },
            ),
            SecretPurpose::SproutApiKey { owner } => {
                (*owner, LegacyPendingProviderSecret::SproutApiKey)
            }
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
    let chat_templates =
        map_chat_templates(chat_rows, &source, &prompts, &mut skipped, &mut notices)?;
    skipped.sort();
    skipped.dedup();
    notices.sort();
    notices.dedup();
    let source_schema_version = settings
        .migration_version
        .and_then(|version| u32::try_from(version).ok())
        .filter(|version| *version > 0)
        .unwrap_or(crate::LEGACY_DATABASE_SCHEMA_VERSION);
    Ok(LegacyBackupConfigurationPlan {
        source_schema_version,
        provider_models,
        prompts,
        settings: settings_candidate,
        audio_providers,
        user_voices,
        secrets,
        chat_templates,
        skipped,
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
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<T>, LegacyBackupConfigurationError> {
    Ok(optional_document::<Vec<T>>(source, kind, notices)?.unwrap_or_default())
}

/// A settings JSON column whose stored text does not parse arrives as that
/// text; it reads as absent (defaults) and is recorded as a skip.
fn parsed_settings_json<'a>(
    value: Option<&'a Value>,
    field: &str,
    skipped: &mut Vec<crate::LegacyImportSkip>,
) -> Option<&'a Value> {
    match value {
        Some(Value::String(_)) => {
            skipped.push(crate::legacy_value_skip(
                &format!("settings.{field}"),
                "1",
                crate::LegacyImportSkipReason::MalformedLegacyValue,
            ));
            None
        }
        value => value,
    }
}

fn map_settings(
    row: &SettingsRow,
    notices: &mut Vec<LegacyBackupConversionNotice>,
    skipped: &mut Vec<crate::LegacyImportSkip>,
) -> Result<LegacyBackupSettingsCandidate, LegacyBackupConfigurationError> {
    let created = row.created_at.unwrap_or(0);
    let updated = row.updated_at.or(row.created_at).unwrap_or(0);
    if created < 0 || updated < created {
        return Err(malformed(LegacyBackupDocumentKind::Settings, "timestamps"));
    }
    let app = object_or_empty(
        parsed_settings_json(Some(&row.app_state), "app_state", skipped).unwrap_or(&Value::Null),
        LegacyBackupDocumentKind::Settings,
        "app_state",
    )?;
    let advanced_value =
        parsed_settings_json(row.advanced_settings.as_ref(), "advanced_settings", skipped)
            .unwrap_or(&Value::Null);
    let advanced = object_or_empty(
        advanced_value,
        LegacyBackupDocumentKind::Settings,
        "advanced_settings",
    )?;
    let pure_mode = match app.get("pureModeLevel").and_then(Value::as_str) {
        Some("off") => PureMode::Off,
        Some("strict") => PureMode::Strict,
        Some("low") => PureMode::Low,
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
    let dynamic_memory_llama_sampler_overwrite_enabled = optional_bool_value(
        advanced,
        "dynamicMemoryLlamaSamplerOverwriteEnabled",
        true,
        LegacyBackupDocumentKind::Settings,
    )?;
    let mut dynamic_memory = map_dynamic_memory(
        advanced.get("dynamicMemory"),
        "advanced_settings.dynamicMemory",
        notices,
    )?;
    let mut group_dynamic_memory = advanced
        .get("groupDynamicMemory")
        .map(|value| {
            map_dynamic_memory(Some(value), "advanced_settings.groupDynamicMemory", notices)
        })
        .transpose()?;
    if let Some(format) = advanced.get("dynamicMemoryStructuredFallbackFormat") {
        let format = match format.as_str() {
            Some("json") => MemoryStructuredFallbackFormat::Json,
            Some("xml") => MemoryStructuredFallbackFormat::Xml,
            _ => {
                return Err(malformed(
                    LegacyBackupDocumentKind::Settings,
                    "advanced_settings.dynamicMemoryStructuredFallbackFormat",
                ));
            }
        };
        dynamic_memory.structured_fallback_format = format;
        if let Some(group) = &mut group_dynamic_memory {
            group.structured_fallback_format = format;
        }
    }
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
    let help_me_reply_model = advanced_id(advanced, "helpMeReplyModelId")?;
    let help_me_reply = map_help_me_reply(advanced, notices)?;
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
    let mut mapped_advanced = BTreeSet::from([
        "appUpdateChecksEnabled",
        "dynamicMemory",
        "groupDynamicMemory",
        "dynamicMemoryStructuredFallbackFormat",
        "dynamicMemorySummarizerPromptTemplateId",
        "dynamicMemoryManagerPromptTemplateId",
        "dynamicMemoryLlamaSamplerOverwriteEnabled",
        "helpMeReplyEnabled",
        "helpMeReplyModelId",
        "helpMeReplyStreaming",
        "helpMeReplyMaxTokens",
        "helpMeReplyHistoryCount",
        "helpMeReplyStyle",
        "helpMeReplyRoleplayPromptTemplateId",
        "helpMeReplyConversationalPromptTemplateId",
        "embeddingDimensions",
        "manualModeContextWindow",
        "summarisationModelId",
        "groupSpeakerSelectionModelId",
        "lorebookGeneratorModelId",
        "lorebookGeneratorDefaultTargetCount",
        "lorebookGeneratorMaxTokens",
        "lorebookGeneratorPlannerPromptTemplateId",
        "lorebookGeneratorWriterPromptTemplateId",
        "lorebookGeneratorRefinePromptTemplateId",
        "lorebookGeneratorCoherencePromptTemplateId",
        "avatarGenerationEnabled",
        "avatarGenerationModelId",
        "sceneGenerationEnabled",
        "sceneGenerationMode",
        "sceneGenerationModelId",
        "sceneWriterModelId",
        "creationHelperImageModelId",
        "creationHelperModelId",
        "creationHelperStreaming",
        "creationHelperEnabledTools",
        "creationHelperToolFallback",
        "lorebookEntryGeneratorModelId",
        "lorebookEntryGeneratorPromptTemplateId",
        "lorebookKeywordGeneratorPromptTemplateId",
        "lorebookEntryGeneratorStructuredFallbackFormat",
        "companionSoulWriterModelId",
        "companionSoulWriterFallbackModelId",
        "companionSoulWriterPromptTemplateId",
        "companionSoulWriterStructuredFallbackFormat",
    ]);
    mapped_advanced.extend(UI_PREFERENCE_ADVANCED_KEYS);
    mapped_advanced.extend([
        "embeddingModelVersion",
        "embeddingMaxTokens",
        "embeddingKeepModelLoaded",
        "customLlmModelsDir",
        "sdDefaultSize",
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
            "pureModeLevel"
                | "pureModeEnabled"
                | "analyticsEnabled"
                | "autoDownloadCharacterCardAvatars"
                | "autoDownloadDiscoveryAvatars"
                | "trustedCertificates"
        ) && !UI_PREFERENCE_APP_KEYS.contains(&field.as_str())
            && !DEVICE_UI_STATE_KEYS.contains(&field.as_str())
            && !APP_USAGE_KEYS.contains(&field.as_str())
    }) {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Unsupported,
            LegacyBackupDocumentKind::Settings,
            format!("app_state.{field}"),
        ));
    }
    let model_settings = legacy_global_model_settings(
        parsed_settings_json(
            row.advanced_model_settings.as_ref(),
            "advanced_model_settings",
            skipped,
        ),
        notices,
    );
    Ok(LegacyBackupSettingsCandidate {
        model_settings,
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
            dynamic_memory_prompts: lettuce_settings::DynamicMemoryPromptSelection::default(),
            dynamic_memory_llama_sampler_overwrite_enabled,
            help_me_reply,
            image_generation: ImageGenerationSettings {
                scene_default_size: scene_default_size(advanced, notices),
                ..map_image_generation(advanced)?
            },
            creation_helper: map_creation_helper(advanced)?,
            lorebook_entry_generator: LorebookEntryGeneratorSettings {
                structured_fallback_format: fallback_format(
                    advanced,
                    "lorebookEntryGeneratorStructuredFallbackFormat",
                    MemoryStructuredFallbackFormat::Json,
                )?,
                ..LorebookEntryGeneratorSettings::default()
            },
            ui_preferences: ui_preferences(app, advanced, notices),
            auto_download_character_card_avatars: [
                "autoDownloadCharacterCardAvatars",
                "autoDownloadDiscoveryAvatars",
            ]
            .iter()
            .find_map(|key| app.get(*key).and_then(Value::as_bool))
            .unwrap_or(true),
            companion_soul_writer: CompanionSoulWriterSettings {
                structured_fallback_format: fallback_format(
                    advanced,
                    "companionSoulWriterStructuredFallbackFormat",
                    MemoryStructuredFallbackFormat::Json,
                )?,
                ..CompanionSoulWriterSettings::default()
            },
            embedding: EmbeddingSettings {
                dimensions: optional_u32(advanced, "embeddingDimensions")?
                    .and_then(|value| u16::try_from(value).ok()),
            },
            manual_mode_context_window: optional_u32(advanced, "manualModeContextWindow")?
                .unwrap_or(50),
        },
        default_provider_account_id,
        default_model_profile_id,
        default_prompt_source_id: normalize_option(row.prompt_template_id.clone()),
        dynamic_memory_model_profile_id: advanced_id(advanced, "summarisationModelId")?,
        group_speaker_model_profile_id: advanced_id(advanced, "groupSpeakerSelectionModelId")?,
        lorebook_generator_model_profile_id: generator_model,
        lorebook_generator_prompt_source_ids: generator_prompts,
        dynamic_memory_prompt_source_ids: DynamicMemoryPromptSources {
            summarizer: normalized_string(advanced, "dynamicMemorySummarizerPromptTemplateId")?,
            manager: normalized_string(advanced, "dynamicMemoryManagerPromptTemplateId")?,
        },
        help_me_reply_model_profile_id: help_me_reply_model,
        help_me_reply_prompt_source_ids: HelpMeReplyPromptSources {
            roleplay: normalized_string(advanced, "helpMeReplyRoleplayPromptTemplateId")?,
            conversational: normalized_string(
                advanced,
                "helpMeReplyConversationalPromptTemplateId",
            )?,
        },
        image_model_profile_ids: ImageModelSources {
            avatar: advanced_id(advanced, "avatarGenerationModelId")?,
            scene: advanced_id(advanced, "sceneGenerationModelId")?,
            scene_writer: advanced_id(advanced, "sceneWriterModelId")?,
            creation_helper: advanced_id(advanced, "creationHelperImageModelId")?,
        },
        device_ui_state: device_ui_state(app, notices),
        app_usage_days: app_usage_days(app, notices),
        device_settings: map_device_settings(app, advanced, notices),
        feature_model_profile_ids: FeatureModelSources {
            creation_helper: advanced_id(advanced, "creationHelperModelId")?,
            lorebook_entry: advanced_id(advanced, "lorebookEntryGeneratorModelId")?,
            soul_writer: advanced_id(advanced, "companionSoulWriterModelId")?,
            soul_writer_fallback: advanced_id(advanced, "companionSoulWriterFallbackModelId")?,
        },
        feature_prompt_source_ids: FeaturePromptSources {
            lorebook_entry: normalized_string(advanced, "lorebookEntryGeneratorPromptTemplateId")?,
            lorebook_keyword: normalized_string(
                advanced,
                "lorebookKeywordGeneratorPromptTemplateId",
            )?,
            soul_writer: normalized_string(advanced, "companionSoulWriterPromptTemplateId")?,
        },
        deprecated_system_prompt: normalize_option(row.system_prompt.clone()),
        created_at: TimestampMillis::new(created),
        updated_at: TimestampMillis::new(updated),
    })
}

/// Legacy app-state keys only the app shell read, kept in `ui_preferences`.
const UI_PREFERENCE_APP_KEYS: [&str; 6] = [
    "theme",
    "settingsCardOpacity",
    "customColors",
    "customColorPresets",
    "chatsViewMode",
    "groupChatsViewMode",
];

/// Legacy advanced-settings keys only the app shell read, kept in
/// `ui_preferences`.
const UI_PREFERENCE_ADVANCED_KEYS: [&str; 9] = [
    "accessibility",
    "navigationStyle",
    "navigationSide",
    "headerStyle",
    "navItems",
    "navAlign",
    "navEdge",
    "chatAppearance",
    "llamaSamplerPresets",
];

/// Legacy app-state keys that describe this install rather than the user's
/// preferences: onboarding progress, dismissed hints, the last version seen
/// and the active-usage counters.
const DEVICE_UI_STATE_KEYS: [&str; 3] = ["onboarding", "tooltips", "lastSeenAppVersion"];

const APP_USAGE_KEYS: [&str; 4] = [
    "appActiveUsageMs",
    "appActiveUsageByDayMs",
    "appActiveUsageStartedAtMs",
    "appActiveUsageLastUpdatedAtMs",
];

/// Legacy per-day usage; an unreadable day is dropped and recorded, and a
/// total above the days' sum (time legacy counted before it kept days) is
/// recorded as lost.
fn app_usage_days(
    app: &Map<String, Value>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Vec<lettuce_usage::AppUsageDay> {
    let mut days = Vec::new();
    for (day, value) in app
        .get("appActiveUsageByDayMs")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        match value.as_u64() {
            Some(active_ms) if lettuce_usage::is_app_usage_day(day) => {
                days.push(lettuce_usage::AppUsageDay {
                    day: day.clone(),
                    active_ms,
                });
            }
            _ => notices.push(notice(
                LegacyBackupConversionNoticeKind::Lossy,
                LegacyBackupDocumentKind::Settings,
                "app_state.appActiveUsageByDayMs",
            )),
        }
    }
    let counted = days
        .iter()
        .fold(0_u64, |total, day| total.saturating_add(day.active_ms));
    if app
        .get("appActiveUsageMs")
        .and_then(Value::as_u64)
        .is_some_and(|total| total > counted)
    {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Lossy,
            LegacyBackupDocumentKind::Settings,
            "app_state.appActiveUsageMs",
        ));
    }
    days
}

/// Legacy device settings: entries that cannot be kept are dropped and
/// recorded, display names and labels past the bound are shortened and
/// recorded, and a value of the wrong type is recorded, so the result always
/// validates.
fn map_device_settings(
    app: &Map<String, Value>,
    advanced: &Map<String, Value>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> DeviceSettings {
    let mut lossy_fields = Vec::new();
    let present = |object: &Map<String, Value>, key: &str| {
        object.get(key).filter(|value| !value.is_null()).cloned()
    };
    let mut typed = |value: Option<Value>, field: &str, expected: fn(&Value) -> bool| {
        let value = value?;
        if expected(&value) {
            Some(value)
        } else {
            lossy_fields.push(field.to_owned());
            None
        }
    };
    let certificates = typed(
        present(app, "trustedCertificates"),
        "app_state.trustedCertificates",
        Value::is_array,
    );
    let version = typed(
        present(advanced, "embeddingModelVersion"),
        "advanced_settings.embeddingModelVersion",
        Value::is_string,
    );
    let tokens = typed(
        present(advanced, "embeddingMaxTokens"),
        "advanced_settings.embeddingMaxTokens",
        Value::is_u64,
    );
    let keep_loaded = typed(
        present(advanced, "embeddingKeepModelLoaded"),
        "advanced_settings.embeddingKeepModelLoaded",
        Value::is_boolean,
    );
    let folder = typed(
        present(advanced, "customLlmModelsDir"),
        "advanced_settings.customLlmModelsDir",
        Value::is_string,
    );
    let mut settings = DeviceSettings::default();
    for (index, value) in certificates
        .as_ref()
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let field = format!("app_state.trustedCertificates[{index}]");
        let certificate = (|| {
            Some(lettuce_settings::TrustedCertificate {
                id: Uuid::parse_str(value.get("id")?.as_str()?).ok()?,
                name: value.get("name")?.as_str()?.to_owned(),
                pem: value.get("pem")?.as_str()?.to_owned(),
                imported_at: value.get("importedAt")?.as_i64()?,
            })
        })()
        .map(|certificate| lettuce_settings::TrustedCertificate {
            name: shorten_name(
                &certificate.name,
                format!("{field}.name"),
                &mut lossy_fields,
            ),
            ..certificate
        });
        let mut candidate = settings.clone();
        candidate.trusted_certificates.extend(certificate.clone());
        match certificate {
            Some(certificate) if candidate.validate().is_ok() => {
                settings.trusted_certificates.push(certificate);
            }
            _ => lossy_fields.push(field),
        }
    }
    settings.embedding.model_version = match version.as_ref().and_then(Value::as_str) {
        Some("v3") => Some(lettuce_settings::EmbeddingModelVersion::V3),
        Some("v4") => Some(lettuce_settings::EmbeddingModelVersion::V4),
        None => None,
        Some(_) => {
            lossy_fields.push("advanced_settings.embeddingModelVersion".to_owned());
            None
        }
    };
    settings.embedding.max_tokens = tokens
        .as_ref()
        .and_then(Value::as_u64)
        .map(|tokens| u16::try_from(tokens.clamp(512, 4096)).unwrap_or(4096));
    settings.embedding.keep_model_loaded = keep_loaded
        .as_ref()
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if let Some(folder) = folder
        .as_ref()
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|folder| !folder.is_empty())
    {
        if folder.len() > 4096 {
            lossy_fields.push("advanced_settings.customLlmModelsDir".to_owned());
        } else {
            settings.llm_models_dir = Some(folder.to_owned());
        }
    }
    notices.extend(lossy_fields.into_iter().map(|field| {
        notice(
            LegacyBackupConversionNoticeKind::Lossy,
            LegacyBackupDocumentKind::Settings,
            field,
        )
    }));
    settings
}

/// A display name or label cut to the device-settings bound at a character
/// boundary; a cut is recorded.
fn shorten_name(value: &str, field: String, lossy: &mut Vec<String>) -> String {
    if value.len() <= lettuce_settings::MAX_CERTIFICATE_NAME_BYTES {
        return value.to_owned();
    }
    lossy.push(field);
    let mut end = lettuce_settings::MAX_CERTIFICATE_NAME_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}

/// Legacy `sdDefaultSize` (read by scene generation only, no writer):
/// trimmed, and dropped when blank or longer than an image option may be.
fn scene_default_size(
    advanced: &Map<String, Value>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Option<String> {
    let size = advanced
        .get("sdDefaultSize")
        .and_then(Value::as_str)?
        .trim();
    if size.is_empty() {
        return None;
    }
    if size.len() > 64 {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Lossy,
            LegacyBackupDocumentKind::Settings,
            "advanced_settings.sdDefaultSize",
        ));
        return None;
    }
    Some(size.to_owned())
}

/// This install's shell state, verbatim under its legacy keys; a document past
/// the device state bound is dropped and recorded.
fn device_ui_state(
    app: &Map<String, Value>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Map<String, Value> {
    let state = DEVICE_UI_STATE_KEYS
        .iter()
        .filter_map(|key| {
            app.get(*key)
                .filter(|value| !value.is_null())
                .map(|value| ((*key).to_owned(), value.clone()))
        })
        .collect::<Map<String, Value>>();
    if serde_json::to_vec(&state)
        .is_ok_and(|bytes| bytes.len() <= lettuce_settings::MAX_UI_PREFERENCES_BYTES)
    {
        state
    } else {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Lossy,
            LegacyBackupDocumentKind::Settings,
            "app_state.device_ui_state",
        ));
        Map::new()
    }
}

/// The shell-only preferences, verbatim under their legacy keys; a document
/// past the size bound is dropped and recorded.
fn ui_preferences(
    app: &Map<String, Value>,
    advanced: &Map<String, Value>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> UiPreferences {
    let preferences = UiPreferences(
        UI_PREFERENCE_APP_KEYS
            .iter()
            .map(|key| (app, *key))
            .chain(
                UI_PREFERENCE_ADVANCED_KEYS
                    .iter()
                    .map(|key| (advanced, *key)),
            )
            .filter_map(|(object, key)| {
                object
                    .get(key)
                    .filter(|value| !value.is_null())
                    .map(|value| (key.to_owned(), value.clone()))
            })
            .collect(),
    );
    if preferences.within_bounds() {
        preferences
    } else {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Lossy,
            LegacyBackupDocumentKind::Settings,
            "app_state.ui_preferences",
        ));
        UiPreferences::default()
    }
}

/// Legacy `creationHelperStreaming` (default on), `creationHelperEnabledTools`
/// (the string entries of an array, as legacy read it; anything else means
/// every tool) and `creationHelperToolFallback` (`from_setting`: json, xml,
/// otherwise native).
fn map_creation_helper(
    advanced: &Map<String, Value>,
) -> Result<CreationHelperSettings, LegacyBackupConfigurationError> {
    let mut result = CreationHelperSettings::default();
    result.streaming = optional_bool_value(
        advanced,
        "creationHelperStreaming",
        result.streaming,
        LegacyBackupDocumentKind::Settings,
    )?;
    result.enabled_tools = advanced
        .get("creationHelperEnabledTools")
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| tool.as_str().map(str::to_owned))
                .collect()
        });
    result.tool_fallback = match advanced
        .get("creationHelperToolFallback")
        .and_then(Value::as_str)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("json") => CreationHelperToolFallback::Json,
        Some("xml") => CreationHelperToolFallback::Xml,
        _ => CreationHelperToolFallback::Native,
    };
    Ok(result)
}

/// A legacy `json` / `xml` structured fallback format setting.
fn fallback_format(
    advanced: &Map<String, Value>,
    key: &str,
    default: MemoryStructuredFallbackFormat,
) -> Result<MemoryStructuredFallbackFormat, LegacyBackupConfigurationError> {
    match advanced.get(key).filter(|value| !value.is_null()) {
        None => Ok(default),
        Some(value) => match value.as_str() {
            Some("json") => Ok(MemoryStructuredFallbackFormat::Json),
            Some("xml") => Ok(MemoryStructuredFallbackFormat::Xml),
            _ => Err(malformed(
                LegacyBackupDocumentKind::Settings,
                format!("advanced_settings.{key}"),
            )),
        },
    }
}

/// Legacy `avatarGenerationEnabled` (default on), `sceneGenerationEnabled`
/// (default off) and `sceneGenerationMode`; the model ids travel separately.
fn map_image_generation(
    advanced: &Map<String, Value>,
) -> Result<ImageGenerationSettings, LegacyBackupConfigurationError> {
    let mut result = ImageGenerationSettings::default();
    result.avatar_enabled = optional_bool_value(
        advanced,
        "avatarGenerationEnabled",
        result.avatar_enabled,
        LegacyBackupDocumentKind::Settings,
    )?;
    result.scene_enabled = optional_bool_value(
        advanced,
        "sceneGenerationEnabled",
        result.scene_enabled,
        LegacyBackupDocumentKind::Settings,
    )?;
    result.scene_mode = match advanced.get("sceneGenerationMode").and_then(Value::as_str) {
        None | Some("auto") => SceneGenerationMode::Auto,
        Some("askFirst") => SceneGenerationMode::AskFirst,
        Some("manual") => SceneGenerationMode::Manual,
        Some(_) => {
            return Err(malformed(
                LegacyBackupDocumentKind::Settings,
                "advanced_settings.sceneGenerationMode",
            ));
        }
    };
    Ok(result)
}

fn map_help_me_reply(
    advanced: &Map<String, Value>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<HelpMeReplySettings, LegacyBackupConfigurationError> {
    let mut result = HelpMeReplySettings::default();
    result.enabled = optional_bool_value(
        advanced,
        "helpMeReplyEnabled",
        result.enabled,
        LegacyBackupDocumentKind::Settings,
    )?;
    result.streaming = optional_bool_value(
        advanced,
        "helpMeReplyStreaming",
        result.streaming,
        LegacyBackupDocumentKind::Settings,
    )?;
    if let Some(max_tokens) = optional_u32(advanced, "helpMeReplyMaxTokens")? {
        result.max_output_tokens = max_tokens;
    }
    if let Some(history_count) = optional_u32(advanced, "helpMeReplyHistoryCount")? {
        if history_count == 0 {
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Lossy,
                LegacyBackupDocumentKind::Settings,
                "advanced_settings.helpMeReplyHistoryCount",
            ));
        } else {
            result.history_count = history_count;
        }
    }
    result.style = match advanced.get("helpMeReplyStyle").and_then(Value::as_str) {
        None | Some("roleplay") => HelpMeReplyStyle::Roleplay,
        Some("conversational") => HelpMeReplyStyle::Conversational,
        Some(_) => {
            return Err(malformed(
                LegacyBackupDocumentKind::Settings,
                "advanced_settings.helpMeReplyStyle",
            ));
        }
    };
    Ok(result)
}

/// Values legacy wrote for `minSimilarityThreshold` without the user
/// choosing one (settings defaults, onboarding, the embedding test), which
/// therefore read as unset.
const LEGACY_WRITTEN_MIN_SIMILARITY: [u16; 2] = [3_200, 3_500];

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
    result.min_similarity_basis_points = object
        .get("minSimilarityThreshold")
        .map(|value| {
            threshold_basis_points(
                Some(value),
                &format!("{path}.minSimilarityThreshold"),
                0,
                notices,
            )
        })
        .transpose()?
        .filter(|basis_points| !LEGACY_WRITTEN_MIN_SIMILARITY.contains(basis_points));
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
    let mut skipped = Vec::new();
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
        let config_value = match crate::lenient_legacy_json(
            row.config.as_deref(),
            "provider_credentials.config",
            &row.id,
            &mut skipped,
            Value::is_object,
        ) {
            Some(Value::Object(object)) => object,
            _ => Map::new(),
        };
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
        let headers_accepted = crate::lenient_legacy_json(
            row.headers.as_deref(),
            "provider_credentials.headers",
            &row.id,
            &mut skipped,
            |value| {
                value
                    .as_object()
                    .is_some_and(|object| object.values().all(Value::is_string))
            },
        )
        .is_some();
        let headers = parse_headers(row.headers.as_deref().filter(|_| headers_accepted), index)?;
        pending_secrets.extend(
            headers
                .keys()
                .cloned()
                .map(|name| LegacyPendingProviderSecret::Header { name }),
        );
        if let Some(sprout) = crate::legacy_sprout_config(&row.provider_id, &config_value) {
            if sprout.key.is_some() {
                pending_secrets.push(LegacyPendingProviderSecret::SproutApiKey);
            }
            if sprout.url_rejected {
                notices.push(notice(
                    LegacyBackupConversionNoticeKind::Lossy,
                    LegacyBackupDocumentKind::ProviderCredentials,
                    format!("[{index}].config.sproutUrl"),
                ));
            }
        }
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
        let Some(provider_id) = resolve_provider(
            &providers,
            &row.provider_id,
            explicit,
            &row.provider_label,
            &row.name,
            settings.default_provider_account_id,
        ) else {
            skipped.push(crate::LegacyImportSkip {
                kind: crate::LegacyImportSkipKind::ModelProfile,
                source_key: id.to_string(),
                reason: crate::LegacyImportSkipReason::MissingProviderAccount,
            });
            continue;
        };
        let input = legacy_scopes(
            row.input_scopes.as_deref(),
            row.model_type.as_deref(),
            true,
            (index, &row.id),
            &mut skipped,
            notices,
        )?;
        let output = legacy_scopes(
            row.output_scopes.as_deref(),
            row.model_type.as_deref(),
            false,
            (index, &row.id),
            &mut skipped,
            notices,
        )?;
        let advanced = match crate::lenient_legacy_json(
            row.advanced_model_settings.as_deref(),
            "models.advanced_model_settings",
            &row.id,
            &mut skipped,
            Value::is_object,
        ) {
            Some(Value::Object(object)) => object,
            _ => Map::new(),
        };
        let crate::LegacyModelParameters {
            chat_parameters,
            feature_parameters,
            llama_cpp,
            stable_diffusion,
            lossy_fields,
            unknown_fields,
        } = crate::legacy_model_parameters(&row.provider_id, &advanced);
        for (kind, field) in lossy_fields
            .iter()
            .map(|field| (LegacyBackupConversionNoticeKind::Lossy, field))
            .chain(
                unknown_fields
                    .iter()
                    .map(|field| (LegacyBackupConversionNoticeKind::Unsupported, field)),
            )
        {
            notices.push(notice(
                kind,
                LegacyBackupDocumentKind::Models,
                format!("[{index}].advanced_model_settings.{field}"),
            ));
        }
        let mut deferred = lossy_fields;
        deferred.extend(unknown_fields);
        deferred.sort();
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
            feature_parameters,
            llama_cpp,
            stable_diffusion,
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
        config.validate_parameters().map_err(|_| {
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
    skipped.sort();
    Ok(LegacyProviderModelPlan {
        provider_accounts: providers,
        model_profiles: models,
        default_provider_account_id: None,
        default_model_profile_id: None,
        skipped,
    })
}

/// The prompt types the legacy prompt store accepted, including its snake_case
/// lorebook aliases; anything else was stored but read back as undefined.
#[must_use]
pub fn legacy_prompt_purpose(value: &str) -> Option<PromptPurpose> {
    Some(match value {
        "undefined" => PromptPurpose::Undefined,
        "directChat" => PromptPurpose::DirectChat,
        "companionChat" => PromptPurpose::CompanionChat,
        "groupChatRoleplay" => PromptPurpose::GroupChatRoleplay,
        "groupChatConversational" => PromptPurpose::GroupChatConversational,
        "dynamicMemorySummarizer" => PromptPurpose::DynamicMemorySummarizer,
        "dynamicMemoryManager" => PromptPurpose::DynamicMemoryManager,
        "replyHelperRoleplay" => PromptPurpose::ReplyHelperRoleplay,
        "replyHelperConversational" => PromptPurpose::ReplyHelperConversational,
        "lorebookEntryWriter" | "lorebook_entry_writer" => PromptPurpose::LorebookEntryWriter,
        "lorebookKeywordGenerator" | "lorebook_keyword_generator" => {
            PromptPurpose::LorebookKeywordGenerator
        }
        "lorebookGeneratorPlanner" | "lorebook_generator_planner" => {
            PromptPurpose::LorebookGeneratorPlanner
        }
        "lorebookGeneratorWriter" | "lorebook_generator_writer" => {
            PromptPurpose::LorebookGeneratorWriter
        }
        "lorebookGeneratorRefine" | "lorebook_generator_refine" => {
            PromptPurpose::LorebookGeneratorRefine
        }
        "lorebookGeneratorCoherence" | "lorebook_generator_coherence" => {
            PromptPurpose::LorebookGeneratorCoherence
        }
        "avatarGeneration" => PromptPurpose::AvatarGeneration,
        "avatarEditRequest" => PromptPurpose::AvatarEditRequest,
        "sceneGeneration" => PromptPurpose::SceneGeneration,
        "scenePromptWriter" => PromptPurpose::ScenePromptWriter,
        "designReferenceWriter" => PromptPurpose::DesignReferenceWriter,
        "companionSoulWriter" => PromptPurpose::CompanionSoulWriter,
        "companionGrowthcycle" => PromptPurpose::CompanionGrowthcycle,
        "companionConsolidation" => PromptPurpose::CompanionConsolidation,
        _ => return None,
    })
}

fn map_prompts(
    rows: Vec<PromptRow>,
    settings: &LegacyBackupSettingsCandidate,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<LegacyPromptPlan, LegacyBackupConfigurationError> {
    let mut skipped = Vec::new();
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
        let unknown_type = |skipped: &mut Vec<crate::LegacyImportSkip>| {
            skipped.push(crate::legacy_value_skip(
                "prompt_templates.prompt_type",
                &row.id,
                crate::LegacyImportSkipReason::UnknownLegacyValue,
            ));
        };
        let prompt_type = match row.prompt_type {
            None | Some(Value::Null) => "undefined".to_owned(),
            Some(Value::String(value)) => value,
            Some(_) => {
                unknown_type(&mut skipped);
                "undefined".to_owned()
            }
        };
        let mut purpose = legacy_prompt_purpose(&prompt_type).unwrap_or_else(|| {
            unknown_type(&mut skipped);
            PromptPurpose::Undefined
        });
        if purpose == PromptPurpose::Undefined {
            purpose = PromptPurpose::DirectChat;
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Lossy,
                LegacyBackupDocumentKind::PromptTemplates,
                format!("[{index}].prompt_type"),
            ));
        }
        let entry_value = match row.entries {
            Value::Null => Value::Array(Vec::new()),
            Value::Array(items) => Value::Array(items),
            other => {
                match other
                    .as_str()
                    .and_then(|text| serde_json::from_str::<Value>(text).ok())
                {
                    Some(Value::Array(items)) => Value::Array(items),
                    _ => {
                        skipped.push(crate::legacy_value_skip(
                            "prompt_templates.entries",
                            &row.id,
                            crate::LegacyImportSkipReason::MalformedLegacyValue,
                        ));
                        Value::Array(Vec::new())
                    }
                }
            }
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
                    conditions: lettuce_context::legacy_scene_protocol_conditions(
                        purpose,
                        &source_id,
                        entry.conditions,
                    ),
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
    skipped.sort();
    Ok(LegacyPromptPlan {
        skipped,
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
        let headers = if provider
            .pending_secrets
            .iter()
            .any(|secret| matches!(secret, LegacyPendingProviderSecret::Header { .. }))
        {
            parse_headers(row.headers.as_deref(), index)?
        } else {
            BTreeMap::new()
        };
        let sprout_key = row
            .config
            .as_deref()
            .and_then(|config| serde_json::from_str::<Value>(config).ok())
            .and_then(|config| {
                config
                    .as_object()
                    .and_then(|object| crate::legacy_sprout_config(&row.provider_id, object))
            })
            .and_then(|sprout| sprout.key);
        if let Some(value) = sprout_key {
            result.push(ProviderBackupSecret {
                reference: deterministic_secret_ref(&format!("provider:{id}:sprout")),
                purpose: SecretPurpose::SproutApiKey {
                    owner: provider.secret_owner_id,
                },
                generation: 1,
                value: SecretValue::new(value).map_err(|_| {
                    malformed(
                        LegacyBackupDocumentKind::ProviderCredentials,
                        format!("[{index}].config.sproutApiKey"),
                    )
                })?,
            });
        }
        for (name, value) in headers {
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
    skipped: &mut Vec<crate::LegacyImportSkip>,
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
        let template_id = row.id.clone();
        let reference = |kind, field: &str, reason| crate::LegacyImportSkip {
            kind,
            source_key: format!("chat_templates.{field}:{template_id}"),
            reason,
        };
        let mut scene_source_id = normalize_option(row.scene_id);
        if scene_source_id.as_deref().is_some_and(|scene_id| {
            !characters.is_empty()
                && !character_scenes
                    .get(&row.character_id)
                    .is_some_and(|scenes| scenes.contains(scene_id))
        }) {
            scene_source_id = None;
            skipped.push(reference(
                crate::LegacyImportSkipKind::SceneReference,
                "scene_id",
                crate::LegacyImportSkipReason::MissingScene,
            ));
        }
        let prompt_source_id = normalize_option(row.prompt_template_id);
        if prompt_source_id
            .as_deref()
            .is_some_and(|prompt| !prompt_ids.contains(prompt))
        {
            skipped.push(reference(
                crate::LegacyImportSkipKind::PromptReference,
                "prompt_template_id",
                crate::LegacyImportSkipReason::MissingPrompt,
            ));
        }
        let parsed_override = crate::lenient_legacy_json(
            row.lorebook_ids_override.as_deref(),
            "chat_templates.lorebook_ids_override",
            &template_id,
            skipped,
            |value| {
                value
                    .as_array()
                    .is_some_and(|items| items.iter().all(Value::is_string))
            },
        );
        let has_lorebook_override = parsed_override.is_some();
        let mut lorebook_ids = parsed_override
            .and_then(|value| serde_json::from_value::<Vec<String>>(value).ok())
            .unwrap_or_default();
        let mentions_lorebooks = !lorebook_ids.is_empty();
        lorebook_ids.retain(|lorebook_id| {
            let present = lorebooks.contains(lorebook_id);
            if !present {
                skipped.push(crate::LegacyImportSkip {
                    kind: crate::LegacyImportSkipKind::LorebookReference,
                    source_key: format!(
                        "chat_templates.lorebook_ids_override:{template_id}:{lorebook_id}"
                    ),
                    reason: crate::LegacyImportSkipReason::MissingLorebook,
                });
            }
            present
        });
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
        if mentions_lorebooks && lorebooks.is_empty() {
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
            scene_source_id,
            prompt_source_id,
            has_lorebook_override,
            lorebook_source_ids: lorebook_ids,
            messages,
            created_at: TimestampMillis::new(row.created_at),
        });
    }
    result.sort_by(|left, right| left.source_id.cmp(&right.source_id));
    Ok(result)
}

/// Legacy never cleared these ids when their target was deleted: prompt
/// references fell back to the default prompt and most model references failed
/// until the user picked another model. The stale id is cleared and recorded.
fn reconcile_selections(
    settings: &mut LegacyBackupSettingsCandidate,
    providers: &mut LegacyProviderModelPlan,
    prompts: &mut LegacyPromptPlan,
) {
    use crate::{LegacyImportSkip, LegacyImportSkipKind, LegacyImportSkipReason};
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
    let chat_model_ids = providers
        .model_profiles
        .iter()
        .filter(|model| model.kind == ModelKind::Chat)
        .map(|model| model.id)
        .collect::<BTreeSet<_>>();
    let prompt_purposes = prompts
        .prompts
        .iter()
        .map(|prompt| (prompt.source_id.clone(), prompt.purpose))
        .collect::<BTreeMap<_, _>>();
    let reference = |kind, field: &str, row: &str, reason| LegacyImportSkip {
        kind,
        source_key: format!("{field}:{row}"),
        reason,
    };
    if let Some(stale) = settings
        .default_provider_account_id
        .take_if(|id| !provider_ids.contains(id))
    {
        providers.skipped.push(LegacyImportSkip {
            kind: LegacyImportSkipKind::SettingsDefaultProviderAccount,
            source_key: stale.to_string(),
            reason: LegacyImportSkipReason::MissingProviderAccount,
        });
    }
    if let Some(stale) = settings
        .default_model_profile_id
        .take_if(|id| !model_ids.contains(id))
    {
        providers.skipped.push(LegacyImportSkip {
            kind: LegacyImportSkipKind::SettingsDefaultModelProfile,
            source_key: stale.to_string(),
            reason: LegacyImportSkipReason::MissingModelProfile,
        });
    }
    if let Some(incompatible) = settings
        .default_model_profile_id
        .take_if(|id| !chat_model_ids.contains(id))
    {
        providers.skipped.push(LegacyImportSkip {
            kind: LegacyImportSkipKind::SettingsDefaultModelProfile,
            source_key: incompatible.to_string(),
            reason: LegacyImportSkipReason::IncompatibleReference,
        });
    }
    providers.default_provider_account_id = settings.default_provider_account_id;
    providers.default_model_profile_id = settings.default_model_profile_id;
    for (field, value) in [
        (
            "settings.advanced_settings.summarisationModelId",
            &mut settings.dynamic_memory_model_profile_id,
        ),
        (
            "settings.advanced_settings.groupSpeakerSelectionModelId",
            &mut settings.group_speaker_model_profile_id,
        ),
        (
            "settings.advanced_settings.lorebookGeneratorModelId",
            &mut settings.lorebook_generator_model_profile_id,
        ),
        (
            "settings.advanced_settings.helpMeReplyModelId",
            &mut settings.help_me_reply_model_profile_id,
        ),
        (
            "settings.advanced_settings.creationHelperModelId",
            &mut settings.feature_model_profile_ids.creation_helper,
        ),
        (
            "settings.advanced_settings.lorebookEntryGeneratorModelId",
            &mut settings.feature_model_profile_ids.lorebook_entry,
        ),
        (
            "settings.advanced_settings.companionSoulWriterModelId",
            &mut settings.feature_model_profile_ids.soul_writer,
        ),
        (
            "settings.advanced_settings.companionSoulWriterFallbackModelId",
            &mut settings.feature_model_profile_ids.soul_writer_fallback,
        ),
    ] {
        let Some(id) = *value else {
            continue;
        };
        let reason = if !model_ids.contains(&id) {
            LegacyImportSkipReason::MissingModelProfile
        } else if !chat_model_ids.contains(&id) {
            LegacyImportSkipReason::IncompatibleReference
        } else {
            continue;
        };
        *value = None;
        providers.skipped.push(reference(
            LegacyImportSkipKind::ModelReference,
            field,
            &id.to_string(),
            reason,
        ));
    }
    let supported = |status: CapabilityStatus| status == CapabilityStatus::Supported;
    let image_model = |model: &crate::LegacyModelProfileCandidate| {
        supported(model.config.capabilities.output_modalities.image)
    };
    let vision_text_model = |model: &crate::LegacyModelProfileCandidate| {
        let capabilities = &model.config.capabilities;
        supported(capabilities.input_modalities.text)
            && supported(capabilities.input_modalities.image)
            && supported(capabilities.output_modalities.text)
    };
    let configured_scene_model = settings.image_model_profile_ids.scene;
    let images = &mut settings.image_model_profile_ids;
    for (field, value, compatible) in [
        (
            "settings.advanced_settings.avatarGenerationModelId",
            &mut images.avatar,
            &image_model as &dyn Fn(&crate::LegacyModelProfileCandidate) -> bool,
        ),
        (
            "settings.advanced_settings.sceneGenerationModelId",
            &mut images.scene,
            &image_model,
        ),
        (
            "settings.advanced_settings.sceneWriterModelId",
            &mut images.scene_writer,
            &vision_text_model,
        ),
        (
            "settings.advanced_settings.creationHelperImageModelId",
            &mut images.creation_helper,
            &image_model,
        ),
    ] {
        let Some(id) = *value else {
            continue;
        };
        let reason = match providers.model_profiles.iter().find(|model| model.id == id) {
            None => LegacyImportSkipReason::MissingModelProfile,
            Some(model) if !compatible(model) => LegacyImportSkipReason::IncompatibleReference,
            Some(_) => continue,
        };
        *value = None;
        providers.skipped.push(reference(
            LegacyImportSkipKind::ModelReference,
            field,
            &id.to_string(),
            reason,
        ));
    }
    if configured_scene_model.is_some() && settings.image_model_profile_ids.scene.is_none() {
        settings.value.image_generation.scene_enabled = false;
    }
    for (field, value, purpose) in [
        (
            "settings.prompt_template_id",
            &mut settings.default_prompt_source_id,
            None,
        ),
        (
            "settings.advanced_settings.lorebookGeneratorPlannerPromptTemplateId",
            &mut settings.lorebook_generator_prompt_source_ids.planner,
            Some(PromptPurpose::LorebookGeneratorPlanner),
        ),
        (
            "settings.advanced_settings.lorebookGeneratorWriterPromptTemplateId",
            &mut settings.lorebook_generator_prompt_source_ids.writer,
            Some(PromptPurpose::LorebookGeneratorWriter),
        ),
        (
            "settings.advanced_settings.lorebookGeneratorRefinePromptTemplateId",
            &mut settings.lorebook_generator_prompt_source_ids.refine,
            Some(PromptPurpose::LorebookGeneratorRefine),
        ),
        (
            "settings.advanced_settings.lorebookGeneratorCoherencePromptTemplateId",
            &mut settings.lorebook_generator_prompt_source_ids.coherence,
            Some(PromptPurpose::LorebookGeneratorCoherence),
        ),
        (
            "settings.advanced_settings.dynamicMemorySummarizerPromptTemplateId",
            &mut settings.dynamic_memory_prompt_source_ids.summarizer,
            Some(PromptPurpose::DynamicMemorySummarizer),
        ),
        (
            "settings.advanced_settings.dynamicMemoryManagerPromptTemplateId",
            &mut settings.dynamic_memory_prompt_source_ids.manager,
            Some(PromptPurpose::DynamicMemoryManager),
        ),
        (
            "settings.advanced_settings.helpMeReplyRoleplayPromptTemplateId",
            &mut settings.help_me_reply_prompt_source_ids.roleplay,
            Some(PromptPurpose::ReplyHelperRoleplay),
        ),
        (
            "settings.advanced_settings.helpMeReplyConversationalPromptTemplateId",
            &mut settings.help_me_reply_prompt_source_ids.conversational,
            Some(PromptPurpose::ReplyHelperConversational),
        ),
        (
            "settings.advanced_settings.lorebookEntryGeneratorPromptTemplateId",
            &mut settings.feature_prompt_source_ids.lorebook_entry,
            Some(PromptPurpose::LorebookEntryWriter),
        ),
        (
            "settings.advanced_settings.lorebookKeywordGeneratorPromptTemplateId",
            &mut settings.feature_prompt_source_ids.lorebook_keyword,
            Some(PromptPurpose::LorebookKeywordGenerator),
        ),
        (
            "settings.advanced_settings.companionSoulWriterPromptTemplateId",
            &mut settings.feature_prompt_source_ids.soul_writer,
            Some(PromptPurpose::CompanionSoulWriter),
        ),
    ] {
        let Some(id) = value.take() else {
            continue;
        };
        let reason = match prompt_purposes.get(&id) {
            None => LegacyImportSkipReason::MissingPrompt,
            Some(actual) if purpose.is_some_and(|purpose| purpose != *actual) => {
                LegacyImportSkipReason::IncompatibleReference
            }
            Some(_) => {
                *value = Some(id);
                continue;
            }
        };
        prompts.skipped.push(reference(
            LegacyImportSkipKind::PromptReference,
            field,
            &id,
            reason,
        ));
    }
    prompts.default_prompt_source_id = settings.default_prompt_source_id.clone();
    for model in &mut providers.model_profiles {
        if model
            .prompt_template_id
            .take_if(|id| !prompt_purposes.contains_key(id))
            .is_some()
        {
            prompts.skipped.push(reference(
                LegacyImportSkipKind::PromptReference,
                "models.prompt_template_id",
                &model.id.to_string(),
                LegacyImportSkipReason::MissingPrompt,
            ));
        }
    }
    providers.skipped.sort();
    prompts.skipped.sort();
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
) -> Option<ProviderAccountId> {
    if kind.eq_ignore_ascii_case("llamacpp") {
        return providers
            .iter()
            .find(|provider| provider.origin == LegacyProviderAccountOrigin::BuiltInLlamaCpp)
            .map(|provider| provider.id);
    }
    if let Some(provider) = explicit.and_then(|id| {
        providers
            .iter()
            .find(|provider| provider.id == id && provider.provider_kind == kind)
    }) {
        return Some(provider.id);
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
        return Some(provider.id);
    }
    if let [provider] = candidates.as_slice() {
        return Some(provider.id);
    }
    candidates
        .iter()
        .copied()
        .find(|provider| provider.label == label)
        .or_else(|| {
            candidates
                .iter()
                .copied()
                .find(|provider| provider.default_model.as_deref() == Some(model))
        })
        .map(|provider| provider.id)
}

fn legacy_scopes(
    value: Option<&str>,
    model_type: Option<&str>,
    input: bool,
    (index, row_id): (usize, &str),
    skipped: &mut Vec<crate::LegacyImportSkip>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<ModalityCapabilities, LegacyBackupConfigurationError> {
    let fallback = if input && model_type == Some("multimodel")
        || !input && model_type == Some("imagegeneration")
    {
        vec!["text", "image"]
    } else {
        vec!["text"]
    };
    let field = if input {
        "models.input_scopes"
    } else {
        "models.output_scopes"
    };
    let values: Vec<String> = match value {
        Some(value) => {
            match crate::lenient_legacy_json(Some(value), field, row_id, skipped, Value::is_array) {
                Some(Value::Array(items)) => {
                    if items.iter().any(|item| !item.is_string()) {
                        skipped.push(crate::legacy_value_skip(
                            field,
                            row_id,
                            crate::LegacyImportSkipReason::MalformedLegacyValue,
                        ));
                    }
                    items
                        .into_iter()
                        .filter_map(|item| item.as_str().map(str::to_owned))
                        .collect()
                }
                _ => Vec::new(),
            }
        }
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
        match value.to_ascii_lowercase().as_str() {
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
    if let Some(sprout) = crate::legacy_sprout_config(kind, object) {
        mapped.extend(crate::LEGACY_SPROUT_CONFIG_KEYS);
        return Ok((sprout.config, mapped));
    }
    if kind.eq_ignore_ascii_case("comfyui") {
        mapped.extend(["txt2imgWorkflow", "img2imgWorkflow"]);
        let workflow =
            |key: &'static str| -> Result<Option<String>, LegacyBackupConfigurationError> {
                object
                    .get(key)
                    .filter(|value| !value.is_null())
                    .map(|value| {
                        value.as_str().map(str::to_owned).ok_or_else(|| {
                            malformed(LegacyBackupDocumentKind::ProviderCredentials, "config")
                        })
                    })
                    .transpose()
            };
        return Ok((
            ProviderConfig::ComfyUi(lettuce_models::ComfyUiConfig {
                txt2img_workflow: workflow("txt2imgWorkflow")?,
                img2img_workflow: workflow("img2imgWorkflow")?,
            }),
            mapped,
        ));
    }
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
        .as_deref()
        .and_then(legacy_custom_endpoint_path)
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
        string("modelsEndpoint")?
            .as_deref()
            .and_then(legacy_custom_endpoint_path)
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
        _ => CustomAuth::Bearer,
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

/// The legacy app `advanced_model_settings` as the global model settings
/// layer; fields legacy never read from the app layer are left out with Lossy
/// notices (`legacy_settings_layer`).
fn legacy_global_model_settings(
    value: Option<&Value>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> lettuce_models::ModelSettingsLayer {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Default::default();
    };
    let Some(object) = value.as_object() else {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Lossy,
            LegacyBackupDocumentKind::Settings,
            "advanced_model_settings",
        ));
        return Default::default();
    };
    let (layer, lossy, unknown) =
        crate::legacy::legacy_backup_model_settings::legacy_settings_layer(object, true);
    for (kind, field) in lossy
        .iter()
        .map(|field| (LegacyBackupConversionNoticeKind::Lossy, field))
        .chain(
            unknown
                .iter()
                .map(|field| (LegacyBackupConversionNoticeKind::Unsupported, field)),
        )
    {
        notices.push(notice(
            kind,
            LegacyBackupDocumentKind::Settings,
            format!("advanced_model_settings.{field}"),
        ));
    }
    layer
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
/// Serves the provider API keys and secret headers a planned legacy source
/// carried, keyed like the provider secret sources of a legacy import admission.
impl crate::LegacyProviderSecretSource for LegacyBackupConfigurationPlan {
    fn sources(
        &self,
    ) -> Result<Vec<crate::LegacyImportProviderSecretSource>, crate::LegacyProviderSecretSourceError>
    {
        let mut sources = self
            .provider_models
            .provider_accounts
            .iter()
            .filter(|provider| provider.origin == LegacyProviderAccountOrigin::Stored)
            .flat_map(|provider| {
                provider.pending_secrets.iter().cloned().map(|secret| {
                    crate::LegacyImportProviderSecretSource {
                        provider_account_id: provider.id,
                        secret,
                    }
                })
            })
            .collect::<Vec<_>>();
        sources.sort();
        Ok(sources)
    }

    fn load(
        &self,
        source: &crate::LegacyImportProviderSecretSource,
    ) -> Result<SecretValue, crate::LegacyProviderSecretSourceError> {
        let reference = match &source.secret {
            LegacyPendingProviderSecret::ApiKey => {
                deterministic_secret_ref(&format!("provider:{}:api", source.provider_account_id))
            }
            LegacyPendingProviderSecret::Header { name } => deterministic_secret_ref(&format!(
                "provider:{}:header:{}",
                source.provider_account_id,
                name.as_str().to_ascii_lowercase()
            )),
            LegacyPendingProviderSecret::SproutApiKey => {
                deterministic_secret_ref(&format!("provider:{}:sprout", source.provider_account_id))
            }
        };
        self.secrets
            .iter()
            .find(|secret| secret.reference == reference)
            .ok_or(crate::LegacyProviderSecretSourceError::Unavailable)?
            .value
            .with(|value| SecretValue::new(value))
            .map_err(|_| crate::LegacyProviderSecretSourceError::Invalid)
    }
}

/// Legacy's app-wide tokens: the Hugging Face and CivitAI tokens it kept in
/// `meta`. Each goes to the rewrite's fixed reference for its purpose; a blank
/// one is dropped, as legacy treated it as unset.
fn map_app_secrets(
    source: &LegacyBackupInventory,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Vec<ProviderBackupSecret> {
    let meta = source
        .documents
        .iter()
        .find(|document| document.kind == LegacyBackupDocumentKind::Meta)
        .and_then(|document| serde_json::from_slice::<Vec<Value>>(&document.bytes).ok())
        .unwrap_or_default();
    let meta_value = |key: &str| {
        meta.iter()
            .find(|row| row.get("key").and_then(Value::as_str) == Some(key))
            .and_then(|row| row.get("value")?.as_str())
            .map(str::to_owned)
    };
    [
        (
            SecretPurpose::HuggingFaceAccessToken,
            meta_value("hugging_face_access_token"),
            LegacyBackupDocumentKind::Meta,
            "hugging_face_access_token",
        ),
        (
            SecretPurpose::CivitaiAccessToken,
            meta_value("civitai_access_token"),
            LegacyBackupDocumentKind::Meta,
            "civitai_access_token",
        ),
    ]
    .into_iter()
    .filter_map(|(purpose, value, document, field)| {
        let value = value.filter(|value| !value.trim().is_empty())?;
        let reference = purpose.app_secret_ref()?;
        match SecretValue::new(value) {
            Ok(value) => Some(ProviderBackupSecret {
                reference,
                purpose,
                generation: 1,
                value,
            }),
            Err(_) => {
                notices.push(notice(
                    LegacyBackupConversionNoticeKind::Lossy,
                    document,
                    field,
                ));
                None
            }
        }
    })
    .collect()
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

/// A custom `chatEndpoint`/`modelsEndpoint` value as a provider path:
/// trimmed, a whole `http://`/`https://` URL kept as is, and a bare segment
/// given a leading `/`. A blank value has no path.
#[must_use]
pub fn legacy_custom_endpoint_path(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(
        if trimmed.starts_with("http://")
            || trimmed.starts_with("https://")
            || trimmed.starts_with('/')
        {
            trimmed.to_owned()
        } else {
            format!("/{trimmed}")
        },
    )
}

#[cfg(test)]
mod tests {
    use lettuce_types::ContentHash;
    use serde_json::json;
    use zeroize::Zeroizing;

    use super::*;
    use crate::{LegacyBackupDocument, LegacyBackupMedia};

    #[test]
    fn custom_endpoint_paths_are_read_like_legacy_custom_adapter() {
        for (value, expected) in [
            (" chat/completions ", Some("/chat/completions")),
            (
                "/openai/deployments/d/chat/completions?api-version=2024-10-21",
                Some("/openai/deployments/d/chat/completions?api-version=2024-10-21"),
            ),
            (
                "https://other.host/v1/models",
                Some("https://other.host/v1/models"),
            ),
            ("HTTPS://other.host/chat", Some("/HTTPS://other.host/chat")),
            ("  ", None),
        ] {
            assert_eq!(
                legacy_custom_endpoint_path(value).as_deref(),
                expected,
                "{value}"
            );
        }
    }

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
    fn legacy_meta_tokens_become_app_secrets() {
        let mut notices = Vec::new();
        let secrets = map_app_secrets(
            &inventory(vec![document(
                LegacyBackupDocumentKind::Meta,
                json!([
                    {"key": "hugging_face_access_token", "value": "hf_abc"},
                    {"key": "civitai_access_token", "value": "  "},
                    {"key": "schema_version", "value": "90"}
                ]),
            )]),
            &mut notices,
        );
        assert!(notices.is_empty());
        assert_eq!(secrets.len(), 1);
        assert_eq!(secrets[0].purpose, SecretPurpose::HuggingFaceAccessToken);
        assert!(secrets[0].value.with(|value| value == "hf_abc"));
    }

    #[test]
    fn imported_scene_protocol_copies_keep_the_legacy_variant_filter() {
        use lettuce_context::{PromptEntryCondition, SceneImageProtocolKind};
        let chat = PromptEntryCondition::ChatMode {
            value: lettuce_context::PromptEntryChatMode::Direct,
        };
        assert_eq!(
            lettuce_context::legacy_scene_protocol_conditions(
                PromptPurpose::DirectChat,
                "entry_scene_image_protocol_local",
                Some(chat.clone()),
            ),
            Some(PromptEntryCondition::All {
                conditions: vec![
                    chat.clone(),
                    PromptEntryCondition::SceneImageProtocol {
                        value: SceneImageProtocolKind::Local,
                    },
                ],
            })
        );
        assert_eq!(
            lettuce_context::legacy_scene_protocol_conditions(
                PromptPurpose::GroupChatRoleplay,
                "entry_scene_image_protocol",
                Some(chat.clone()),
            ),
            Some(chat)
        );
    }

    #[test]
    fn unknown_structured_fallback_format_is_malformed() {
        let error = plan_legacy_backup_configuration(inventory(vec![document(
            LegacyBackupDocumentKind::Settings,
            json!({
                "advanced_settings": {
                    "dynamicMemoryStructuredFallbackFormat": "yaml"
                },
                "created_at": 10,
                "updated_at": 20
            }),
        )]))
        .expect_err("an unknown fallback format is malformed");
        assert!(matches!(
            error,
            LegacyBackupConfigurationError::Malformed { ref field, .. }
                if field == "advanced_settings.dynamicMemoryStructuredFallbackFormat"
        ));
    }

    #[test]
    fn models_whose_provider_was_deleted_are_skipped() {
        let orphan_model = ModelProfileId::new();
        let plan = plan_legacy_backup_configuration(inventory(vec![document(
            LegacyBackupDocumentKind::Models,
            json!([{
                "id": orphan_model,
                "name": "model-a",
                "provider_id": "anthropic",
                "provider_label": "Deleted",
                "display_name": "Model A",
                "created_at": 10
            }]),
        )]))
        .expect("a model without a provider is skipped");
        assert!(plan.provider_models.model_profiles.is_empty());
        assert_eq!(
            plan.provider_models.skipped,
            vec![crate::LegacyImportSkip {
                kind: crate::LegacyImportSkipKind::ModelProfile,
                source_key: orphan_model.to_string(),
                reason: crate::LegacyImportSkipReason::MissingProviderAccount,
            }]
        );
    }

    #[test]
    fn comfyui_credentials_keep_their_workflows() {
        let comfy = ProviderAccountId::new();
        let config = json!({
            "txt2imgWorkflow": "{\"3\":{\"inputs\":{\"text\":\"%PROMPT%\"}}}",
            "img2imgWorkflow": "{\"4\":{}}"
        })
        .to_string();
        let plan = plan_legacy_backup_configuration(inventory(vec![document(
            LegacyBackupDocumentKind::ProviderCredentials,
            json!([{"id": comfy, "provider_id": "comfyui", "label": "Comfy", "config": config}]),
        )]))
        .expect("comfyui credentials plan");
        let account = &plan.provider_models.provider_accounts[0];
        assert_eq!(account.protocol, ProviderProtocol::StableDiffusion);
        assert!(account.deferred_config_fields.is_empty());
        assert_eq!(
            account.config,
            ProviderConfig::ComfyUi(lettuce_models::ComfyUiConfig {
                txt2img_workflow: Some(r#"{"3":{"inputs":{"text":"%PROMPT%"}}}"#.to_owned()),
                img2img_workflow: Some(r#"{"4":{}}"#.to_owned()),
            })
        );
    }

    #[test]
    fn malformed_provider_and_model_json_falls_back_like_legacy_restore() {
        let unparseable = ProviderAccountId::new();
        let non_text = ProviderAccountId::new();
        let malformed_model = ModelProfileId::new();
        let null_model = ModelProfileId::new();
        let plan = plan_legacy_backup_configuration(inventory(vec![
            document(
                LegacyBackupDocumentKind::ProviderCredentials,
                json!([
                    {"id": unparseable, "provider_id": "openai", "label": "Primary", "headers": "not-json", "config": "[true]"},
                    {"id": non_text, "provider_id": "anthropic", "label": "Secondary", "headers": "{\"X-Key\":1}", "config": "null"}
                ]),
            ),
            document(
                LegacyBackupDocumentKind::Models,
                json!([
                    {"id": malformed_model, "name": "model-a", "provider_id": "openai", "provider_credential_id": unparseable, "provider_label": "Primary", "display_name": "Model A", "created_at": 10, "input_scopes": "nope", "output_scopes": "[\"text\",3]", "advanced_model_settings": "[1]"},
                    {"id": null_model, "name": "model-b", "provider_id": "anthropic", "provider_credential_id": non_text, "provider_label": "Secondary", "display_name": "Model B", "created_at": 20, "model_type": "multimodel", "input_scopes": "null", "advanced_model_settings": "null"}
                ]),
            ),
        ]))
        .expect("malformed JSON falls back");
        let plan = plan.provider_models;
        assert!(plan.provider_accounts.iter().all(|provider| {
            provider.pending_secrets.is_empty()
                && provider.deferred_config_fields.is_empty()
                && provider.streaming_enabled
        }));
        let malformed = &plan.model_profiles[0];
        assert_eq!(malformed.id, malformed_model);
        assert_eq!(
            malformed.config.capabilities.input_modalities.text,
            CapabilityStatus::Supported
        );
        assert_eq!(
            malformed.config.capabilities.output_modalities.text,
            CapabilityStatus::Supported
        );
        assert!(malformed.deferred_advanced_fields.is_empty());
        let null_values = &plan.model_profiles[1];
        assert_eq!(null_values.id, null_model);
        assert_eq!(
            null_values.config.capabilities.input_modalities.image,
            CapabilityStatus::Unsupported
        );
        let skip = |field: &str, row_id: String| {
            crate::legacy_value_skip(
                field,
                &row_id,
                crate::LegacyImportSkipReason::MalformedLegacyValue,
            )
        };
        let mut expected = vec![
            skip("provider_credentials.config", unparseable.to_string()),
            skip("provider_credentials.headers", unparseable.to_string()),
            skip("provider_credentials.headers", non_text.to_string()),
            skip("models.input_scopes", malformed_model.to_string()),
            skip("models.output_scopes", malformed_model.to_string()),
            skip(
                "models.advanced_model_settings",
                malformed_model.to_string(),
            ),
        ];
        expected.sort();
        assert_eq!(plan.skipped, expected);
    }

    #[test]
    fn stale_settings_and_model_references_are_cleared_and_recorded() {
        let provider_id = ProviderAccountId::new();
        let model_id = ModelProfileId::new();
        let stale_provider = ProviderAccountId::new();
        let stale_model = ModelProfileId::new();
        let stale_summary_model = ModelProfileId::new();
        let plan = plan_legacy_backup_configuration(inventory(vec![
            document(
                LegacyBackupDocumentKind::Settings,
                json!({
                    "default_provider_credential_id": stale_provider,
                    "default_model_id": stale_model,
                    "prompt_template_id": "deleted-default",
                    "advanced_settings": {
                        "summarisationModelId": stale_summary_model,
                        "helpMeReplyRoleplayPromptTemplateId": "deleted-helper"
                    },
                    "created_at": 10,
                    "updated_at": 20
                }),
            ),
            document(
                LegacyBackupDocumentKind::ProviderCredentials,
                json!([{"id": provider_id, "provider_id": "openai", "label": "Primary"}]),
            ),
            document(
                LegacyBackupDocumentKind::Models,
                json!([{
                    "id": model_id,
                    "name": "model-a",
                    "provider_id": "openai",
                    "provider_credential_id": provider_id,
                    "provider_label": "Primary",
                    "display_name": "Model A",
                    "created_at": 10,
                    "prompt_template_id": "deleted-model-prompt"
                }]),
            ),
        ]))
        .expect("stale references are cleared");
        assert_eq!(plan.settings.default_provider_account_id, None);
        assert_eq!(plan.settings.default_model_profile_id, None);
        assert_eq!(plan.settings.default_prompt_source_id, None);
        assert_eq!(plan.settings.dynamic_memory_model_profile_id, None);
        assert_eq!(plan.settings.help_me_reply_prompt_source_ids.roleplay, None);
        assert_eq!(plan.provider_models.default_provider_account_id, None);
        assert_eq!(plan.provider_models.default_model_profile_id, None);
        assert_eq!(
            plan.provider_models.model_profiles[0].prompt_template_id,
            None
        );
        assert_eq!(plan.prompts.default_prompt_source_id, None);
        let skip = |kind, source_key: String, reason| crate::LegacyImportSkip {
            kind,
            source_key,
            reason,
        };
        let mut provider_skips = vec![
            skip(
                crate::LegacyImportSkipKind::SettingsDefaultProviderAccount,
                stale_provider.to_string(),
                crate::LegacyImportSkipReason::MissingProviderAccount,
            ),
            skip(
                crate::LegacyImportSkipKind::SettingsDefaultModelProfile,
                stale_model.to_string(),
                crate::LegacyImportSkipReason::MissingModelProfile,
            ),
            skip(
                crate::LegacyImportSkipKind::ModelReference,
                format!("settings.advanced_settings.summarisationModelId:{stale_summary_model}"),
                crate::LegacyImportSkipReason::MissingModelProfile,
            ),
        ];
        provider_skips.sort();
        assert_eq!(plan.provider_models.skipped, provider_skips);
        let mut prompt_skips = vec![
            skip(
                crate::LegacyImportSkipKind::PromptReference,
                "settings.prompt_template_id:deleted-default".to_owned(),
                crate::LegacyImportSkipReason::MissingPrompt,
            ),
            skip(
                crate::LegacyImportSkipKind::PromptReference,
                "settings.advanced_settings.helpMeReplyRoleplayPromptTemplateId:deleted-helper"
                    .to_owned(),
                crate::LegacyImportSkipReason::MissingPrompt,
            ),
            skip(
                crate::LegacyImportSkipKind::PromptReference,
                format!("models.prompt_template_id:{model_id}"),
                crate::LegacyImportSkipReason::MissingPrompt,
            ),
        ];
        prompt_skips.sort();
        assert_eq!(plan.prompts.skipped, prompt_skips);
    }

    #[test]
    fn settings_references_the_rewrite_cannot_run_are_cleared_and_recorded() {
        let provider_id = ProviderAccountId::new();
        let chat_model = ModelProfileId::new();
        let image_model = ModelProfileId::new();
        let plan = plan_legacy_backup_configuration(inventory(vec![
            document(
                LegacyBackupDocumentKind::Settings,
                json!({
                    "default_model_id": image_model,
                    "advanced_settings": {
                        "summarisationModelId": image_model,
                        "helpMeReplyModelId": chat_model,
                        "helpMeReplyRoleplayPromptTemplateId": "direct",
                        "dynamicMemorySummarizerPromptTemplateId": "summary"
                    },
                    "created_at": 10,
                    "updated_at": 20
                }),
            ),
            document(
                LegacyBackupDocumentKind::ProviderCredentials,
                json!([{"id": provider_id, "provider_id": "openai", "label": "Primary"}]),
            ),
            document(
                LegacyBackupDocumentKind::Models,
                json!([
                    {"id": chat_model, "name": "chat", "provider_id": "openai", "provider_credential_id": provider_id, "provider_label": "Primary", "display_name": "Chat", "created_at": 10},
                    {"id": image_model, "name": "image", "provider_id": "openai", "provider_credential_id": provider_id, "provider_label": "Primary", "display_name": "Image", "model_type": "imagegeneration", "created_at": 10}
                ]),
            ),
            document(
                LegacyBackupDocumentKind::PromptTemplates,
                json!([
                    {"id": "direct", "name": "Direct", "prompt_type": "directChat", "content": ""},
                    {"id": "summary", "name": "Summary", "prompt_type": "dynamicMemorySummarizer", "content": ""}
                ]),
            ),
        ]))
        .expect("incompatible references are cleared");
        assert_eq!(plan.settings.default_model_profile_id, None);
        assert_eq!(plan.provider_models.default_model_profile_id, None);
        assert_eq!(plan.settings.dynamic_memory_model_profile_id, None);
        assert_eq!(
            plan.settings.help_me_reply_model_profile_id,
            Some(chat_model)
        );
        assert_eq!(plan.settings.help_me_reply_prompt_source_ids.roleplay, None);
        assert_eq!(
            plan.settings
                .dynamic_memory_prompt_source_ids
                .summarizer
                .as_deref(),
            Some("summary")
        );
        let skip = |kind, source_key: String| crate::LegacyImportSkip {
            kind,
            source_key,
            reason: crate::LegacyImportSkipReason::IncompatibleReference,
        };
        let mut provider_skips = vec![
            skip(
                crate::LegacyImportSkipKind::SettingsDefaultModelProfile,
                image_model.to_string(),
            ),
            skip(
                crate::LegacyImportSkipKind::ModelReference,
                format!("settings.advanced_settings.summarisationModelId:{image_model}"),
            ),
        ];
        provider_skips.sort();
        assert_eq!(plan.provider_models.skipped, provider_skips);
        assert_eq!(
            plan.prompts.skipped,
            vec![skip(
                crate::LegacyImportSkipKind::PromptReference,
                "settings.advanced_settings.helpMeReplyRoleplayPromptTemplateId:direct".to_owned(),
            )]
        );
    }

    #[test]
    fn malformed_prompt_type_and_entries_fall_back_like_legacy_restore() {
        let plan = plan_legacy_backup_configuration(inventory(vec![document(
            LegacyBackupDocumentKind::PromptTemplates,
            json!([
                {"id": "prompt-unknown", "name": "Unknown", "prompt_type": "villain", "content": "Body", "entries": "not-json"},
                {"id": "prompt-number", "name": "Number", "prompt_type": 7, "content": "", "entries": {"entry": 1}},
                {"id": "prompt-missing", "name": "Missing", "content": ""},
                {"id": "prompt-snake", "name": "Snake", "prompt_type": "lorebook_entry_writer", "content": ""},
                {"id": "prompt-runtime", "name": "Runtime", "prompt_type": "runtimeText", "content": ""}
            ]),
        )]))
        .expect("malformed prompt values fall back");
        let prompts = plan.prompts;
        assert!(prompts.prompts.iter().all(|prompt| {
            prompt.purpose
                == if prompt.source_id == "prompt-snake" {
                    PromptPurpose::LorebookEntryWriter
                } else {
                    PromptPurpose::DirectChat
                }
        }));
        let unknown = prompts
            .prompts
            .iter()
            .find(|prompt| prompt.source_id == "prompt-unknown")
            .expect("unknown prompt");
        assert_eq!(unknown.entries.len(), 1);
        assert_eq!(unknown.entries[0].draft.content, "Body");
        let mut expected = vec![
            crate::legacy_value_skip(
                "prompt_templates.prompt_type",
                "prompt-runtime",
                crate::LegacyImportSkipReason::UnknownLegacyValue,
            ),
            crate::legacy_value_skip(
                "prompt_templates.prompt_type",
                "prompt-unknown",
                crate::LegacyImportSkipReason::UnknownLegacyValue,
            ),
            crate::legacy_value_skip(
                "prompt_templates.entries",
                "prompt-unknown",
                crate::LegacyImportSkipReason::MalformedLegacyValue,
            ),
            crate::legacy_value_skip(
                "prompt_templates.prompt_type",
                "prompt-number",
                crate::LegacyImportSkipReason::UnknownLegacyValue,
            ),
            crate::legacy_value_skip(
                "prompt_templates.entries",
                "prompt-number",
                crate::LegacyImportSkipReason::MalformedLegacyValue,
            ),
        ];
        expected.sort();
        assert_eq!(prompts.skipped, expected);
    }

    #[test]
    fn unparsable_settings_json_falls_back_to_defaults_and_is_recorded() {
        let plan = plan_legacy_backup_configuration(inventory(vec![document(
            LegacyBackupDocumentKind::Settings,
            json!({
                "default_provider_credential_id": null,
                "default_model_id": null,
                "app_state": "{broken",
                "advanced_settings": "not json",
                "advanced_model_settings": "{",
                "created_at": 10,
                "updated_at": 20
            }),
        )]))
        .expect("legacy fell back to defaults");
        assert_eq!(
            plan.settings.value.pure_mode,
            GlobalSettings::default().pure_mode
        );
        for field in ["app_state", "advanced_settings", "advanced_model_settings"] {
            assert!(plan.skipped.contains(&crate::legacy_value_skip(
                &format!("settings.{field}"),
                "1",
                crate::LegacyImportSkipReason::MalformedLegacyValue,
            )));
        }
    }

    #[test]
    fn image_feature_settings_keep_suitable_models_and_record_the_rest() {
        let provider_id = ProviderAccountId::new();
        let image_model = ModelProfileId::new();
        let vision_model = ModelProfileId::new();
        let missing_model = ModelProfileId::new();
        let model = |id: ModelProfileId, name: &str, input: &str, output: &str| {
            json!({
                "id": id,
                "name": name,
                "provider_id": "openrouter",
                "provider_credential_id": provider_id,
                "provider_label": "Router",
                "display_name": name,
                "created_at": 15,
                "model_type": "multimodel",
                "input_scopes": input,
                "output_scopes": output,
            })
        };
        let settings = |advanced: Value| {
            document(
                LegacyBackupDocumentKind::Settings,
                json!({
                    "default_provider_credential_id": null,
                    "default_model_id": null,
                    "app_state": {},
                    "advanced_settings": advanced,
                    "created_at": 10,
                    "updated_at": 20
                }),
            )
        };
        let documents = |advanced: Value| {
            vec![
                settings(advanced),
                document(
                    LegacyBackupDocumentKind::ProviderCredentials,
                    json!([{"id": provider_id, "provider_id": "openrouter", "label": "Router"}]),
                ),
                document(
                    LegacyBackupDocumentKind::Models,
                    json!([
                        model(image_model, "vendor/image", "[\"text\"]", "[\"image\"]"),
                        model(
                            vision_model,
                            "vendor/vision",
                            "[\"text\",\"image\"]",
                            "[\"text\"]"
                        ),
                    ]),
                ),
            ]
        };
        let plan = plan_legacy_backup_configuration(inventory(documents(json!({
            "avatarGenerationEnabled": false,
            "avatarGenerationModelId": image_model,
            "sceneGenerationEnabled": true,
            "sceneGenerationMode": "askFirst",
            "sceneGenerationModelId": image_model,
            "sceneWriterModelId": vision_model,
            "creationHelperImageModelId": image_model,
        }))))
        .expect("plan");
        let image = &plan.settings.value.image_generation;
        assert!(!image.avatar_enabled && image.scene_enabled);
        assert_eq!(image.scene_mode, SceneGenerationMode::AskFirst);
        assert_eq!(
            plan.settings.image_model_profile_ids,
            ImageModelSources {
                avatar: Some(image_model),
                scene: Some(image_model),
                scene_writer: Some(vision_model),
                creation_helper: Some(image_model),
            }
        );
        assert!(
            !plan
                .notices
                .iter()
                .any(|notice| notice.field.contains("Generation")
                    || notice.field.contains("sceneWriter")
                    || notice.field.contains("creationHelperImage"))
        );

        let plan = plan_legacy_backup_configuration(inventory(documents(json!({
            "avatarGenerationModelId": vision_model,
            "sceneWriterModelId": image_model,
            "creationHelperImageModelId": missing_model,
        }))))
        .expect("plan");
        let image = &plan.settings.value.image_generation;
        assert!(image.avatar_enabled && !image.scene_enabled);
        assert_eq!(image.scene_mode, SceneGenerationMode::Auto);
        assert_eq!(
            plan.settings.image_model_profile_ids,
            ImageModelSources::default()
        );
        let skip = |field: &str, id: ModelProfileId, reason| crate::LegacyImportSkip {
            kind: crate::LegacyImportSkipKind::ModelReference,
            source_key: format!("settings.advanced_settings.{field}:{id}"),
            reason,
        };
        for expected in [
            skip(
                "avatarGenerationModelId",
                vision_model,
                crate::LegacyImportSkipReason::IncompatibleReference,
            ),
            skip(
                "sceneWriterModelId",
                image_model,
                crate::LegacyImportSkipReason::IncompatibleReference,
            ),
            skip(
                "creationHelperImageModelId",
                missing_model,
                crate::LegacyImportSkipReason::MissingModelProfile,
            ),
        ] {
            assert!(
                plan.provider_models.skipped.contains(&expected),
                "{expected:?} in {:?}",
                plan.provider_models.skipped
            );
        }
        let unusable_scene = plan_legacy_backup_configuration(inventory(documents(json!({
            "sceneGenerationEnabled": true,
            "sceneGenerationModelId": missing_model,
        }))))
        .expect("plan");
        assert!(!unusable_scene.settings.value.image_generation.scene_enabled);
        assert!(matches!(
            plan_legacy_backup_configuration(inventory(documents(json!({
                "sceneGenerationMode": "sometimes"
            })))),
            Err(LegacyBackupConfigurationError::Malformed { ref field, .. })
                if field == "advanced_settings.sceneGenerationMode"
        ));
    }

    #[test]
    fn legacy_default_min_similarity_imports_as_unset_and_a_chosen_one_as_set() {
        for (value, expected) in [
            (None, None),
            (Some(json!(0.32)), None),
            (Some(json!(0.35)), None),
            (Some(json!(0.5)), Some(5_000)),
            (Some(json!(0.25)), Some(2_500)),
        ] {
            let mut object = json!({"maxEntries": 60});
            if let Some(value) = value {
                object["minSimilarityThreshold"] = value;
            }
            let mut notices = Vec::new();
            let settings = super::map_dynamic_memory(
                Some(&object),
                "advanced_settings.dynamicMemory",
                &mut notices,
            )
            .expect("dynamic memory");
            assert_eq!(settings.min_similarity_basis_points, expected);
        }
    }

    #[test]
    fn legacy_configuration_maps_graph_secrets_speech_and_retains_source() {
        let provider_id = ProviderAccountId::new();
        let model_id = ModelProfileId::new();
        let audio_id = AudioProviderId::new();
        let voice_id = VoiceProfileId::new();
        let mut advanced_settings = json!({
            "appUpdateChecksEnabled": false,
            "embeddingDimensions": 512,
            "manualModeContextWindow": 30,
            "summarisationModelId": model_id,
            "groupSpeakerSelectionModelId": model_id,
            "lorebookGeneratorModelId": model_id,
            "lorebookGeneratorDefaultTargetCount": 14,
            "lorebookGeneratorMaxTokens": 2048,
            "lorebookGeneratorPlannerPromptTemplateId": "prompt-main",
            "dynamicMemoryStructuredFallbackFormat": "json",
            "dynamicMemorySummarizerPromptTemplateId": "prompt-main",
            "dynamicMemoryManagerPromptTemplateId": " ",
            "dynamicMemoryLlamaSamplerOverwriteEnabled": false,
            "helpMeReplyEnabled": false,
            "helpMeReplyModelId": model_id,
            "helpMeReplyStreaming": false,
            "helpMeReplyMaxTokens": 220,
            "helpMeReplyHistoryCount": 0,
            "helpMeReplyStyle": "conversational",
            "helpMeReplyConversationalPromptTemplateId": "prompt-main",
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
        });
        advanced_settings
            .as_object_mut()
            .expect("advanced settings object")
            .extend(
                json!({
                    "creationHelperModelId": model_id,
                    "creationHelperStreaming": false,
                    "creationHelperEnabledTools": ["set_name", 7],
                    "creationHelperToolFallback": "XML",
                    "lorebookEntryGeneratorModelId": model_id,
                    "lorebookEntryGeneratorPromptTemplateId": "prompt-main",
                    "lorebookEntryGeneratorStructuredFallbackFormat": "xml",
                    "companionSoulWriterFallbackModelId": model_id,
                    "companionSoulWriterStructuredFallbackFormat": "xml",
                    "navigationStyle": "sidebar",
                    "hostApi": {
                        "enabled": true,
                        "bindAddress": "127.0.0.1",
                        "port": 4444,
                        "token": "secret-token",
                        "exposedModels": [
                            {"id": "main", "modelId": model_id, "label": "L".repeat(300)},
                            {"id": "gone", "modelId": ModelProfileId::new()}
                        ]
                    },
                    "embeddingModelVersion": "v4",
                    "embeddingMaxTokens": 4096,
                    "embeddingKeepModelLoaded": true,
                    "customLlmModelsDir": " /models/gguf ",
                    "sdDefaultSize": "768x768"
                })
                .as_object()
                .expect("feature settings")
                .clone(),
            );
        let documents = vec![
            document(
                LegacyBackupDocumentKind::Settings,
                json!({
                    "default_provider_credential_id": provider_id,
                    "default_model_id": model_id,
                    "app_state": {"pureModeEnabled": false, "analyticsEnabled": false, "theme": "dark", "customColors": {"accent": "#abcdef"}, "onboarding": {"completed": true, "skipped": false, "providerSetupCompleted": true, "modelSetupCompleted": true}, "appActiveUsageMs": 9000, "appActiveUsageByDayMs": {"2026-09-01": 3000, "2026-09-02": 4000, "bad": 5}, "appActiveUsageStartedAtMs": 1, "appActiveUsageLastUpdatedAtMs": 2, "autoDownloadCharacterCardAvatars": false, "trustedCertificates": [{"id": "5b1f7a8e-8c43-4d7c-9f0e-2d5b7a1c3e44", "name": "corp.pem", "pem": "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----", "importedAt": 5}, {"id": "bad", "name": "x", "pem": "y", "importedAt": 1}]},
                    "advanced_model_settings": {"temperature": 0.2, "topK": 5},
                    "prompt_template_id": "prompt-main",
                    "system_prompt": "Old global prompt",
                    "migration_version": 92,
                    "advanced_settings": advanced_settings,
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
        assert_eq!(
            plan.settings.model_settings.chat_parameters.temperature,
            Some(0.2)
        );
        assert_eq!(plan.settings.model_settings.chat_parameters.top_k, None);
        assert!(plan.notices.iter().any(|notice| {
            notice.kind == LegacyBackupConversionNoticeKind::Lossy
                && notice.document == LegacyBackupDocumentKind::Settings
                && notice.field == "advanced_model_settings.topK"
        }));
        assert!(!plan.settings.value.analytics_enabled);
        assert!(!plan.settings.value.update_checks_enabled);
        assert_eq!(plan.settings.value.dynamic_memory.max_entries, 60);
        assert_eq!(
            plan.settings
                .value
                .dynamic_memory
                .min_similarity_basis_points,
            Some(4200)
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
        assert_eq!(plan.settings.value.manual_mode_context_window, 30);
        let memory = &plan.settings.value.dynamic_memory;
        assert!(memory.enabled);
        assert_eq!(memory.summary_message_interval, 12);
        assert_eq!(memory.run_mode, MemoryRunMode::AskFirst);
        assert!(memory.recursive_memory_loops);
        assert_eq!(memory.recursive_memory_loop_hard_cap, 6);
        assert_eq!(memory.decay_rate_basis_points, 1_000);
        assert_eq!(memory.delete_confidence_basis_points, 7_000);
        assert_eq!(memory.max_hard_delete_ratio_basis_points, 2_500);
        assert_eq!(
            memory.structured_fallback_format,
            MemoryStructuredFallbackFormat::Json
        );
        assert!(!plan.notices.iter().any(|notice| {
            notice.field == "advanced_settings.dynamicMemoryStructuredFallbackFormat"
                || notice.field == "advanced_settings.dynamicMemorySummarizerPromptTemplateId"
                || notice.field == "advanced_settings.dynamicMemoryManagerPromptTemplateId"
        }));
        assert_eq!(
            plan.settings.dynamic_memory_prompt_source_ids,
            DynamicMemoryPromptSources::default()
        );
        assert!(
            plan.prompts.skipped.contains(&crate::LegacyImportSkip {
                kind: crate::LegacyImportSkipKind::PromptReference,
                source_key:
                    "settings.advanced_settings.dynamicMemorySummarizerPromptTemplateId:prompt-main"
                        .to_owned(),
                reason: crate::LegacyImportSkipReason::IncompatibleReference,
            })
        );
        assert!(
            !plan
                .settings
                .value
                .dynamic_memory_llama_sampler_overwrite_enabled
        );
        let help_me_reply = &plan.settings.value.help_me_reply;
        assert!(!help_me_reply.enabled && !help_me_reply.streaming);
        assert_eq!(help_me_reply.max_output_tokens, 220);
        assert_eq!(help_me_reply.history_count(), 10);
        assert_eq!(help_me_reply.style, HelpMeReplyStyle::Conversational);
        assert_eq!(plan.settings.help_me_reply_model_profile_id, Some(model_id));
        let preferences = &plan.settings.value.ui_preferences.0;
        assert_eq!(preferences.get("theme"), Some(&json!("dark")));
        assert_eq!(
            preferences.get("customColors"),
            Some(&json!({"accent": "#abcdef"}))
        );
        assert_eq!(preferences.get("navigationStyle"), Some(&json!("sidebar")));
        assert_eq!(
            plan.settings
                .device_ui_state
                .get("onboarding")
                .and_then(|value| value.get("completed")),
            Some(&json!(true))
        );
        assert!(!plan.settings.value.auto_download_character_card_avatars);
        assert_eq!(
            plan.settings.app_usage_days,
            vec![
                lettuce_usage::AppUsageDay {
                    day: "2026-09-01".into(),
                    active_ms: 3_000,
                },
                lettuce_usage::AppUsageDay {
                    day: "2026-09-02".into(),
                    active_ms: 4_000,
                },
            ]
        );
        assert!(
            !plan
                .settings
                .device_ui_state
                .contains_key("appActiveUsageMs")
        );
        for field in [
            "app_state.appActiveUsageByDayMs",
            "app_state.appActiveUsageMs",
        ] {
            assert!(plan.notices.iter().any(|notice| notice.field == field));
        }
        assert!(
            !plan
                .notices
                .iter()
                .any(|notice| notice.field == "app_state.appActiveUsageStartedAtMs")
        );
        let device = &plan.settings.device_settings;
        assert_eq!(device.trusted_certificates.len(), 1);
        assert_eq!(device.trusted_certificates[0].imported_at, 5);
        assert_eq!(device.embedding.max_tokens, Some(4096));
        assert!(device.embedding.keep_model_loaded);
        assert_eq!(device.llm_models_dir.as_deref(), Some("/models/gguf"));
        assert_eq!(
            plan.settings
                .value
                .image_generation
                .scene_default_size
                .as_deref(),
            Some("768x768")
        );
        let field_notice = |field: &str| plan.notices.iter().any(|notice| notice.field == field);
        assert!(field_notice("app_state.trustedCertificates[1]"));
        assert!(plan.notices.iter().any(|notice| {
            notice.kind == LegacyBackupConversionNoticeKind::Unsupported
                && notice.field == "advanced_settings.hostApi"
        }));
        assert!(!field_notice("advanced_settings.hostApi.token"));
        assert!(
            !plan
                .notices
                .iter()
                .any(|notice| notice.field == "app_state.theme"
                    || notice.field == "app_state.onboarding"
                    || notice.field == "advanced_settings.navigationStyle")
        );
        let creation_helper = &plan.settings.value.creation_helper;
        assert!(!creation_helper.streaming);
        assert_eq!(
            creation_helper.enabled_tools.as_deref(),
            Some(&["set_name".to_owned()][..])
        );
        assert_eq!(
            creation_helper.tool_fallback,
            CreationHelperToolFallback::Xml
        );
        assert_eq!(
            plan.settings
                .value
                .lorebook_entry_generator
                .structured_fallback_format,
            MemoryStructuredFallbackFormat::Xml
        );
        assert_eq!(
            plan.settings
                .value
                .companion_soul_writer
                .structured_fallback_format,
            MemoryStructuredFallbackFormat::Xml
        );
        assert_eq!(
            plan.settings.feature_model_profile_ids,
            FeatureModelSources {
                creation_helper: Some(model_id),
                lorebook_entry: Some(model_id),
                soul_writer: None,
                soul_writer_fallback: Some(model_id),
            }
        );
        assert_eq!(plan.settings.feature_prompt_source_ids.lorebook_entry, None);
        assert!(plan.prompts.skipped.iter().any(|skip| skip.source_key
            == "settings.advanced_settings.lorebookEntryGeneratorPromptTemplateId:prompt-main"));
        assert!(!plan.notices.iter().any(|notice| {
            notice.field.starts_with("advanced_settings.creationHelper")
                || notice
                    .field
                    .starts_with("advanced_settings.companionSoulWriter")
                || notice
                    .field
                    .starts_with("advanced_settings.lorebookEntryGenerator")
        }));
        assert_eq!(
            plan.settings.help_me_reply_prompt_source_ids,
            HelpMeReplyPromptSources::default()
        );
        assert!(plan.prompts.skipped.contains(&crate::LegacyImportSkip {
            kind: crate::LegacyImportSkipKind::PromptReference,
            source_key:
                "settings.advanced_settings.helpMeReplyConversationalPromptTemplateId:prompt-main"
                    .to_owned(),
            reason: crate::LegacyImportSkipReason::IncompatibleReference,
        }));
        assert!(plan.notices.iter().any(|notice| notice.kind
            == LegacyBackupConversionNoticeKind::Lossy
            && notice.field == "advanced_settings.helpMeReplyHistoryCount"));
        assert!(
            !plan
                .notices
                .iter()
                .any(|notice| notice.field == "advanced_settings.dynamicMemory.decayRate")
        );
        assert!(plan.notices.iter().any(|notice| notice.kind
            == LegacyBackupConversionNoticeKind::Unsupported
            && notice.field == "advanced_settings.dynamicMemory.unknownKnob"));
        assert!(plan.notices.iter().any(|notice| notice.kind
            == LegacyBackupConversionNoticeKind::Lossy
            && notice.document == LegacyBackupDocumentKind::AudioProviders
            && notice.field == "[1].id"));
    }

    #[test]
    fn advanced_model_settings_move_into_typed_model_settings() {
        use lettuce_models::{
            LlamaKvPlacement, LlamaKvType, LlamaSamplerProfile, LlamaSamplerStage,
            ParameterOverride, StableDiffusionCacheMode,
        };

        let advanced = json!({
            "temperature": 0.7,
            "ollamaRepeatPenalty": 0,
            "forceSendThinkingState": true,
            "llamaGpuLayers": 33,
            "llamaKvType": "q8_0",
            "llamaKvPlacement": "systemRam",
            "llamaThreads": 999,
            "llamaSamplerProfile": " Creative ",
            "llamaSamplerOrder": ["TopK", "adaptive", "bogus", "temperature", "top_k"],
            "llamaAdaptiveTarget": 0.4,
            "llamaAdaptiveDecay": 1.5,
            "forceGemma4Reasoning": true,
            "llamaMtpModelPath": null,
            "llamaLastRuntimeReport": {"backend": "vulkan"},
            "sdSteps": 28,
            "sdCacheMode": "cache-dit",
            "sdBaseLoras": [{"path": "/loras/a.safetensors", "multiplier": 0.8}],
            "sdcppVaePath": "/models/vae.safetensors",
            "futureKey": 1,
            "contextLength": 0,
            "ollamaNumCtx": 8192,
            "reasoningBudgetTokens": 512,
            "featureGenerationSettings": {
                "lorebookGenerator": {
                    "temperature": 0.25,
                    "maxOutputTokens": 900,
                    "ollamaStop": ["</entry>"],
                    "llamaMinP": 0.05,
                    "llamaSeed": null
                },
                "dynamicMemory": {"temperature": 0.4, "llamaXtcProbability": 0.2},
                "helpMeReply": {"ollamaStop": vec!["x"; 257]},
                "sceneWriter": null
            }
        });
        let parameters =
            crate::legacy_model_parameters("llamacpp", advanced.as_object().expect("object"));
        assert_eq!(parameters.chat_parameters.temperature, Some(0.7));
        assert_eq!(parameters.chat_parameters.repetition_penalty, None);
        assert_eq!(parameters.chat_parameters.send_thinking_state, Some(true));
        assert_eq!(parameters.chat_parameters.context_length, None);
        assert_eq!(parameters.chat_parameters.ollama.num_ctx, Some(8192));
        assert_eq!(parameters.chat_parameters.reasoning_budget_tokens, None);
        let llama = &parameters.llama_cpp;
        assert_eq!(llama.gpu_layers, Some(33));
        assert_eq!(llama.kv_type, Some(LlamaKvType::Q80));
        assert_eq!(llama.kv_placement, Some(LlamaKvPlacement::SystemRam));
        assert_eq!(llama.threads, None);
        assert_eq!(llama.sampler.profile, Some(LlamaSamplerProfile::Creative));
        assert_eq!(
            llama.sampler.order,
            Some(vec![
                LlamaSamplerStage::TopK,
                LlamaSamplerStage::AdaptiveP,
                LlamaSamplerStage::Temp
            ])
        );
        assert_eq!(llama.sampler.adaptive_target, Some(0.4));
        assert_eq!(llama.sampler.adaptive_decay, None);
        assert_eq!(llama.force_gemma4_reasoning, Some(true));
        let diffusion = &parameters.stable_diffusion;
        assert_eq!(diffusion.steps, Some(28));
        assert_eq!(
            diffusion.cache_mode,
            Some(StableDiffusionCacheMode::CacheDit)
        );
        assert_eq!(diffusion.base_loras.as_ref().map(Vec::len), Some(1));
        assert_eq!(
            diffusion.cpp.vae_path.as_deref(),
            Some("/models/vae.safetensors")
        );
        let lorebook = &parameters.feature_parameters.lorebook_generator;
        assert_eq!(
            lorebook.parameters.temperature,
            ParameterOverride::Set(0.25)
        );
        assert_eq!(
            lorebook.parameters.max_output_tokens,
            ParameterOverride::Set(900)
        );
        assert_eq!(
            lorebook.parameters.ollama.stop,
            ParameterOverride::Set(vec!["</entry>".to_owned()])
        );
        assert_eq!(lorebook.parameters.top_p, ParameterOverride::Inherit);
        assert_eq!(lorebook.llama_sampler.min_p, Some(0.05));
        let memory = &parameters.feature_parameters.dynamic_memory;
        assert_eq!(memory.parameters.temperature, ParameterOverride::Set(0.4));
        assert_eq!(memory.llama_sampler.xtc_probability, Some(0.2));
        assert_eq!(
            parameters.lossy_fields,
            vec![
                "featureGenerationSettings.helpMeReply.ollamaStop".to_owned(),
                "llamaAdaptiveDecay".to_owned(),
                "llamaLastRuntimeReport".to_owned(),
                "llamaThreads".to_owned(),
                "ollamaRepeatPenalty".to_owned(),
                "reasoningBudgetTokens".to_owned(),
            ]
        );
        assert_eq!(parameters.unknown_fields, vec!["futureKey".to_owned()]);
        let config = ModelProfileConfig {
            chat_parameters: parameters.chat_parameters,
            feature_parameters: parameters.feature_parameters,
            capabilities: ModelCapabilities::default(),
            llama_cpp: parameters.llama_cpp,
            stable_diffusion: parameters.stable_diffusion,
        };
        config.validate_parameters().expect("valid typed settings");
        let round_trip: ModelProfileConfig =
            serde_json::from_value(serde_json::to_value(&config).expect("serialize config"))
                .expect("deserialize config");
        assert_eq!(round_trip, config);
    }

    #[test]
    fn planned_configuration_serves_provider_secrets_by_admission_source() {
        use crate::LegacyProviderSecretSource;

        let provider_id = ProviderAccountId::new();
        let plan = plan_legacy_backup_configuration(inventory(vec![document(
            LegacyBackupDocumentKind::ProviderCredentials,
            json!([{
                "id": provider_id,
                "provider_id": "openrouter",
                "label": "Router",
                "api_key_ref": null,
                "api_key": "provider-secret",
                "base_url": "https://openrouter.ai/api/v1",
                "default_model": null,
                "headers": "{\"X-Client\":\"header-secret\"}",
                "config": null
            }]),
        )]))
        .expect("configuration plan");
        let sources = plan.sources().expect("secret sources");
        assert_eq!(sources.len(), 2);
        let values = sources
            .iter()
            .map(|source| {
                plan.load(source)
                    .expect("secret value")
                    .with(|value| value.to_owned())
            })
            .collect::<Vec<_>>();
        assert!(values.iter().any(|value| value == "provider-secret"));
        assert!(values.iter().any(|value| value == "header-secret"));
        assert_eq!(
            plan.load(&crate::LegacyImportProviderSecretSource {
                provider_account_id: ProviderAccountId::new(),
                secret: LegacyPendingProviderSecret::ApiKey,
            })
            .err(),
            Some(crate::LegacyProviderSecretSourceError::Unavailable)
        );
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
