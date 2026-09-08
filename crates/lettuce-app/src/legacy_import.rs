use lettuce_transfer::{
    LEGACY_DATABASE_SCHEMA_VERSION, LegacyCrop, LegacyDatabaseInventory, LegacyImageRecommendation,
    LegacyImportAdmission, LegacyImportAdmissionRequest, LegacyImportExecutionRequest,
    LegacyImportPlan, LegacyImportProviderSecretSource, LegacyImportReceipt,
    LegacyImportRepository, LegacyImportRepositoryError, LegacyImportSources,
    LegacyKeywordMatchMode, LegacyLorebookDetectionPolicy, LegacyLorebookPlan, LegacyMediaPlan,
    LegacyMediaUse, LegacyPendingProviderSecret, LegacyPersonaPlan, LegacyProviderAccountOrigin,
    LegacyProviderModelPlan,
};
use lettuce_types::{ContentHash, LegacyImportRunId, TimestampMillis};

#[derive(Debug)]
pub struct LegacyImportAdmissionCoordinator<'a, R: ?Sized> {
    repository: &'a R,
}

#[derive(Debug)]
pub struct LegacyImportExecutionCoordinator<'a, R: ?Sized> {
    repository: &'a R,
}

impl<'a, R: LegacyImportRepository + ?Sized> LegacyImportExecutionCoordinator<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    pub fn execute(
        &self,
        admission: &LegacyImportAdmission,
        provider_models: &LegacyProviderModelPlan,
        personas: &LegacyPersonaPlan,
        lorebooks: &LegacyLorebookPlan,
        media: &LegacyMediaPlan,
        completed_at: TimestampMillis,
    ) -> Result<LegacyImportReceipt, LegacyImportRepositoryError> {
        let fingerprint = plan_fingerprint(provider_models, personas, lorebooks, media);
        if fingerprint != admission.plan_fingerprint {
            return Err(LegacyImportRepositoryError::Conflict);
        }
        self.repository.materialize(LegacyImportExecutionRequest {
            run_id: admission.run_id,
            plan_fingerprint: fingerprint,
            provider_models: provider_models.clone(),
            personas: personas.clone(),
            lorebooks: lorebooks.clone(),
            media: media.clone(),
            completed_at,
        })
    }
}

impl<'a, R: LegacyImportRepository + ?Sized> LegacyImportAdmissionCoordinator<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    pub fn admit(
        &self,
        run_id: LegacyImportRunId,
        inventory: &LegacyDatabaseInventory,
        plan: &LegacyImportPlan,
        admitted_at: TimestampMillis,
    ) -> Result<LegacyImportAdmission, LegacyImportRepositoryError> {
        let LegacyImportPlan {
            provider_models,
            personas,
            lorebooks,
            media,
        } = plan;
        validate_plan(inventory, provider_models, personas, lorebooks, media)?;
        self.repository.admit(LegacyImportAdmissionRequest {
            run_id,
            source_schema_version: inventory.schema_version,
            inventory_fingerprint: inventory_fingerprint(inventory),
            plan_fingerprint: plan_fingerprint(provider_models, personas, lorebooks, media),
            sources: LegacyImportSources {
                provider_account_ids: provider_models
                    .provider_accounts
                    .iter()
                    .map(|provider| provider.id)
                    .collect(),
                model_profile_ids: provider_models
                    .model_profiles
                    .iter()
                    .map(|model| model.id)
                    .collect(),
                provider_secrets: provider_models
                    .provider_accounts
                    .iter()
                    .flat_map(|provider| {
                        provider.pending_secrets.iter().cloned().map(|secret| {
                            LegacyImportProviderSecretSource {
                                provider_account_id: provider.id,
                                secret,
                            }
                        })
                    })
                    .collect(),
                persona_ids: personas.personas.iter().map(|persona| persona.id).collect(),
                lorebook_ids: lorebooks
                    .lorebooks
                    .iter()
                    .map(|lorebook| lorebook.id)
                    .collect(),
                lorebook_entry_ids: lorebooks
                    .lorebooks
                    .iter()
                    .flat_map(|lorebook| lorebook.entries.iter().map(|entry| entry.id))
                    .collect(),
                media: media
                    .media
                    .iter()
                    .map(|candidate| lettuce_transfer::LegacyImportMediaSource {
                        relative_path: candidate.relative_path.clone(),
                        byte_len: candidate.byte_len,
                        content_hash: candidate.content_hash.clone(),
                    })
                    .collect(),
            },
            admitted_at,
        })
    }
}

fn validate_plan(
    inventory: &LegacyDatabaseInventory,
    provider_models: &LegacyProviderModelPlan,
    personas: &LegacyPersonaPlan,
    lorebooks: &LegacyLorebookPlan,
    media: &LegacyMediaPlan,
) -> Result<(), LegacyImportRepositoryError> {
    let persona_count = u64::try_from(personas.personas.len())
        .map_err(|_| LegacyImportRepositoryError::InvalidInput)?;
    let lorebook_count = u64::try_from(lorebooks.lorebooks.len())
        .map_err(|_| LegacyImportRepositoryError::InvalidInput)?;
    let provider_count = provider_models
        .provider_accounts
        .iter()
        .filter(|provider| provider.origin == LegacyProviderAccountOrigin::Stored)
        .count();
    let provider_count =
        u64::try_from(provider_count).map_err(|_| LegacyImportRepositoryError::InvalidInput)?;
    let model_count = u64::try_from(provider_models.model_profiles.len())
        .map_err(|_| LegacyImportRepositoryError::InvalidInput)?;
    let total_bytes = media.media.iter().try_fold(0_u64, |total, candidate| {
        total.checked_add(candidate.byte_len)
    });
    if inventory.schema_version != LEGACY_DATABASE_SCHEMA_VERSION
        || inventory.personas != persona_count
        || inventory.lorebooks != lorebook_count
        || inventory.provider_accounts != provider_count
        || inventory.models != model_count
        || !valid_provider_model_plan(provider_models)
        || total_bytes != Some(media.total_bytes)
    {
        return Err(LegacyImportRepositoryError::InvalidInput);
    }
    Ok(())
}

fn valid_provider_model_plan(plan: &LegacyProviderModelPlan) -> bool {
    let provider_ids = plan
        .provider_accounts
        .iter()
        .map(|provider| provider.id)
        .collect::<std::collections::BTreeSet<_>>();
    let model_ids = plan
        .model_profiles
        .iter()
        .map(|model| model.id)
        .collect::<std::collections::BTreeSet<_>>();
    provider_ids.len() == plan.provider_accounts.len()
        && model_ids.len() == plan.model_profiles.len()
        && plan.provider_accounts.iter().all(|provider| {
            provider
                .pending_secrets
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == provider.pending_secrets.len()
        })
        && plan
            .model_profiles
            .iter()
            .all(|model| provider_ids.contains(&model.provider_account_id))
        && plan
            .default_provider_account_id
            .is_none_or(|id| provider_ids.contains(&id))
        && plan
            .default_model_profile_id
            .is_none_or(|id| model_ids.contains(&id))
}

struct Fingerprint(blake3::Hasher);

impl Fingerprint {
    fn new(domain: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        write_bytes(&mut hasher, domain.as_bytes());
        Self(hasher)
    }

    fn bytes(&mut self, value: &[u8]) {
        write_bytes(&mut self.0, value);
    }

    fn text(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    fn bool(&mut self, value: bool) {
        self.bytes(&[u8::from(value)]);
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn i32(&mut self, value: i32) {
        self.bytes(&value.to_le_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.bytes(&value.to_le_bytes());
    }

    fn f64(&mut self, value: f64) {
        self.u64(value.to_bits());
    }

    fn option<T>(&mut self, value: Option<&T>, write: impl FnOnce(&mut Self, &T)) {
        self.bool(value.is_some());
        if let Some(value) = value {
            write(self, value);
        }
    }

    fn finish(self) -> ContentHash {
        ContentHash::parse(self.0.finalize().to_hex().to_string())
            .expect("BLAKE3 always produces a valid content hash")
    }
}

fn write_bytes(hasher: &mut blake3::Hasher, value: &[u8]) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value);
}

fn inventory_fingerprint(inventory: &LegacyDatabaseInventory) -> ContentHash {
    let mut hash = Fingerprint::new("lettuce-legacy-inventory-v1");
    hash.u32(inventory.schema_version);
    hash.u64(inventory.provider_accounts);
    hash.u64(inventory.models);
    hash.u64(inventory.prompts);
    hash.u64(inventory.personas);
    hash.u64(inventory.characters);
    hash.u64(inventory.lorebooks);
    hash.u64(inventory.chat_templates);
    hash.u64(inventory.direct_conversations);
    hash.u64(inventory.group_profiles);
    hash.u64(inventory.group_conversations);
    hash.finish()
}

pub(crate) fn plan_fingerprint(
    provider_models: &LegacyProviderModelPlan,
    personas: &LegacyPersonaPlan,
    lorebooks: &LegacyLorebookPlan,
    media: &LegacyMediaPlan,
) -> ContentHash {
    let mut hash = Fingerprint::new("lettuce-legacy-import-plan-v2");
    hash.u64(provider_models.provider_accounts.len() as u64);
    for provider in &provider_models.provider_accounts {
        hash.text(&provider.id.to_string());
        hash.u32(match provider.origin {
            LegacyProviderAccountOrigin::Stored => 1,
            LegacyProviderAccountOrigin::BuiltInLlamaCpp => 2,
        });
        hash.text(&provider.secret_owner_id.as_uuid().to_string());
        hash.text(&provider.provider_kind);
        hash.bytes(&serde_json::to_vec(&provider.protocol).expect("provider protocol serializes"));
        hash.text(&provider.label);
        hash.option(provider.endpoint.as_ref(), |hash, value| hash.text(value));
        hash.bool(provider.enabled);
        hash.bool(provider.streaming_enabled);
        hash.bool(provider.allow_invalid_tls);
        hash.option(provider.default_model.as_ref(), |hash, value| {
            hash.text(value)
        });
        hash.bytes(&serde_json::to_vec(&provider.config).expect("provider config serializes"));
        hash.u64(provider.pending_secrets.len() as u64);
        for secret in &provider.pending_secrets {
            match secret {
                LegacyPendingProviderSecret::ApiKey => hash.u32(1),
                LegacyPendingProviderSecret::Header { name } => {
                    hash.u32(2);
                    hash.text(name.as_str());
                }
            }
        }
        hash.u64(provider.deferred_config_fields.len() as u64);
        for field in &provider.deferred_config_fields {
            hash.text(field);
        }
        hash.i64(provider.created_at.get());
        hash.i64(provider.updated_at.get());
    }
    hash.u64(provider_models.model_profiles.len() as u64);
    for model in &provider_models.model_profiles {
        hash.text(&model.id.to_string());
        hash.text(&model.provider_account_id.to_string());
        hash.text(&model.source_provider_kind);
        hash.text(&model.source_provider_label);
        hash.text(&model.external_model_id);
        hash.text(&model.display_name);
        hash.bytes(&serde_json::to_vec(&model.kind).expect("model kind serializes"));
        hash.bytes(&serde_json::to_vec(&model.config).expect("model config serializes"));
        hash.option(model.prompt_template_id.as_ref(), |hash, value| {
            hash.text(value)
        });
        hash.option(model.deprecated_system_prompt.as_ref(), |hash, value| {
            hash.text(value)
        });
        hash.u64(model.deferred_advanced_fields.len() as u64);
        for field in &model.deferred_advanced_fields {
            hash.text(field);
        }
        hash.i64(model.created_at.get());
    }
    hash.option(
        provider_models.default_provider_account_id.as_ref(),
        |hash, value| hash.text(&value.to_string()),
    );
    hash.option(
        provider_models.default_model_profile_id.as_ref(),
        |hash, value| hash.text(&value.to_string()),
    );
    hash.u64(personas.personas.len() as u64);
    for persona in &personas.personas {
        hash.text(&persona.id.to_string());
        hash.text(&persona.title);
        hash.text(&persona.description);
        hash.option(persona.nickname.as_ref(), |hash, value| hash.text(value));
        hash.option(persona.avatar.as_ref(), |hash, value| {
            hash.text(&value.locator)
        });
        hash.option(persona.avatar_crop.as_ref(), write_crop);
        hash.option(persona.design_description.as_ref(), |hash, value| {
            hash.text(value)
        });
        hash.u64(persona.design_references.len() as u64);
        for reference in &persona.design_references {
            hash.text(&reference.locator);
        }
        hash.option(persona.image_recommendation.as_ref(), write_recommendation);
        hash.u64(persona.active_lorebook_ids.len() as u64);
        for lorebook_id in &persona.active_lorebook_ids {
            hash.text(&lorebook_id.to_string());
        }
        hash.i64(persona.created_at.get());
        hash.i64(persona.updated_at.get());
    }
    hash.option(personas.default_persona_id.as_ref(), |hash, value| {
        hash.text(&value.to_string())
    });
    hash.u64(lorebooks.lorebooks.len() as u64);
    for lorebook in &lorebooks.lorebooks {
        hash.text(&lorebook.id.to_string());
        hash.text(&lorebook.name);
        hash.option(lorebook.avatar.as_ref(), |hash, value| {
            hash.text(&value.locator)
        });
        hash.u32(match lorebook.detection_policy {
            LegacyLorebookDetectionPolicy::RecentMessageWindow => 1,
            LegacyLorebookDetectionPolicy::LatestUserMessage => 2,
        });
        hash.u64(lorebook.entries.len() as u64);
        for entry in &lorebook.entries {
            hash.text(&entry.id.to_string());
            hash.text(&entry.title);
            hash.bool(entry.enabled);
            hash.bool(entry.always_active);
            hash.u64(entry.keywords.len() as u64);
            for keyword in &entry.keywords {
                hash.text(keyword);
            }
            hash.bool(entry.case_sensitive);
            hash.u32(match entry.match_mode {
                LegacyKeywordMatchMode::Literal => 1,
                LegacyKeywordMatchMode::Regex => 2,
            });
            hash.text(&entry.content);
            hash.i32(entry.priority);
            hash.i32(entry.display_order);
            hash.i64(entry.created_at.get());
            hash.i64(entry.updated_at.get());
        }
        hash.i64(lorebook.created_at.get());
        hash.i64(lorebook.updated_at.get());
    }
    hash.u64(media.media.len() as u64);
    for candidate in &media.media {
        hash.text(&candidate.relative_path);
        hash.u64(candidate.byte_len);
        hash.text(candidate.content_hash.as_str());
        hash.u64(candidate.uses.len() as u64);
        for media_use in &candidate.uses {
            match media_use {
                LegacyMediaUse::PersonaAvatar { persona_id } => {
                    hash.u32(1);
                    hash.text(&persona_id.to_string());
                }
                LegacyMediaUse::PersonaDesignReference {
                    persona_id,
                    ordinal,
                } => {
                    hash.u32(2);
                    hash.text(&persona_id.to_string());
                    hash.u32(*ordinal);
                }
                LegacyMediaUse::LorebookAvatar { lorebook_id } => {
                    hash.u32(3);
                    hash.text(&lorebook_id.to_string());
                }
            }
        }
    }
    hash.u64(media.total_bytes);
    hash.finish()
}

fn write_crop(hash: &mut Fingerprint, crop: &LegacyCrop) {
    hash.f64(crop.x);
    hash.f64(crop.y);
    hash.f64(crop.scale);
}

fn write_recommendation(hash: &mut Fingerprint, recommendation: &LegacyImageRecommendation) {
    hash.text(&recommendation.model_name);
    hash.f64(recommendation.strength);
}

#[cfg(test)]
mod tests {
    use std::{fs, str::FromStr};

    use lettuce_characters::{Persona, PersonaRepository};
    use lettuce_context::LorebookRepository;
    use lettuce_models::{
        ModelProfile, ModelProfileConfig, ModelProfileRepository, ProviderAccount,
        ProviderAccountRepository, ProviderConfig, ProviderProtocol,
    };
    use lettuce_settings::{
        GlobalSettingsStore, HeaderName, InMemorySecretStore, SecretOwnerId, SecretPurpose,
        SecretRecord, SecretStore, SecretValue,
    };
    use lettuce_transfer::{
        LegacyDatabaseInventory, LegacyImportAssignment, LegacyImportPlan, LegacyImportRepository,
        LegacyImportRepositoryError, LegacyImportRunStatus, LegacyImportSecretCompletionRequest,
        LegacyLorebookCandidate, LegacyLorebookDetectionPolicy, LegacyLorebookPlan,
        LegacyMediaPlan, LegacyPendingProviderSecret, LegacyPersonaCandidate, LegacyPersonaPlan,
        LegacyProviderAccountCandidate, LegacyProviderAccountOrigin, LegacyProviderModelPlan,
    };
    use lettuce_types::{
        LegacyImportRunId, LorebookId, ModelProfileId, PersonaId, ProviderAccountId, Revision,
        TimestampMillis,
    };

    use crate::AppBackend;

    fn inventory() -> LegacyDatabaseInventory {
        LegacyDatabaseInventory {
            schema_version: 92,
            provider_accounts: 0,
            models: 0,
            prompts: 0,
            personas: 1,
            characters: 0,
            lorebooks: 0,
            chat_templates: 0,
            direct_conversations: 0,
            group_profiles: 0,
            group_conversations: 0,
        }
    }

    fn personas() -> LegacyPersonaPlan {
        LegacyPersonaPlan {
            personas: vec![LegacyPersonaCandidate {
                id: PersonaId::new(),
                title: "Owner".to_owned(),
                description: "Imported owner profile".to_owned(),
                nickname: None,
                avatar: None,
                avatar_crop: None,
                design_description: None,
                design_references: Vec::new(),
                image_recommendation: None,
                active_lorebook_ids: Vec::new(),
                created_at: TimestampMillis::new(1),
                updated_at: TimestampMillis::new(2),
            }],
            default_persona_id: None,
        }
    }

    fn provider_models() -> LegacyProviderModelPlan {
        LegacyProviderModelPlan {
            provider_accounts: Vec::new(),
            model_profiles: Vec::new(),
            default_provider_account_id: None,
            default_model_profile_id: None,
        }
    }

    fn import_plan(
        provider_models: &LegacyProviderModelPlan,
        personas: &LegacyPersonaPlan,
        lorebooks: &LegacyLorebookPlan,
        media: &LegacyMediaPlan,
    ) -> LegacyImportPlan {
        LegacyImportPlan {
            provider_models: provider_models.clone(),
            personas: personas.clone(),
            lorebooks: lorebooks.clone(),
            media: media.clone(),
        }
    }

    fn provider_plan() -> LegacyProviderModelPlan {
        let provider_id = ProviderAccountId::new();
        let model_id = ModelProfileId::new();
        let llama_id = ProviderAccountId::from_str("6c657474-7563-652d-6c6c-616d61637070")
            .expect("built-in llama account id");
        LegacyProviderModelPlan {
            provider_accounts: vec![
                LegacyProviderAccountCandidate {
                    id: provider_id,
                    origin: LegacyProviderAccountOrigin::Stored,
                    secret_owner_id: SecretOwnerId::from_uuid(provider_id.as_uuid()),
                    provider_kind: "openai".to_owned(),
                    protocol: ProviderProtocol::OpenAiCompatible,
                    label: "Primary".to_owned(),
                    endpoint: None,
                    enabled: true,
                    streaming_enabled: true,
                    allow_invalid_tls: false,
                    default_model: None,
                    config: ProviderConfig::Standard,
                    pending_secrets: vec![
                        LegacyPendingProviderSecret::Header {
                            name: HeaderName::new("x-zeta-key").expect("header name"),
                        },
                        LegacyPendingProviderSecret::ApiKey,
                        LegacyPendingProviderSecret::Header {
                            name: HeaderName::new("x-alpha-key").expect("header name"),
                        },
                    ],
                    deferred_config_fields: Vec::new(),
                    created_at: TimestampMillis::new(1),
                    updated_at: TimestampMillis::new(2),
                },
                LegacyProviderAccountCandidate {
                    id: llama_id,
                    origin: LegacyProviderAccountOrigin::BuiltInLlamaCpp,
                    secret_owner_id: SecretOwnerId::from_uuid(llama_id.as_uuid()),
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
                    created_at: TimestampMillis::new(1),
                    updated_at: TimestampMillis::new(2),
                },
            ],
            model_profiles: vec![lettuce_transfer::LegacyModelProfileCandidate {
                id: model_id,
                provider_account_id: provider_id,
                source_provider_kind: "openai".to_owned(),
                source_provider_label: "Primary".to_owned(),
                external_model_id: "gpt-example".to_owned(),
                display_name: "Example Chat".to_owned(),
                kind: lettuce_models::ModelKind::Chat,
                config: ModelProfileConfig {
                    chat_parameters: Default::default(),
                    lorebook_generator_parameters: Default::default(),
                    capabilities: lettuce_models::ModelCapabilities {
                        input_modalities: lettuce_models::ModalityCapabilities {
                            text: lettuce_models::CapabilityStatus::Supported,
                            ..Default::default()
                        },
                        output_modalities: lettuce_models::ModalityCapabilities {
                            text: lettuce_models::CapabilityStatus::Supported,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                },
                prompt_template_id: Some("legacy-prompt".to_owned()),
                deprecated_system_prompt: Some("Retained in the legacy source".to_owned()),
                deferred_advanced_fields: vec!["legacy_sampling_extension".to_owned()],
                created_at: TimestampMillis::new(3),
            }],
            default_provider_account_id: Some(provider_id),
            default_model_profile_id: Some(model_id),
        }
    }

    #[test]
    fn provider_metadata_admission_replays_without_materializing_or_storing_secret_bytes() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-app-legacy-provider-admission-{}.sqlite3",
            LegacyImportRunId::new()
        ));
        let backend = AppBackend::open(&path, TimestampMillis::new(10)).expect("open backend");
        let run_id = LegacyImportRunId::new();
        let providers = provider_plan();
        let inventory = LegacyDatabaseInventory {
            provider_accounts: 1,
            models: 1,
            ..inventory()
        };
        let personas = personas();
        let lorebooks = LegacyLorebookPlan {
            lorebooks: Vec::new(),
        };
        let media = LegacyMediaPlan {
            media: Vec::new(),
            total_bytes: 0,
        };
        let admission = backend
            .legacy_import_admission()
            .admit(
                run_id,
                &inventory,
                &import_plan(&providers, &personas, &lorebooks, &media),
                TimestampMillis::new(20),
            )
            .expect("admit provider metadata");
        assert_eq!(
            admission
                .assignments
                .iter()
                .filter(|assignment| matches!(
                    assignment,
                    LegacyImportAssignment::ProviderSecret { .. }
                ))
                .count(),
            3
        );
        assert_eq!(
            admission
                .assignments
                .iter()
                .filter(|assignment| matches!(
                    assignment,
                    LegacyImportAssignment::ProviderAccount { .. }
                ))
                .count(),
            2
        );
        backend
            .legacy_import_executor()
            .execute(
                &admission,
                &providers,
                &personas,
                &lorebooks,
                &media,
                TimestampMillis::new(30),
            )
            .expect("materialize graph only");
        let replay = backend
            .legacy_import_admission()
            .admit(
                run_id,
                &inventory,
                &import_plan(&providers, &personas, &lorebooks, &media),
                TimestampMillis::new(40),
            )
            .expect("replay provider metadata");
        assert_eq!(replay.status, LegacyImportRunStatus::Importing);
        assert_eq!(replay.assignments, admission.assignments);
        let destination_provider_id = admission
            .assignments
            .iter()
            .find_map(|assignment| match assignment {
                LegacyImportAssignment::ProviderAccount { destination_id, .. } => {
                    Some(*destination_id)
                }
                _ => None,
            })
            .expect("provider assignment");
        assert!(
            ProviderAccountRepository::get(backend.database(), destination_provider_id)
                .expect("read destination provider")
                .is_none()
        );
        let mut changed = providers;
        changed.provider_accounts[0].label = "Changed".to_owned();
        assert_eq!(
            backend.legacy_import_admission().admit(
                run_id,
                &inventory,
                &import_plan(&changed, &personas, &lorebooks, &media),
                TimestampMillis::new(50),
            ),
            Err(LegacyImportRepositoryError::Conflict)
        );
        drop(backend);
        fs::remove_file(path).expect("remove database");
    }

    #[tokio::test]
    async fn provider_models_materialize_atomically_and_replay_after_reopen() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-app-legacy-provider-materialization-{}.sqlite3",
            LegacyImportRunId::new()
        ));
        let secret_store = InMemorySecretStore::new();
        let run_id = LegacyImportRunId::new();
        let provider_models = provider_plan();
        let personas = personas();
        let lorebooks = LegacyLorebookPlan {
            lorebooks: Vec::new(),
        };
        let media = LegacyMediaPlan {
            media: Vec::new(),
            total_bytes: 0,
        };
        let plan = import_plan(&provider_models, &personas, &lorebooks, &media);
        let inventory = LegacyDatabaseInventory {
            provider_accounts: 1,
            models: 1,
            ..inventory()
        };
        let backend = AppBackend::open(&path, TimestampMillis::new(10)).expect("open backend");
        let admission = backend
            .legacy_import_admission()
            .admit(run_id, &inventory, &plan, TimestampMillis::new(20))
            .expect("admit import");
        backend
            .legacy_import_executor()
            .execute(
                &admission,
                &provider_models,
                &personas,
                &lorebooks,
                &media,
                TimestampMillis::new(30),
            )
            .expect("materialize graph");

        let owners = admission
            .assignments
            .iter()
            .filter_map(|assignment| match assignment {
                LegacyImportAssignment::ProviderAccount {
                    legacy_id,
                    secret_owner_id,
                    ..
                } => Some((*legacy_id, *secret_owner_id)),
                _ => None,
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        for assignment in &admission.assignments {
            let LegacyImportAssignment::ProviderSecret {
                source,
                destination_ref,
            } = assignment
            else {
                continue;
            };
            let owner = owners[&source.provider_account_id];
            let purpose = match &source.secret {
                LegacyPendingProviderSecret::ApiKey => SecretPurpose::ProviderApiKey { owner },
                LegacyPendingProviderSecret::Header { name } => {
                    SecretPurpose::ProviderSecretHeader {
                        owner,
                        name: name.clone(),
                    }
                }
            };
            let value = match &source.secret {
                LegacyPendingProviderSecret::ApiKey => "api-canary-value",
                LegacyPendingProviderSecret::Header { name } if name.as_str() == "x-alpha-key" => {
                    "alpha-canary-value"
                }
                LegacyPendingProviderSecret::Header { .. } => "zeta-canary-value",
            };
            let status = secret_store
                .put(
                    SecretRecord::new(*destination_ref, purpose),
                    SecretValue::new(value).expect("valid secret"),
                    None,
                )
                .await
                .expect("store secret");
            backend
                .database()
                .complete_secret(LegacyImportSecretCompletionRequest {
                    run_id,
                    source: source.clone(),
                    destination_ref: *destination_ref,
                    generation: status.generation,
                    completed_at: TimestampMillis::new(40),
                })
                .expect("complete secret");
        }

        let receipt = backend
            .legacy_provider_model_importer(&secret_store)
            .execute(&admission, &plan, TimestampMillis::new(50))
            .await
            .expect("materialize provider models");
        assert_eq!(receipt.provider_account_count, 2);
        assert_eq!(receipt.model_profile_count, 1);
        assert!(!receipt.replayed);
        let destination_provider_id = admission
            .assignments
            .iter()
            .find_map(|assignment| match assignment {
                LegacyImportAssignment::ProviderAccount {
                    legacy_id,
                    destination_id,
                    ..
                } if *legacy_id == provider_models.provider_accounts[0].id => Some(*destination_id),
                _ => None,
            })
            .expect("provider assignment");
        let destination_model_id = admission
            .assignments
            .iter()
            .find_map(|assignment| match assignment {
                LegacyImportAssignment::ModelProfile {
                    legacy_id,
                    destination_id,
                } if *legacy_id == provider_models.model_profiles[0].id => Some(*destination_id),
                _ => None,
            })
            .expect("model assignment");
        let account = ProviderAccountRepository::get(backend.database(), destination_provider_id)
            .expect("read provider")
            .expect("provider exists");
        assert_eq!(
            account.secret_owner_id,
            owners[&provider_models.provider_accounts[0].id]
        );
        assert!(account.api_key_ref.is_some());
        assert_eq!(
            account
                .secret_headers
                .iter()
                .map(|header| header.name.as_str())
                .collect::<Vec<_>>(),
            vec!["x-zeta-key", "x-alpha-key"]
        );
        let profile = ModelProfileRepository::get(backend.database(), destination_model_id)
            .expect("read model")
            .expect("model exists");
        assert_eq!(profile.provider_account_id, destination_provider_id);
        assert_eq!(profile.external_model_id, "gpt-example");
        assert_eq!(
            GlobalSettingsStore::load(backend.database())
                .expect("read settings")
                .default_model_profile_id,
            Some(destination_model_id)
        );
        drop(backend);

        let reopened = AppBackend::open(&path, TimestampMillis::new(60)).expect("reopen backend");
        let replayed_admission = reopened
            .legacy_import_admission()
            .admit(run_id, &inventory, &plan, TimestampMillis::new(70))
            .expect("replay admission");
        assert_eq!(replayed_admission.status, LegacyImportRunStatus::Completed);
        let replay = reopened
            .legacy_provider_model_importer(&secret_store)
            .execute(&replayed_admission, &plan, TimestampMillis::new(80))
            .await
            .expect("replay provider models");
        assert!(replay.replayed);
        drop(reopened);
        let sqlite_bytes = fs::read(&path).expect("read database bytes");
        for canary in [
            "api-canary-value",
            "alpha-canary-value",
            "zeta-canary-value",
        ] {
            assert!(
                !sqlite_bytes
                    .windows(canary.len())
                    .any(|window| window == canary.as_bytes())
            );
        }
        fs::remove_file(path).expect("remove database");
    }

    #[tokio::test]
    async fn provider_materialization_without_secret_receipts_writes_nothing() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-app-legacy-provider-missing-secrets-{}.sqlite3",
            LegacyImportRunId::new()
        ));
        let backend = AppBackend::open(&path, TimestampMillis::new(10)).expect("open backend");
        let provider_models = provider_plan();
        let personas = personas();
        let lorebooks = LegacyLorebookPlan {
            lorebooks: Vec::new(),
        };
        let media = LegacyMediaPlan {
            media: Vec::new(),
            total_bytes: 0,
        };
        let plan = import_plan(&provider_models, &personas, &lorebooks, &media);
        let admission = backend
            .legacy_import_admission()
            .admit(
                LegacyImportRunId::new(),
                &LegacyDatabaseInventory {
                    provider_accounts: 1,
                    models: 1,
                    ..inventory()
                },
                &plan,
                TimestampMillis::new(20),
            )
            .expect("admit import");
        backend
            .legacy_import_executor()
            .execute(
                &admission,
                &provider_models,
                &personas,
                &lorebooks,
                &media,
                TimestampMillis::new(30),
            )
            .expect("materialize graph");
        let empty_store = InMemorySecretStore::new();
        assert_eq!(
            backend
                .legacy_provider_model_importer(&empty_store)
                .execute(&admission, &plan, TimestampMillis::new(40))
                .await,
            Err(crate::LegacyProviderModelImportError::SecretChanged)
        );
        for destination_id in admission
            .assignments
            .iter()
            .filter_map(|assignment| match assignment {
                LegacyImportAssignment::ProviderAccount { destination_id, .. } => {
                    Some(*destination_id)
                }
                _ => None,
            })
        {
            assert!(
                ProviderAccountRepository::get(backend.database(), destination_id)
                    .expect("read provider")
                    .is_none()
            );
        }
        assert_eq!(
            GlobalSettingsStore::load(backend.database())
                .expect("read settings")
                .default_model_profile_id,
            None
        );
        drop(backend);
        fs::remove_file(path).expect("remove database");
    }

    #[tokio::test]
    async fn provider_destination_collision_rolls_back_accounts_models_and_default() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-app-legacy-provider-collision-{}.sqlite3",
            LegacyImportRunId::new()
        ));
        let backend = AppBackend::open(&path, TimestampMillis::new(10)).expect("open backend");
        let mut provider_models = provider_plan();
        for provider in &mut provider_models.provider_accounts {
            provider.pending_secrets.clear();
        }
        let personas = personas();
        let lorebooks = LegacyLorebookPlan {
            lorebooks: Vec::new(),
        };
        let media = LegacyMediaPlan {
            media: Vec::new(),
            total_bytes: 0,
        };
        let plan = import_plan(&provider_models, &personas, &lorebooks, &media);
        let admission = backend
            .legacy_import_admission()
            .admit(
                LegacyImportRunId::new(),
                &LegacyDatabaseInventory {
                    provider_accounts: 1,
                    models: 1,
                    ..inventory()
                },
                &plan,
                TimestampMillis::new(20),
            )
            .expect("admit import");
        backend
            .legacy_import_executor()
            .execute(
                &admission,
                &provider_models,
                &personas,
                &lorebooks,
                &media,
                TimestampMillis::new(30),
            )
            .expect("materialize graph");
        let assignments = admission
            .assignments
            .iter()
            .filter_map(|assignment| match assignment {
                LegacyImportAssignment::ProviderAccount {
                    legacy_id,
                    destination_id,
                    secret_owner_id,
                } => Some((*legacy_id, (*destination_id, *secret_owner_id))),
                _ => None,
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let collision_candidate = &provider_models.provider_accounts[1];
        let (collision_id, collision_owner) = assignments[&collision_candidate.id];
        ProviderAccountRepository::upsert(
            backend.database(),
            ProviderAccount {
                id: collision_id,
                secret_owner_id: collision_owner,
                provider_kind: collision_candidate.provider_kind.clone(),
                protocol: collision_candidate.protocol,
                label: "Existing collision".to_owned(),
                endpoint: collision_candidate.endpoint.clone(),
                enabled: true,
                streaming_enabled: true,
                allow_invalid_tls: false,
                api_key_ref: None,
                secret_headers: Vec::new(),
                config: collision_candidate.config.clone(),
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(35),
                updated_at: TimestampMillis::new(35),
            },
            None,
        )
        .expect("create collision");
        let store = InMemorySecretStore::new();
        assert_eq!(
            backend
                .legacy_provider_model_importer(&store)
                .execute(&admission, &plan, TimestampMillis::new(40))
                .await,
            Err(crate::LegacyProviderModelImportError::Repository(
                LegacyImportRepositoryError::Conflict
            ))
        );
        let first_destination_id = assignments[&provider_models.provider_accounts[0].id].0;
        assert!(
            ProviderAccountRepository::get(backend.database(), first_destination_id)
                .expect("read rolled back provider")
                .is_none()
        );
        let destination_model_id = admission
            .assignments
            .iter()
            .find_map(|assignment| match assignment {
                LegacyImportAssignment::ModelProfile { destination_id, .. } => {
                    Some(*destination_id)
                }
                _ => None,
            })
            .expect("model assignment");
        assert!(
            ModelProfileRepository::get(backend.database(), destination_model_id)
                .expect("read rolled back model")
                .is_none()
        );
        assert_eq!(
            GlobalSettingsStore::load(backend.database())
                .expect("read settings")
                .default_model_profile_id,
            None
        );
        drop(backend);
        fs::remove_file(path).expect("remove database");
    }

    #[tokio::test]
    async fn authored_default_conflict_rolls_back_imported_provider_graph() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-app-legacy-provider-default-conflict-{}.sqlite3",
            LegacyImportRunId::new()
        ));
        let backend = AppBackend::open(&path, TimestampMillis::new(10)).expect("open backend");
        let existing_provider = ProviderAccountRepository::upsert(
            backend.database(),
            ProviderAccount {
                id: ProviderAccountId::new(),
                secret_owner_id: SecretOwnerId::new(),
                provider_kind: "existing".to_owned(),
                protocol: ProviderProtocol::OpenAiCompatible,
                label: "Existing Provider".to_owned(),
                endpoint: None,
                enabled: true,
                streaming_enabled: true,
                allow_invalid_tls: false,
                api_key_ref: None,
                secret_headers: Vec::new(),
                config: ProviderConfig::Standard,
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(11),
                updated_at: TimestampMillis::new(11),
            },
            None,
        )
        .expect("create existing provider");
        let existing_model = ModelProfileRepository::upsert(
            backend.database(),
            ModelProfile {
                id: ModelProfileId::new(),
                provider_account_id: existing_provider.id,
                external_model_id: "existing-model".to_owned(),
                display_name: "Existing Model".to_owned(),
                kind: lettuce_models::ModelKind::Chat,
                config: provider_plan().model_profiles[0].config.clone(),
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(12),
                updated_at: TimestampMillis::new(12),
            },
            None,
        )
        .expect("create existing model");
        let settings = GlobalSettingsStore::load(backend.database()).expect("load settings");
        GlobalSettingsStore::save(
            backend.database(),
            settings.settings,
            Some(existing_model.id),
            settings.revision,
        )
        .expect("select authored default");

        let mut provider_models = provider_plan();
        for provider in &mut provider_models.provider_accounts {
            provider.pending_secrets.clear();
        }
        let personas = personas();
        let lorebooks = LegacyLorebookPlan {
            lorebooks: Vec::new(),
        };
        let media = LegacyMediaPlan {
            media: Vec::new(),
            total_bytes: 0,
        };
        let plan = import_plan(&provider_models, &personas, &lorebooks, &media);
        let admission = backend
            .legacy_import_admission()
            .admit(
                LegacyImportRunId::new(),
                &LegacyDatabaseInventory {
                    provider_accounts: 1,
                    models: 1,
                    ..inventory()
                },
                &plan,
                TimestampMillis::new(20),
            )
            .expect("admit import");
        backend
            .legacy_import_executor()
            .execute(
                &admission,
                &provider_models,
                &personas,
                &lorebooks,
                &media,
                TimestampMillis::new(30),
            )
            .expect("materialize graph");
        let store = InMemorySecretStore::new();
        assert_eq!(
            backend
                .legacy_provider_model_importer(&store)
                .execute(&admission, &plan, TimestampMillis::new(40))
                .await,
            Err(crate::LegacyProviderModelImportError::Repository(
                LegacyImportRepositoryError::Conflict
            ))
        );
        for destination_id in admission
            .assignments
            .iter()
            .filter_map(|assignment| match assignment {
                LegacyImportAssignment::ProviderAccount { destination_id, .. } => {
                    Some(*destination_id)
                }
                _ => None,
            })
        {
            assert!(
                ProviderAccountRepository::get(backend.database(), destination_id)
                    .expect("read rolled back provider")
                    .is_none()
            );
        }
        assert_eq!(
            GlobalSettingsStore::load(backend.database())
                .expect("read settings")
                .default_model_profile_id,
            Some(existing_model.id)
        );
        drop(backend);
        fs::remove_file(path).expect("remove database");
    }

    #[test]
    fn backend_admission_replays_and_rejects_changed_source_content() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-app-legacy-import-{}.sqlite3",
            LegacyImportRunId::new()
        ));
        let run_id = LegacyImportRunId::new();
        let inventory = inventory();
        let personas = personas();
        let lorebooks = LegacyLorebookPlan {
            lorebooks: Vec::new(),
        };
        let media = LegacyMediaPlan {
            media: Vec::new(),
            total_bytes: 0,
        };
        let backend = AppBackend::open(&path, TimestampMillis::new(10)).expect("open backend");
        let first = backend
            .legacy_import_admission()
            .admit(
                run_id,
                &inventory,
                &import_plan(&provider_models(), &personas, &lorebooks, &media),
                TimestampMillis::new(20),
            )
            .expect("admit import");
        let replay = backend
            .legacy_import_admission()
            .admit(
                run_id,
                &inventory,
                &import_plan(&provider_models(), &personas, &lorebooks, &media),
                TimestampMillis::new(30),
            )
            .expect("replay import");
        assert!(replay.replayed);
        assert_eq!(replay.assignments, first.assignments);
        assert_eq!(replay.admitted_at, TimestampMillis::new(20));

        let mut changed = personas;
        changed.personas[0].description = "Changed owner profile".to_owned();
        assert_eq!(
            backend.legacy_import_admission().admit(
                run_id,
                &inventory,
                &import_plan(&provider_models(), &changed, &lorebooks, &media),
                TimestampMillis::new(40),
            ),
            Err(LegacyImportRepositoryError::Conflict)
        );
        drop(backend);
        fs::remove_file(path).expect("remove database");
    }

    #[test]
    fn empty_media_graph_completes_and_collision_rolls_back_all_new_rows() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-app-legacy-graph-rollback-{}.sqlite3",
            LegacyImportRunId::new()
        ));
        let backend = AppBackend::open(&path, TimestampMillis::new(10)).expect("open backend");
        let empty_media = LegacyMediaPlan {
            media: Vec::new(),
            total_bytes: 0,
        };
        let empty_personas = personas();
        let empty_books = LegacyLorebookPlan {
            lorebooks: Vec::new(),
        };
        let empty_run = LegacyImportRunId::new();
        let empty_admission = backend
            .legacy_import_admission()
            .admit(
                empty_run,
                &inventory(),
                &import_plan(
                    &provider_models(),
                    &empty_personas,
                    &empty_books,
                    &empty_media,
                ),
                TimestampMillis::new(20),
            )
            .expect("admit empty media import");
        let empty_receipt = backend
            .legacy_import_executor()
            .execute(
                &empty_admission,
                &provider_models(),
                &empty_personas,
                &empty_books,
                &empty_media,
                TimestampMillis::new(30),
            )
            .expect("complete empty media import");
        assert_eq!(
            (empty_receipt.persona_count, empty_receipt.lorebook_count),
            (1, 0)
        );

        let source_persona_id = PersonaId::new();
        let source_book_id = LorebookId::new();
        let collision_personas = LegacyPersonaPlan {
            personas: vec![LegacyPersonaCandidate {
                id: source_persona_id,
                title: "Collision Source".to_owned(),
                description: "This graph must roll back.".to_owned(),
                nickname: None,
                avatar: None,
                avatar_crop: None,
                design_description: None,
                design_references: Vec::new(),
                image_recommendation: None,
                active_lorebook_ids: vec![source_book_id],
                created_at: TimestampMillis::new(40),
                updated_at: TimestampMillis::new(41),
            }],
            default_persona_id: Some(source_persona_id),
        };
        let collision_books = LegacyLorebookPlan {
            lorebooks: vec![LegacyLorebookCandidate {
                id: source_book_id,
                name: "Rollback Book".to_owned(),
                avatar: None,
                detection_policy: LegacyLorebookDetectionPolicy::RecentMessageWindow,
                entries: Vec::new(),
                created_at: TimestampMillis::new(40),
                updated_at: TimestampMillis::new(41),
            }],
        };
        let collision_inventory = LegacyDatabaseInventory {
            lorebooks: 1,
            ..inventory()
        };
        let collision_run = LegacyImportRunId::new();
        let collision_admission = backend
            .legacy_import_admission()
            .admit(
                collision_run,
                &collision_inventory,
                &import_plan(
                    &provider_models(),
                    &collision_personas,
                    &collision_books,
                    &empty_media,
                ),
                TimestampMillis::new(50),
            )
            .expect("admit collision graph");
        let destination_persona_id = collision_admission
            .assignments
            .iter()
            .find_map(|assignment| match assignment {
                LegacyImportAssignment::Persona {
                    legacy_id,
                    destination_id,
                } if *legacy_id == source_persona_id => Some(*destination_id),
                _ => None,
            })
            .expect("persona assignment");
        let destination_book_id = collision_admission
            .assignments
            .iter()
            .find_map(|assignment| match assignment {
                LegacyImportAssignment::Lorebook {
                    legacy_id,
                    destination_id,
                } if *legacy_id == source_book_id => Some(*destination_id),
                _ => None,
            })
            .expect("lorebook assignment");
        PersonaRepository::create(
            backend.database(),
            Persona::new(
                destination_persona_id,
                "Existing Persona".to_owned(),
                "Preexisting collision row".to_owned(),
                TimestampMillis::new(45),
            )
            .expect("valid collision persona"),
        )
        .expect("create collision");
        assert_eq!(
            backend.legacy_import_executor().execute(
                &collision_admission,
                &provider_models(),
                &collision_personas,
                &collision_books,
                &empty_media,
                TimestampMillis::new(60),
            ),
            Err(LegacyImportRepositoryError::Conflict)
        );
        assert!(
            LorebookRepository::get(backend.database(), destination_book_id)
                .expect("read rolled back lorebook")
                .is_none()
        );
        let replayed_admission = backend
            .legacy_import_admission()
            .admit(
                collision_run,
                &collision_inventory,
                &import_plan(
                    &provider_models(),
                    &collision_personas,
                    &collision_books,
                    &empty_media,
                ),
                TimestampMillis::new(70),
            )
            .expect("read rolled back admission");
        assert_eq!(replayed_admission.status, LegacyImportRunStatus::Admitted);
        let default = PersonaRepository::get_default_snapshot(backend.database())
            .expect("read unchanged default");
        assert_eq!(default.state.persona_id, None);
        drop(backend);
        fs::remove_file(path).expect("remove database");
    }
}
