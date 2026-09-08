use std::{collections::BTreeMap, str::FromStr};

use lettuce_characters::{
    Crop, ImageRecommendation, LifecycleStatus, Persona, PersonaMedia, PersonaMediaLink,
    PersonaMediaSlot,
};
use lettuce_context::{
    DetectionPolicy, KeywordMatchMode, LifecycleStatus as LorebookLifecycleStatus,
    LifecycleStatus as PromptLifecycleStatus, Lorebook, LorebookBehaviorVersion, LorebookBinding,
    LorebookDetails, LorebookEntry, PromptBehaviorVersion, PromptDocument, PromptEntry,
    PromptProvenance,
};
use lettuce_models::{ModelProfile, ProviderAccount, SecretHeader};
use lettuce_settings::{HeaderName, SecretOwnerId, SecretRef};
use lettuce_transfer::{
    LEGACY_DATABASE_SCHEMA_VERSION, LegacyImportAdmission, LegacyImportAdmissionRequest,
    LegacyImportAssignment, LegacyImportExecutionRequest, LegacyImportMediaCompletion,
    LegacyImportMediaCompletionRequest, LegacyImportMediaSource, LegacyImportProviderSecretSource,
    LegacyImportReceipt, LegacyImportRepository, LegacyImportRepositoryError,
    LegacyImportRunStatus, LegacyImportSecretCompletion, LegacyImportSecretCompletionRequest,
    LegacyImportSources, LegacyKeywordMatchMode, LegacyLorebookDetectionPolicy, LegacyMediaUse,
    LegacyPendingProviderSecret, LegacyProviderModelMaterializationRequest,
    LegacyProviderModelReceipt,
};
use lettuce_types::{
    AsrCorrectionId, AsrIgnoredSuggestionId, AsrVocabularyTermId, AsrVoiceExampleId, AssetId,
    ContentHash, LegacyImportRunId, LorebookEntryId, LorebookId, ModelProfileId, PersonaId,
    PromptDocumentId, PromptEntryId, ProviderAccountId, Revision, TimestampMillis,
};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::Database;

const MAX_MEDIA_PATH_BYTES: usize = 1_024;

impl LegacyImportRepository for Database {
    fn admit(
        &self,
        mut request: LegacyImportAdmissionRequest,
    ) -> Result<LegacyImportAdmission, LegacyImportRepositoryError> {
        normalize_sources(&mut request.sources)?;
        if request.source_schema_version != LEGACY_DATABASE_SCHEMA_VERSION {
            return Err(LegacyImportRepositoryError::InvalidInput);
        }
        let mut connection = self
            .connection()
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        if let Some(mut existing) = load_admission(&transaction, request.run_id)? {
            if existing.source_schema_version != request.source_schema_version
                || existing.inventory_fingerprint != request.inventory_fingerprint
                || existing.plan_fingerprint != request.plan_fingerprint
                || assignment_sources(&existing.assignments) != request.sources
            {
                return Err(LegacyImportRepositoryError::Conflict);
            }
            existing.replayed = true;
            transaction
                .commit()
                .map_err(|_| LegacyImportRepositoryError::Storage)?;
            return Ok(existing);
        }

        transaction
            .execute(
                "INSERT INTO legacy_import_runs (id,source_schema_version,inventory_fingerprint,plan_fingerprint,status,admitted_at,updated_at) VALUES (?1,?2,?3,?4,'admitting',?5,?5)",
                params![
                    request.run_id.to_string(),
                    request.source_schema_version,
                    request.inventory_fingerprint.as_str(),
                    request.plan_fingerprint.as_str(),
                    request.admitted_at.get(),
                ],
            )
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        insert_assignments(&transaction, request.run_id, &request.sources)?;
        transaction
            .execute(
                "UPDATE legacy_import_runs SET status='admitted' WHERE id=?1 AND status='admitting'",
                [request.run_id.to_string()],
            )
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        let admission = load_admission(&transaction, request.run_id)?
            .ok_or(LegacyImportRepositoryError::Storage)?;
        transaction
            .commit()
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        Ok(admission)
    }

    fn complete_media(
        &self,
        request: LegacyImportMediaCompletionRequest,
    ) -> Result<LegacyImportMediaCompletion, LegacyImportRepositoryError> {
        if !valid_media_path(&request.relative_path) {
            return Err(LegacyImportRepositoryError::InvalidInput);
        }
        let byte_len = i64::try_from(request.byte_len)
            .map_err(|_| LegacyImportRepositoryError::InvalidInput)?;
        let mut connection = self
            .connection()
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        if let Some(mut existing) =
            load_media_completion(&transaction, request.run_id, &request.relative_path)?
        {
            if existing.destination_asset_id != request.destination_asset_id
                || existing.blob_id != request.blob_id
                || existing.byte_len != request.byte_len
                || existing.content_hash != request.content_hash
            {
                return Err(LegacyImportRepositoryError::Conflict);
            }
            existing.replayed = true;
            transaction
                .commit()
                .map_err(|_| LegacyImportRepositoryError::Storage)?;
            return Ok(existing);
        }
        transaction
            .execute(
                "INSERT INTO legacy_import_media_completions (run_id,relative_path,destination_asset_id,blob_id,byte_len,content_hash,completed_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![
                    request.run_id.to_string(),
                    request.relative_path,
                    request.destination_asset_id.to_string(),
                    request.blob_id.to_string(),
                    byte_len,
                    request.content_hash.as_str(),
                    request.completed_at.get(),
                ],
            )
            .map_err(map_completion_insert_error)?;
        transaction
            .execute(
                "UPDATE legacy_import_runs SET status='importing',updated_at=?2 WHERE id=?1 AND status='admitted'",
                params![request.run_id.to_string(), request.completed_at.get()],
            )
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        let completion =
            load_media_completion(&transaction, request.run_id, &request.relative_path)?
                .ok_or(LegacyImportRepositoryError::Storage)?;
        transaction
            .commit()
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        Ok(completion)
    }

    fn get_secret_completion(
        &self,
        run_id: LegacyImportRunId,
        source: &LegacyImportProviderSecretSource,
    ) -> Result<Option<LegacyImportSecretCompletion>, LegacyImportRepositoryError> {
        let connection = self
            .connection()
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        load_secret_completion(&connection, run_id, source)
    }

    fn complete_secret(
        &self,
        request: LegacyImportSecretCompletionRequest,
    ) -> Result<LegacyImportSecretCompletion, LegacyImportRepositoryError> {
        if request.generation == 0 {
            return Err(LegacyImportRepositoryError::InvalidInput);
        }
        let mut connection = self
            .connection()
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        if let Some(mut existing) =
            load_secret_completion(&transaction, request.run_id, &request.source)?
        {
            if existing.destination_ref != request.destination_ref
                || existing.generation != request.generation
            {
                return Err(LegacyImportRepositoryError::Conflict);
            }
            existing.replayed = true;
            transaction
                .commit()
                .map_err(|_| LegacyImportRepositoryError::Storage)?;
            return Ok(existing);
        }
        let (source_kind, source_detail) = secret_source_parts(&request.source);
        transaction
            .execute(
                "INSERT INTO legacy_import_secret_completions (run_id,source_kind,source_key,source_detail,destination_ref,generation,completed_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![
                    request.run_id.to_string(),
                    source_kind,
                    request.source.provider_account_id.to_string(),
                    source_detail,
                    request.destination_ref.to_string(),
                    i64::try_from(request.generation)
                        .map_err(|_| LegacyImportRepositoryError::InvalidInput)?,
                    request.completed_at.get(),
                ],
            )
            .map_err(|_| LegacyImportRepositoryError::Conflict)?;
        let completion = load_secret_completion(&transaction, request.run_id, &request.source)?
            .ok_or(LegacyImportRepositoryError::Storage)?;
        transaction
            .commit()
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        Ok(completion)
    }

    fn materialize(
        &self,
        request: LegacyImportExecutionRequest,
    ) -> Result<LegacyImportReceipt, LegacyImportRepositoryError> {
        let mut connection = self
            .connection()
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        let admission = load_admission(&transaction, request.run_id)?
            .ok_or(LegacyImportRepositoryError::Conflict)?;
        let mut sources = execution_sources(&request);
        normalize_sources(&mut sources)?;
        if admission.plan_fingerprint != request.plan_fingerprint
            || assignment_sources(&admission.assignments) != sources
        {
            return Err(LegacyImportRepositoryError::Conflict);
        }
        if let Some(mut receipt) = load_receipt(&transaction, request.run_id)? {
            let expected_status =
                if load_provider_model_receipt(&transaction, request.run_id)?.is_some() {
                    LegacyImportRunStatus::Completed
                } else {
                    LegacyImportRunStatus::Importing
                };
            if admission.status != expected_status {
                return Err(LegacyImportRepositoryError::Storage);
            }
            receipt.replayed = true;
            transaction
                .commit()
                .map_err(|_| LegacyImportRepositoryError::Storage)?;
            return Ok(receipt);
        }
        if !matches!(
            admission.status,
            LegacyImportRunStatus::Admitted | LegacyImportRunStatus::Importing
        ) {
            return Err(LegacyImportRepositoryError::Conflict);
        }
        let assignments = AssignmentMaps::from_admission(&admission)?;
        let media_by_use = completed_media_by_use(&transaction, &request, &assignments)?;
        transaction
            .execute(
                "UPDATE legacy_import_runs SET status='importing',updated_at=?2 WHERE id=?1 AND status='admitted'",
                params![request.run_id.to_string(), request.completed_at.get()],
            )
            .map_err(|_| LegacyImportRepositoryError::Storage)?;

        for candidate in &request.lorebooks.lorebooks {
            let destination_id = *assignments
                .lorebooks
                .get(&candidate.id)
                .ok_or(LegacyImportRepositoryError::Conflict)?;
            let icon_asset_id = candidate
                .avatar
                .as_ref()
                .map(|_| LegacyMediaUse::LorebookAvatar {
                    lorebook_id: candidate.id,
                })
                .map(|media_use| {
                    media_by_use
                        .get(&media_use)
                        .copied()
                        .ok_or(LegacyImportRepositoryError::Conflict)
                })
                .transpose()?;
            let entries = candidate
                .entries
                .iter()
                .enumerate()
                .map(|(ordinal, entry)| {
                    Ok(LorebookEntry {
                        id: *assignments
                            .entries
                            .get(&entry.id)
                            .ok_or(LegacyImportRepositoryError::Conflict)?,
                        lorebook_id: destination_id,
                        title: entry.title.clone(),
                        enabled: entry.enabled,
                        always_active: entry.always_active,
                        keywords: entry.keywords.clone(),
                        case_sensitive: entry.case_sensitive,
                        match_mode: match entry.match_mode {
                            LegacyKeywordMatchMode::Literal => KeywordMatchMode::Literal,
                            LegacyKeywordMatchMode::Regex => KeywordMatchMode::Regex,
                        },
                        content: entry.content.clone(),
                        priority: entry.priority,
                        ordinal: u32::try_from(ordinal)
                            .map_err(|_| LegacyImportRepositoryError::InvalidInput)?,
                        revision: Revision::INITIAL,
                        created_at: entry.created_at,
                        updated_at: entry.updated_at,
                    })
                })
                .collect::<Result<Vec<_>, LegacyImportRepositoryError>>()?;
            let details = LorebookDetails {
                book: Lorebook {
                    id: destination_id,
                    status: LorebookLifecycleStatus::Active,
                    name: candidate.name.clone(),
                    detection_policy: match candidate.detection_policy {
                        LegacyLorebookDetectionPolicy::RecentMessageWindow => {
                            DetectionPolicy::RecentMessageWindow
                        }
                        LegacyLorebookDetectionPolicy::LatestUserMessage => {
                            DetectionPolicy::LatestUserMessage
                        }
                    },
                    icon_asset_id,
                    behavior_version: LorebookBehaviorVersion::LegacyV1,
                    revision: Revision::INITIAL,
                    created_at: candidate.created_at,
                    updated_at: candidate.updated_at,
                },
                entries,
            };
            crate::lorebook_adapter::insert_lorebook_details(&transaction, &details)
                .map_err(|_| LegacyImportRepositoryError::Conflict)?;
        }

        for candidate in &request.personas.personas {
            let destination_id = *assignments
                .personas
                .get(&candidate.id)
                .ok_or(LegacyImportRepositoryError::Conflict)?;
            let mut links = Vec::new();
            if candidate.avatar.is_some() {
                links.push(PersonaMediaLink {
                    asset_id: *media_by_use
                        .get(&LegacyMediaUse::PersonaAvatar {
                            persona_id: candidate.id,
                        })
                        .ok_or(LegacyImportRepositoryError::Conflict)?,
                    slot: PersonaMediaSlot::Avatar,
                    ordinal: 0,
                });
            }
            for (ordinal, _) in candidate.design_references.iter().enumerate() {
                let ordinal = u32::try_from(ordinal)
                    .map_err(|_| LegacyImportRepositoryError::InvalidInput)?;
                links.push(PersonaMediaLink {
                    asset_id: *media_by_use
                        .get(&LegacyMediaUse::PersonaDesignReference {
                            persona_id: candidate.id,
                            ordinal,
                        })
                        .ok_or(LegacyImportRepositoryError::Conflict)?,
                    slot: PersonaMediaSlot::DesignReference,
                    ordinal,
                });
            }
            let persona = Persona {
                id: destination_id,
                status: LifecycleStatus::Active,
                title: candidate.title.clone(),
                description: candidate.description.clone(),
                nickname: candidate.nickname.clone(),
                design_description: candidate.design_description.clone(),
                avatar_crop: candidate
                    .avatar_crop
                    .map(|crop| Crop::new(crop.x as f32, crop.y as f32, crop.scale as f32))
                    .transpose()
                    .map_err(|_| LegacyImportRepositoryError::InvalidInput)?,
                image_recommendation: candidate.image_recommendation.as_ref().map(|value| {
                    ImageRecommendation {
                        artifact_id: None,
                        unresolved_legacy_name: Some(value.model_name.clone()),
                        strength: value.strength as f32,
                    }
                }),
                media: PersonaMedia { links },
                revision: Revision::INITIAL,
                created_at: candidate.created_at,
                updated_at: candidate.updated_at,
            };
            crate::persona_adapter::insert_persona(&transaction, persona)
                .map_err(|_| LegacyImportRepositoryError::Conflict)?;
            insert_persona_bindings(
                &transaction,
                destination_id,
                candidate,
                &assignments.lorebooks,
            )?;
        }

        if let Some(legacy_default_id) = request.personas.default_persona_id {
            let destination_id = assignments
                .personas
                .get(&legacy_default_id)
                .ok_or(LegacyImportRepositoryError::Conflict)?;
            let changed = transaction
                .execute(
                    "UPDATE persona_defaults SET default_persona_id=?1,revision=2,updated_at=?2 WHERE id=1 AND revision=1 AND default_persona_id IS NULL",
                    params![destination_id.to_string(), request.completed_at.get()],
                )
                .map_err(|_| LegacyImportRepositoryError::Storage)?;
            if changed != 1 {
                return Err(LegacyImportRepositoryError::Conflict);
            }
        }

        let persona_count = i64::try_from(request.personas.personas.len())
            .map_err(|_| LegacyImportRepositoryError::InvalidInput)?;
        let lorebook_count = i64::try_from(request.lorebooks.lorebooks.len())
            .map_err(|_| LegacyImportRepositoryError::InvalidInput)?;
        let lorebook_entry_count = request
            .lorebooks
            .lorebooks
            .iter()
            .try_fold(0_i64, |total, book| {
                i64::try_from(book.entries.len())
                    .ok()
                    .and_then(|count| total.checked_add(count))
            })
            .ok_or(LegacyImportRepositoryError::InvalidInput)?;
        transaction
            .execute(
                "INSERT INTO legacy_import_results (run_id,plan_fingerprint,persona_count,lorebook_count,lorebook_entry_count,completed_at) VALUES (?1,?2,?3,?4,?5,?6)",
                params![request.run_id.to_string(), request.plan_fingerprint.as_str(), persona_count, lorebook_count, lorebook_entry_count, request.completed_at.get()],
            )
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        let receipt = load_receipt(&transaction, request.run_id)?
            .ok_or(LegacyImportRepositoryError::Storage)?;
        transaction
            .commit()
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        Ok(receipt)
    }

    fn materialize_provider_models(
        &self,
        request: LegacyProviderModelMaterializationRequest,
    ) -> Result<LegacyProviderModelReceipt, LegacyImportRepositoryError> {
        let mut connection = self
            .connection()
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        let admission = load_admission(&transaction, request.run_id)?
            .ok_or(LegacyImportRepositoryError::Conflict)?;
        if admission.plan_fingerprint != request.plan_fingerprint {
            return Err(LegacyImportRepositoryError::Conflict);
        }
        let assignments = AssignmentMaps::from_admission(&admission)?;
        validate_provider_model_sources(&request, &assignments)?;
        if load_receipt(&transaction, request.run_id)?.is_none() {
            return Err(LegacyImportRepositoryError::Conflict);
        }
        if let Some(mut receipt) = load_provider_model_receipt(&transaction, request.run_id)? {
            if admission.status != LegacyImportRunStatus::Completed {
                return Err(LegacyImportRepositoryError::Storage);
            }
            receipt.replayed = true;
            transaction
                .commit()
                .map_err(|_| LegacyImportRepositoryError::Storage)?;
            return Ok(receipt);
        }
        if admission.status != LegacyImportRunStatus::Importing {
            return Err(LegacyImportRepositoryError::Conflict);
        }

        for candidate in &request.provider_models.provider_accounts {
            let (destination_id, secret_owner_id) = assignments
                .providers
                .get(&candidate.id)
                .copied()
                .ok_or(LegacyImportRepositoryError::Conflict)?;
            let mut api_key_ref = None;
            let mut secret_headers = Vec::new();
            for secret in &candidate.pending_secrets {
                let source = LegacyImportProviderSecretSource {
                    provider_account_id: candidate.id,
                    secret: secret.clone(),
                };
                let destination_ref = *assignments
                    .secrets
                    .get(&source)
                    .ok_or(LegacyImportRepositoryError::Conflict)?;
                let completion = load_secret_completion(&transaction, request.run_id, &source)?
                    .ok_or(LegacyImportRepositoryError::Conflict)?;
                if completion.destination_ref != destination_ref || completion.generation == 0 {
                    return Err(LegacyImportRepositoryError::Conflict);
                }
                match secret {
                    LegacyPendingProviderSecret::ApiKey => {
                        if api_key_ref.replace(destination_ref).is_some() {
                            return Err(LegacyImportRepositoryError::InvalidInput);
                        }
                    }
                    LegacyPendingProviderSecret::Header { name } => {
                        secret_headers.push(SecretHeader {
                            name: name.clone(),
                            secret_ref: destination_ref,
                        });
                    }
                }
            }
            let account = ProviderAccount {
                id: destination_id,
                secret_owner_id,
                provider_kind: candidate.provider_kind.clone(),
                protocol: candidate.protocol,
                label: candidate.label.clone(),
                endpoint: candidate.endpoint.clone(),
                enabled: candidate.enabled,
                streaming_enabled: candidate.streaming_enabled,
                allow_invalid_tls: candidate.allow_invalid_tls,
                api_key_ref,
                secret_headers,
                config: candidate.config.clone(),
                revision: Revision::INITIAL,
                created_at: candidate.created_at,
                updated_at: candidate.updated_at,
            };
            crate::validate_account(&account)
                .map_err(|_| LegacyImportRepositoryError::InvalidInput)?;
            let headers = serde_json::to_string(&account.secret_headers)
                .map_err(|_| LegacyImportRepositoryError::InvalidInput)?;
            let config =
                crate::encode_versioned(&account.config, crate::PROVIDER_CONFIG_FORMAT_VERSION)
                    .map_err(|_| LegacyImportRepositoryError::InvalidInput)?;
            transaction
                .execute(
                    "INSERT INTO provider_accounts (id,provider_kind,protocol,label,endpoint,enabled,streaming_enabled,allow_invalid_tls,api_key_secret_ref,secret_owner_id,secret_headers_json,config_json,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,1,?13,?14)",
                    params![account.id.to_string(), account.provider_kind, crate::provider_protocol_name(account.protocol), account.label, account.endpoint, account.enabled, account.streaming_enabled, account.allow_invalid_tls, account.api_key_ref.map(|value| value.to_string()), account.secret_owner_id.as_uuid().to_string(), headers, config, account.created_at.get(), account.updated_at.get()],
                )
                .map_err(|_| LegacyImportRepositoryError::Conflict)?;
        }

        for candidate in &request.provider_models.model_profiles {
            let destination_id = *assignments
                .models
                .get(&candidate.id)
                .ok_or(LegacyImportRepositoryError::Conflict)?;
            let provider_account_id = assignments
                .providers
                .get(&candidate.provider_account_id)
                .map(|(destination_id, _)| *destination_id)
                .ok_or(LegacyImportRepositoryError::Conflict)?;
            let profile = ModelProfile {
                id: destination_id,
                provider_account_id,
                external_model_id: candidate.external_model_id.clone(),
                display_name: candidate.display_name.clone(),
                kind: candidate.kind,
                config: candidate.config.clone(),
                revision: Revision::INITIAL,
                created_at: candidate.created_at,
                updated_at: candidate.created_at,
            };
            crate::validate_profile(&profile)
                .map_err(|_| LegacyImportRepositoryError::InvalidInput)?;
            let config = crate::encode_versioned(
                &profile.config,
                crate::MODEL_PROFILE_CONFIG_FORMAT_VERSION,
            )
            .map_err(|_| LegacyImportRepositoryError::InvalidInput)?;
            transaction
                .execute(
                    "INSERT INTO model_profiles (id,provider_account_id,external_model_id,display_name,kind,config_json,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,1,?7,?8)",
                    params![profile.id.to_string(), profile.provider_account_id.to_string(), profile.external_model_id, profile.display_name, crate::model_kind_name(profile.kind), config, profile.created_at.get(), profile.updated_at.get()],
                )
                .map_err(|_| LegacyImportRepositoryError::Conflict)?;
        }

        for candidate in &request.prompts.prompts {
            let destination_id = *assignments
                .prompts
                .get(&candidate.source_id)
                .ok_or(LegacyImportRepositoryError::Conflict)?;
            let entries = candidate
                .entries
                .iter()
                .enumerate()
                .map(|(ordinal, entry)| {
                    let identity = format!("{}:{ordinal}", entry.source_id);
                    PromptEntry {
                        id: PromptEntryId::from_uuid(uuid::Uuid::new_v5(
                            &destination_id.as_uuid(),
                            identity.as_bytes(),
                        )),
                        built_in_entry_key: None,
                        name: entry.draft.name.clone(),
                        role: entry.draft.role,
                        content: entry.draft.content.clone(),
                        enabled: entry.draft.enabled,
                        injection_position: entry.draft.injection_position,
                        depth: entry.draft.depth,
                        conditional_min_messages: entry.draft.conditional_min_messages,
                        interval_turns: entry.draft.interval_turns,
                        system_prompt: entry.draft.system_prompt,
                        conditions: entry.draft.conditions.clone(),
                        payload: entry.draft.payload.clone(),
                    }
                })
                .collect();
            let document = PromptDocument {
                id: destination_id,
                status: PromptLifecycleStatus::Active,
                name: candidate.name.clone(),
                purpose: candidate.purpose,
                entries,
                condense: candidate.condense,
                behavior_version: PromptBehaviorVersion::LegacyV1,
                provenance: PromptProvenance::Imported,
                revision: Revision::INITIAL,
                created_at: candidate.created_at,
                updated_at: candidate.updated_at,
            };
            crate::prompt_adapter::insert_imported_document(&transaction, &document)
                .map_err(|_| LegacyImportRepositoryError::Conflict)?;
        }

        if let Some(legacy_default_id) = request.provider_models.default_model_profile_id {
            let destination_id = assignments
                .models
                .get(&legacy_default_id)
                .ok_or(LegacyImportRepositoryError::Conflict)?;
            let changed = transaction
                .execute(
                    "UPDATE app_settings SET default_model_profile_id=?1,revision=2,updated_at=?2 WHERE id=1 AND revision=1 AND default_model_profile_id IS NULL",
                    params![destination_id.to_string(), request.completed_at.get()],
                )
                .map_err(|_| LegacyImportRepositoryError::Storage)?;
            if changed != 1 {
                return Err(LegacyImportRepositoryError::Conflict);
            }
        }

        if let Some(destination_id) = request
            .prompts
            .default_prompt_source_id
            .as_ref()
            .and_then(|source_id| assignments.prompts.get(source_id))
        {
            let changed = transaction
                .execute(
                    "UPDATE app_settings SET default_prompt_document_id=?1,revision=revision+1,updated_at=?2 WHERE id=1 AND default_prompt_document_id IS NULL",
                    params![destination_id.to_string(), request.completed_at.get()],
                )
                .map_err(|_| LegacyImportRepositoryError::Storage)?;
            if changed != 1 {
                return Err(LegacyImportRepositoryError::Conflict);
            }
        }

        let provider_account_count = i64::try_from(request.provider_models.provider_accounts.len())
            .map_err(|_| LegacyImportRepositoryError::InvalidInput)?;
        let model_profile_count = i64::try_from(request.provider_models.model_profiles.len())
            .map_err(|_| LegacyImportRepositoryError::InvalidInput)?;
        let prompt_count = i64::try_from(request.prompts.prompts.len())
            .map_err(|_| LegacyImportRepositoryError::InvalidInput)?;
        transaction
            .execute(
                "INSERT INTO legacy_import_provider_model_results (run_id,plan_fingerprint,provider_account_count,model_profile_count,prompt_count,completed_at) VALUES (?1,?2,?3,?4,?5,?6)",
                params![request.run_id.to_string(), request.plan_fingerprint.as_str(), provider_account_count, model_profile_count, prompt_count, request.completed_at.get()],
            )
            .map_err(|_| LegacyImportRepositoryError::Conflict)?;
        let changed = transaction
            .execute(
                "UPDATE legacy_import_runs SET status='completed',updated_at=?2 WHERE id=?1 AND status='importing'",
                params![request.run_id.to_string(), request.completed_at.get()],
            )
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        if changed != 1 {
            return Err(LegacyImportRepositoryError::Conflict);
        }
        let receipt = load_provider_model_receipt(&transaction, request.run_id)?
            .ok_or(LegacyImportRepositoryError::Storage)?;
        transaction
            .commit()
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        Ok(receipt)
    }
}

fn secret_source_parts(source: &LegacyImportProviderSecretSource) -> (&str, &str) {
    match &source.secret {
        LegacyPendingProviderSecret::ApiKey => ("provider_api_key", ""),
        LegacyPendingProviderSecret::Header { name } => ("provider_secret_header", name.as_str()),
    }
}

fn load_secret_completion(
    connection: &rusqlite::Connection,
    run_id: LegacyImportRunId,
    source: &LegacyImportProviderSecretSource,
) -> Result<Option<LegacyImportSecretCompletion>, LegacyImportRepositoryError> {
    let (source_kind, source_detail) = secret_source_parts(source);
    connection
        .query_row(
            "SELECT destination_ref,generation,completed_at FROM legacy_import_secret_completions WHERE run_id=?1 AND source_kind=?2 AND source_key=?3 AND source_detail=?4",
            params![run_id.to_string(), source_kind, source.provider_account_id.to_string(), source_detail],
            |row| {
                let reference = uuid::Uuid::parse_str(&row.get::<_, String>(0)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                let generation = u64::try_from(row.get::<_, i64>(1)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                Ok(LegacyImportSecretCompletion {
                    run_id,
                    source: source.clone(),
                    destination_ref: SecretRef::from_uuid(reference),
                    generation,
                    completed_at: TimestampMillis::new(row.get(2)?),
                    replayed: false,
                })
            },
        )
        .optional()
        .map_err(|_| LegacyImportRepositoryError::Storage)
}

fn map_completion_insert_error(error: rusqlite::Error) -> LegacyImportRepositoryError {
    match &error {
        rusqlite::Error::SqliteFailure(_, Some(message))
            if message.contains("legacy import media completion is invalid") =>
        {
            LegacyImportRepositoryError::Conflict
        }
        _ => LegacyImportRepositoryError::Storage,
    }
}

struct AssignmentMaps {
    providers: BTreeMap<ProviderAccountId, (ProviderAccountId, SecretOwnerId)>,
    models: BTreeMap<ModelProfileId, ModelProfileId>,
    prompts: BTreeMap<String, PromptDocumentId>,
    secrets: BTreeMap<LegacyImportProviderSecretSource, SecretRef>,
    personas: BTreeMap<PersonaId, PersonaId>,
    lorebooks: BTreeMap<LorebookId, LorebookId>,
    entries: BTreeMap<LorebookEntryId, LorebookEntryId>,
    asr_vocabulary: BTreeMap<i64, AsrVocabularyTermId>,
    asr_corrections: BTreeMap<i64, AsrCorrectionId>,
    asr_ignored: BTreeMap<i64, AsrIgnoredSuggestionId>,
    asr_voice_examples: BTreeMap<i64, AsrVoiceExampleId>,
    media: BTreeMap<String, (AssetId, u64, ContentHash)>,
}

impl AssignmentMaps {
    fn from_admission(
        admission: &LegacyImportAdmission,
    ) -> Result<Self, LegacyImportRepositoryError> {
        let mut maps = Self {
            providers: BTreeMap::new(),
            models: BTreeMap::new(),
            prompts: BTreeMap::new(),
            secrets: BTreeMap::new(),
            personas: BTreeMap::new(),
            lorebooks: BTreeMap::new(),
            entries: BTreeMap::new(),
            asr_vocabulary: BTreeMap::new(),
            asr_corrections: BTreeMap::new(),
            asr_ignored: BTreeMap::new(),
            asr_voice_examples: BTreeMap::new(),
            media: BTreeMap::new(),
        };
        for assignment in &admission.assignments {
            let duplicate = match assignment {
                LegacyImportAssignment::ProviderAccount {
                    legacy_id,
                    destination_id,
                    secret_owner_id,
                } => maps
                    .providers
                    .insert(*legacy_id, (*destination_id, *secret_owner_id))
                    .is_some(),
                LegacyImportAssignment::ModelProfile {
                    legacy_id,
                    destination_id,
                } => maps.models.insert(*legacy_id, *destination_id).is_some(),
                LegacyImportAssignment::Prompt {
                    legacy_id,
                    destination_id,
                } => maps
                    .prompts
                    .insert(legacy_id.clone(), *destination_id)
                    .is_some(),
                LegacyImportAssignment::ProviderSecret {
                    source,
                    destination_ref,
                } => maps
                    .secrets
                    .insert(source.clone(), *destination_ref)
                    .is_some(),
                LegacyImportAssignment::Persona {
                    legacy_id,
                    destination_id,
                } => maps.personas.insert(*legacy_id, *destination_id).is_some(),
                LegacyImportAssignment::Lorebook {
                    legacy_id,
                    destination_id,
                } => maps.lorebooks.insert(*legacy_id, *destination_id).is_some(),
                LegacyImportAssignment::LorebookEntry {
                    legacy_id,
                    destination_id,
                } => maps.entries.insert(*legacy_id, *destination_id).is_some(),
                LegacyImportAssignment::AsrVocabulary {
                    legacy_id,
                    destination_id,
                } => maps
                    .asr_vocabulary
                    .insert(*legacy_id, *destination_id)
                    .is_some(),
                LegacyImportAssignment::AsrCorrection {
                    legacy_id,
                    destination_id,
                } => maps
                    .asr_corrections
                    .insert(*legacy_id, *destination_id)
                    .is_some(),
                LegacyImportAssignment::AsrIgnoredSuggestion {
                    legacy_id,
                    destination_id,
                } => maps
                    .asr_ignored
                    .insert(*legacy_id, *destination_id)
                    .is_some(),
                LegacyImportAssignment::AsrVoiceExample {
                    legacy_id,
                    destination_id,
                } => maps
                    .asr_voice_examples
                    .insert(*legacy_id, *destination_id)
                    .is_some(),
                LegacyImportAssignment::Media {
                    relative_path,
                    destination_id,
                    byte_len,
                    content_hash,
                } => maps
                    .media
                    .insert(
                        relative_path.clone(),
                        (*destination_id, *byte_len, content_hash.clone()),
                    )
                    .is_some(),
            };
            if duplicate {
                return Err(LegacyImportRepositoryError::Storage);
            }
        }
        Ok(maps)
    }
}

fn execution_sources(request: &LegacyImportExecutionRequest) -> LegacyImportSources {
    LegacyImportSources {
        provider_account_ids: request
            .provider_models
            .provider_accounts
            .iter()
            .map(|provider| provider.id)
            .collect(),
        model_profile_ids: request
            .provider_models
            .model_profiles
            .iter()
            .map(|model| model.id)
            .collect(),
        prompt_ids: request
            .prompts
            .prompts
            .iter()
            .map(|prompt| prompt.source_id.clone())
            .collect(),
        provider_secrets: request
            .provider_models
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
        persona_ids: request
            .personas
            .personas
            .iter()
            .map(|persona| persona.id)
            .collect(),
        lorebook_ids: request
            .lorebooks
            .lorebooks
            .iter()
            .map(|book| book.id)
            .collect(),
        lorebook_entry_ids: request
            .lorebooks
            .lorebooks
            .iter()
            .flat_map(|book| book.entries.iter().map(|entry| entry.id))
            .collect(),
        asr_vocabulary_ids: Vec::new(),
        asr_correction_ids: Vec::new(),
        asr_ignored_suggestion_ids: Vec::new(),
        asr_voice_example_ids: Vec::new(),
        media: request
            .media
            .media
            .iter()
            .map(|candidate| LegacyImportMediaSource {
                relative_path: candidate.relative_path.clone(),
                byte_len: candidate.byte_len,
                content_hash: candidate.content_hash.clone(),
            })
            .collect(),
    }
}

fn completed_media_by_use(
    transaction: &Transaction<'_>,
    request: &LegacyImportExecutionRequest,
    assignments: &AssignmentMaps,
) -> Result<BTreeMap<LegacyMediaUse, AssetId>, LegacyImportRepositoryError> {
    if assignments.media.len() != request.media.media.len() {
        return Err(LegacyImportRepositoryError::Conflict);
    }
    let mut by_use = BTreeMap::new();
    for candidate in &request.media.media {
        let (destination_id, byte_len, content_hash) = assignments
            .media
            .get(&candidate.relative_path)
            .ok_or(LegacyImportRepositoryError::Conflict)?;
        if *byte_len != candidate.byte_len || *content_hash != candidate.content_hash {
            return Err(LegacyImportRepositoryError::Conflict);
        }
        let completion =
            load_media_completion(transaction, request.run_id, &candidate.relative_path)?
                .ok_or(LegacyImportRepositoryError::Conflict)?;
        if completion.destination_asset_id != *destination_id
            || completion.byte_len != candidate.byte_len
            || completion.content_hash != candidate.content_hash
        {
            return Err(LegacyImportRepositoryError::Conflict);
        }
        for media_use in &candidate.uses {
            if by_use.insert(media_use.clone(), *destination_id).is_some() {
                return Err(LegacyImportRepositoryError::Conflict);
            }
        }
    }
    if by_use.keys().cloned().collect::<Vec<_>>() != expected_media_uses(request)? {
        return Err(LegacyImportRepositoryError::Conflict);
    }
    Ok(by_use)
}

fn expected_media_uses(
    request: &LegacyImportExecutionRequest,
) -> Result<Vec<LegacyMediaUse>, LegacyImportRepositoryError> {
    let mut uses = Vec::new();
    for persona in &request.personas.personas {
        if persona.avatar.is_some() {
            uses.push(LegacyMediaUse::PersonaAvatar {
                persona_id: persona.id,
            });
        }
        for ordinal in 0..persona.design_references.len() {
            uses.push(LegacyMediaUse::PersonaDesignReference {
                persona_id: persona.id,
                ordinal: u32::try_from(ordinal)
                    .map_err(|_| LegacyImportRepositoryError::InvalidInput)?,
            });
        }
    }
    for lorebook in &request.lorebooks.lorebooks {
        if lorebook.avatar.is_some() {
            uses.push(LegacyMediaUse::LorebookAvatar {
                lorebook_id: lorebook.id,
            });
        }
    }
    uses.sort();
    Ok(uses)
}

fn insert_persona_bindings(
    transaction: &Transaction<'_>,
    destination_persona_id: PersonaId,
    candidate: &lettuce_transfer::LegacyPersonaCandidate,
    lorebook_assignments: &BTreeMap<LorebookId, LorebookId>,
) -> Result<(), LegacyImportRepositoryError> {
    let bindings = candidate
        .active_lorebook_ids
        .iter()
        .enumerate()
        .map(|(ordinal, legacy_lorebook_id)| {
            Ok(LorebookBinding {
                lorebook_id: *lorebook_assignments
                    .get(legacy_lorebook_id)
                    .ok_or(LegacyImportRepositoryError::Conflict)?,
                enabled: true,
                ordinal: u32::try_from(ordinal)
                    .map_err(|_| LegacyImportRepositoryError::InvalidInput)?,
                revision: Revision::INITIAL,
                created_at: candidate.created_at,
                updated_at: candidate.updated_at,
            })
        })
        .collect::<Result<Vec<_>, LegacyImportRepositoryError>>()?;
    lettuce_context::validate_bindings(&bindings)
        .map_err(|_| LegacyImportRepositoryError::InvalidInput)?;
    for binding in bindings {
        transaction
            .execute(
                "INSERT INTO persona_lorebook_bindings (persona_id,lorebook_id,enabled,ordinal,revision,created_at,updated_at) VALUES (?1,?2,1,?3,1,?4,?5)",
                params![destination_persona_id.to_string(), binding.lorebook_id.to_string(), i64::from(binding.ordinal), binding.created_at.get(), binding.updated_at.get()],
            )
            .map_err(|_| LegacyImportRepositoryError::Conflict)?;
    }
    Ok(())
}

fn load_receipt(
    transaction: &Transaction<'_>,
    run_id: LegacyImportRunId,
) -> Result<Option<LegacyImportReceipt>, LegacyImportRepositoryError> {
    transaction
        .query_row(
            "SELECT persona_count,lorebook_count,lorebook_entry_count,completed_at FROM legacy_import_results WHERE run_id=?1",
            [run_id.to_string()],
            |row| {
                Ok(LegacyImportReceipt {
                    run_id,
                    persona_count: u64::try_from(row.get::<_, i64>(0)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    lorebook_count: u64::try_from(row.get::<_, i64>(1)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    lorebook_entry_count: u64::try_from(row.get::<_, i64>(2)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    completed_at: TimestampMillis::new(row.get(3)?),
                    replayed: false,
                })
            },
        )
        .optional()
        .map_err(|_| LegacyImportRepositoryError::Storage)
}

fn load_provider_model_receipt(
    transaction: &Transaction<'_>,
    run_id: LegacyImportRunId,
) -> Result<Option<LegacyProviderModelReceipt>, LegacyImportRepositoryError> {
    transaction
        .query_row(
            "SELECT provider_account_count,model_profile_count,prompt_count,completed_at FROM legacy_import_provider_model_results WHERE run_id=?1",
            [run_id.to_string()],
            |row| {
                Ok(LegacyProviderModelReceipt {
                    run_id,
                    provider_account_count: u64::try_from(row.get::<_, i64>(0)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    model_profile_count: u64::try_from(row.get::<_, i64>(1)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    prompt_count: u64::try_from(row.get::<_, i64>(2)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    completed_at: TimestampMillis::new(row.get(3)?),
                    replayed: false,
                })
            },
        )
        .optional()
        .map_err(|_| LegacyImportRepositoryError::Storage)
}

fn validate_provider_model_sources(
    request: &LegacyProviderModelMaterializationRequest,
    assignments: &AssignmentMaps,
) -> Result<(), LegacyImportRepositoryError> {
    let provider_ids = request
        .provider_models
        .provider_accounts
        .iter()
        .map(|provider| provider.id)
        .collect::<Vec<_>>();
    let model_ids = request
        .provider_models
        .model_profiles
        .iter()
        .map(|model| model.id)
        .collect::<Vec<_>>();
    let secret_sources = request
        .provider_models
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
        .collect::<Vec<_>>();
    let prompt_ids = request
        .prompts
        .prompts
        .iter()
        .map(|prompt| prompt.source_id.clone())
        .collect::<Vec<_>>();
    let mut expected_provider_ids = assignments.providers.keys().copied().collect::<Vec<_>>();
    let mut expected_model_ids = assignments.models.keys().copied().collect::<Vec<_>>();
    let expected_secret_sources = assignments.secrets.keys().cloned().collect::<Vec<_>>();
    let expected_prompt_ids = assignments.prompts.keys().cloned().collect::<Vec<_>>();
    let mut provider_ids = provider_ids;
    let mut model_ids = model_ids;
    let mut secret_sources = secret_sources;
    let mut prompt_ids = prompt_ids;
    provider_ids.sort_unstable();
    model_ids.sort_unstable();
    secret_sources.sort();
    prompt_ids.sort();
    expected_provider_ids.sort_unstable();
    expected_model_ids.sort_unstable();
    if provider_ids != expected_provider_ids
        || model_ids != expected_model_ids
        || secret_sources != expected_secret_sources
        || prompt_ids != expected_prompt_ids
        || has_duplicates(&provider_ids)
        || has_duplicates(&model_ids)
        || has_duplicates(&secret_sources)
        || has_duplicates(&prompt_ids)
        || request.provider_models.model_profiles.iter().any(|model| {
            !assignments
                .providers
                .contains_key(&model.provider_account_id)
        })
        || request
            .provider_models
            .default_provider_account_id
            .is_some_and(|id| !assignments.providers.contains_key(&id))
        || request
            .provider_models
            .default_model_profile_id
            .is_some_and(|id| !assignments.models.contains_key(&id))
    {
        return Err(LegacyImportRepositoryError::Conflict);
    }
    Ok(())
}

fn normalize_sources(sources: &mut LegacyImportSources) -> Result<(), LegacyImportRepositoryError> {
    sources.provider_account_ids.sort_unstable();
    sources.model_profile_ids.sort_unstable();
    sources.prompt_ids.sort();
    sources.provider_secrets.sort();
    sources.persona_ids.sort_unstable();
    sources.lorebook_ids.sort_unstable();
    sources.lorebook_entry_ids.sort_unstable();
    sources.asr_vocabulary_ids.sort_unstable();
    sources.asr_correction_ids.sort_unstable();
    sources.asr_ignored_suggestion_ids.sort_unstable();
    sources.asr_voice_example_ids.sort_unstable();
    sources.media.sort();
    if has_duplicates(&sources.provider_account_ids)
        || has_duplicates(&sources.model_profile_ids)
        || has_duplicates(&sources.prompt_ids)
        || has_duplicates(&sources.provider_secrets)
        || has_duplicates(&sources.lorebook_ids)
        || has_duplicates(&sources.lorebook_entry_ids)
        || has_duplicates(&sources.asr_vocabulary_ids)
        || has_duplicates(&sources.asr_correction_ids)
        || has_duplicates(&sources.asr_ignored_suggestion_ids)
        || has_duplicates(&sources.asr_voice_example_ids)
        || sources.asr_vocabulary_ids.iter().any(|id| *id <= 0)
        || sources.asr_correction_ids.iter().any(|id| *id <= 0)
        || sources.asr_ignored_suggestion_ids.iter().any(|id| *id <= 0)
        || sources.asr_voice_example_ids.iter().any(|id| *id <= 0)
        || has_duplicates(&sources.media)
        || sources.media.iter().any(|source| {
            !valid_media_path(&source.relative_path) || i64::try_from(source.byte_len).is_err()
        })
        || sources.provider_secrets.iter().any(|source| {
            sources
                .provider_account_ids
                .binary_search(&source.provider_account_id)
                .is_err()
        })
    {
        return Err(LegacyImportRepositoryError::InvalidInput);
    }
    Ok(())
}

fn has_duplicates<T: PartialEq>(values: &[T]) -> bool {
    values.windows(2).any(|pair| pair[0] == pair[1])
}

fn valid_media_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= MAX_MEDIA_PATH_BYTES
        && !path.starts_with('/')
        && !path.contains('\\')
        && path
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

fn insert_assignments(
    transaction: &Transaction<'_>,
    run_id: LegacyImportRunId,
    sources: &LegacyImportSources,
) -> Result<(), LegacyImportRepositoryError> {
    for source_id in &sources.provider_account_ids {
        insert_assignment(
            transaction,
            run_id,
            "provider_account",
            &source_id.to_string(),
            "",
            ProviderAccountId::new().to_string(),
            Some(SecretOwnerId::new().as_uuid().to_string()),
        )?;
    }
    for source_id in &sources.model_profile_ids {
        insert_assignment(
            transaction,
            run_id,
            "model_profile",
            &source_id.to_string(),
            "",
            ModelProfileId::new().to_string(),
            None,
        )?;
    }
    for source_id in &sources.prompt_ids {
        insert_assignment(
            transaction,
            run_id,
            "prompt",
            source_id,
            "",
            PromptDocumentId::new().to_string(),
            None,
        )?;
    }
    for source in &sources.provider_secrets {
        let (source_kind, source_detail) = match &source.secret {
            LegacyPendingProviderSecret::ApiKey => ("provider_api_key", ""),
            LegacyPendingProviderSecret::Header { name } => {
                ("provider_secret_header", name.as_str())
            }
        };
        insert_assignment(
            transaction,
            run_id,
            source_kind,
            &source.provider_account_id.to_string(),
            source_detail,
            SecretRef::new().to_string(),
            None,
        )?;
    }
    for source_id in &sources.persona_ids {
        insert_assignment(
            transaction,
            run_id,
            "persona",
            &source_id.to_string(),
            "",
            PersonaId::new().to_string(),
            None,
        )?;
    }
    for source_id in &sources.lorebook_ids {
        insert_assignment(
            transaction,
            run_id,
            "lorebook",
            &source_id.to_string(),
            "",
            LorebookId::new().to_string(),
            None,
        )?;
    }
    for source_id in &sources.lorebook_entry_ids {
        insert_assignment(
            transaction,
            run_id,
            "lorebook_entry",
            &source_id.to_string(),
            "",
            LorebookEntryId::new().to_string(),
            None,
        )?;
    }
    for source_id in &sources.asr_vocabulary_ids {
        insert_assignment(
            transaction,
            run_id,
            "asr_vocabulary",
            &source_id.to_string(),
            "",
            AsrVocabularyTermId::new().to_string(),
            None,
        )?;
    }
    for source_id in &sources.asr_correction_ids {
        insert_assignment(
            transaction,
            run_id,
            "asr_correction",
            &source_id.to_string(),
            "",
            AsrCorrectionId::new().to_string(),
            None,
        )?;
    }
    for source_id in &sources.asr_ignored_suggestion_ids {
        insert_assignment(
            transaction,
            run_id,
            "asr_ignored_suggestion",
            &source_id.to_string(),
            "",
            AsrIgnoredSuggestionId::new().to_string(),
            None,
        )?;
    }
    for source_id in &sources.asr_voice_example_ids {
        insert_assignment(
            transaction,
            run_id,
            "asr_voice_example",
            &source_id.to_string(),
            "",
            AsrVoiceExampleId::new().to_string(),
            None,
        )?;
    }
    for source in &sources.media {
        insert_media_assignment(
            transaction,
            run_id,
            &source.relative_path,
            AssetId::new().to_string(),
            source.byte_len,
            &source.content_hash,
        )?;
    }
    Ok(())
}

fn insert_assignment(
    transaction: &Transaction<'_>,
    run_id: LegacyImportRunId,
    source_kind: &str,
    source_key: &str,
    source_detail: &str,
    destination_id: String,
    auxiliary_id: Option<String>,
) -> Result<(), LegacyImportRepositoryError> {
    transaction
        .execute(
            "INSERT INTO legacy_import_assignments (run_id,source_kind,source_key,source_detail,destination_id,auxiliary_id,expected_byte_len,expected_content_hash) VALUES (?1,?2,?3,?4,?5,?6,NULL,NULL)",
            params![run_id.to_string(), source_kind, source_key, source_detail, destination_id, auxiliary_id],
        )
        .map_err(|_| LegacyImportRepositoryError::Storage)?;
    Ok(())
}

fn insert_media_assignment(
    transaction: &Transaction<'_>,
    run_id: LegacyImportRunId,
    source_key: &str,
    destination_id: String,
    byte_len: u64,
    content_hash: &ContentHash,
) -> Result<(), LegacyImportRepositoryError> {
    transaction
        .execute(
            "INSERT INTO legacy_import_assignments (run_id,source_kind,source_key,source_detail,destination_id,auxiliary_id,expected_byte_len,expected_content_hash) VALUES (?1,'media',?2,'',?3,NULL,?4,?5)",
            params![
                run_id.to_string(),
                source_key,
                destination_id,
                i64::try_from(byte_len).map_err(|_| LegacyImportRepositoryError::InvalidInput)?,
                content_hash.as_str(),
            ],
        )
        .map_err(|_| LegacyImportRepositoryError::Storage)?;
    Ok(())
}

fn load_admission(
    transaction: &Transaction<'_>,
    run_id: LegacyImportRunId,
) -> Result<Option<LegacyImportAdmission>, LegacyImportRepositoryError> {
    let row = transaction
        .query_row(
            "SELECT source_schema_version,inventory_fingerprint,plan_fingerprint,status,admitted_at FROM legacy_import_runs WHERE id=?1",
            [run_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()
        .map_err(|_| LegacyImportRepositoryError::Storage)?;
    let Some((source_schema_version, inventory, plan, status, admitted_at)) = row else {
        return Ok(None);
    };
    Ok(Some(LegacyImportAdmission {
        run_id,
        source_schema_version,
        inventory_fingerprint: ContentHash::parse(inventory)
            .map_err(|_| LegacyImportRepositoryError::Storage)?,
        plan_fingerprint: ContentHash::parse(plan)
            .map_err(|_| LegacyImportRepositoryError::Storage)?,
        status: parse_status(&status)?,
        assignments: load_assignments(transaction, run_id)?,
        admitted_at: TimestampMillis::new(admitted_at),
        replayed: false,
    }))
}

fn parse_status(value: &str) -> Result<LegacyImportRunStatus, LegacyImportRepositoryError> {
    match value {
        "admitted" => Ok(LegacyImportRunStatus::Admitted),
        "importing" => Ok(LegacyImportRunStatus::Importing),
        "completed" => Ok(LegacyImportRunStatus::Completed),
        "failed" => Ok(LegacyImportRunStatus::Failed),
        _ => Err(LegacyImportRepositoryError::Storage),
    }
}

fn load_media_completion(
    transaction: &Transaction<'_>,
    run_id: LegacyImportRunId,
    relative_path: &str,
) -> Result<Option<LegacyImportMediaCompletion>, LegacyImportRepositoryError> {
    transaction
        .query_row(
            "SELECT destination_asset_id,blob_id,byte_len,content_hash,completed_at FROM legacy_import_media_completions WHERE run_id=?1 AND relative_path=?2",
            params![run_id.to_string(), relative_path],
            |row| {
                let byte_len = u64::try_from(row.get::<_, i64>(2)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                Ok(LegacyImportMediaCompletion {
                    run_id,
                    relative_path: relative_path.to_owned(),
                    destination_asset_id: parse_database_id(row.get(0)?)?,
                    blob_id: parse_database_id(row.get(1)?)?,
                    byte_len,
                    content_hash: ContentHash::parse(row.get::<_, String>(3)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    completed_at: TimestampMillis::new(row.get(4)?),
                    replayed: false,
                })
            },
        )
        .optional()
        .map_err(|_| LegacyImportRepositoryError::Storage)
}

fn parse_database_id<T: FromStr>(value: String) -> Result<T, rusqlite::Error> {
    value.parse().map_err(|_| rusqlite::Error::InvalidQuery)
}

fn load_assignments(
    transaction: &Transaction<'_>,
    run_id: LegacyImportRunId,
) -> Result<Vec<LegacyImportAssignment>, LegacyImportRepositoryError> {
    let mut statement = transaction
        .prepare(
            "SELECT source_kind,source_key,source_detail,destination_id,auxiliary_id,expected_byte_len,expected_content_hash FROM legacy_import_assignments WHERE run_id=?1 ORDER BY CASE source_kind WHEN 'provider_account' THEN 1 WHEN 'model_profile' THEN 2 WHEN 'provider_api_key' THEN 3 WHEN 'provider_secret_header' THEN 4 WHEN 'prompt' THEN 5 WHEN 'persona' THEN 6 WHEN 'lorebook' THEN 7 WHEN 'lorebook_entry' THEN 8 WHEN 'asr_vocabulary' THEN 9 WHEN 'asr_correction' THEN 10 WHEN 'asr_ignored_suggestion' THEN 11 WHEN 'asr_voice_example' THEN 12 ELSE 13 END,source_key,source_detail",
        )
        .map_err(|_| LegacyImportRepositoryError::Storage)?;
    statement
        .query_map([run_id.to_string()], |row| {
            let source_kind: String = row.get(0)?;
            let source_key: String = row.get(1)?;
            let source_detail: String = row.get(2)?;
            let destination_id: String = row.get(3)?;
            let auxiliary_id: Option<String> = row.get(4)?;
            let expected_byte_len: Option<i64> = row.get(5)?;
            let expected_content_hash: Option<String> = row.get(6)?;
            parse_assignment(
                &source_kind,
                source_key,
                source_detail,
                destination_id,
                auxiliary_id,
                expected_byte_len,
                expected_content_hash,
            )
            .map_err(|_| rusqlite::Error::InvalidQuery)
        })
        .map_err(|_| LegacyImportRepositoryError::Storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| LegacyImportRepositoryError::Storage)
}

fn parse_assignment(
    source_kind: &str,
    source_key: String,
    source_detail: String,
    destination_id: String,
    auxiliary_id: Option<String>,
    expected_byte_len: Option<i64>,
    expected_content_hash: Option<String>,
) -> Result<LegacyImportAssignment, LegacyImportRepositoryError> {
    match source_kind {
        "provider_account" => Ok(LegacyImportAssignment::ProviderAccount {
            legacy_id: ProviderAccountId::from_str(&source_key)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
            destination_id: ProviderAccountId::from_str(&destination_id)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
            secret_owner_id: SecretOwnerId::from_uuid(
                uuid::Uuid::parse_str(
                    auxiliary_id
                        .as_deref()
                        .ok_or(LegacyImportRepositoryError::Storage)?,
                )
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
            ),
        }),
        "model_profile" => Ok(LegacyImportAssignment::ModelProfile {
            legacy_id: ModelProfileId::from_str(&source_key)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
            destination_id: ModelProfileId::from_str(&destination_id)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
        }),
        "prompt" => Ok(LegacyImportAssignment::Prompt {
            legacy_id: source_key,
            destination_id: PromptDocumentId::from_str(&destination_id)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
        }),
        "provider_api_key" => Ok(LegacyImportAssignment::ProviderSecret {
            source: LegacyImportProviderSecretSource {
                provider_account_id: ProviderAccountId::from_str(&source_key)
                    .map_err(|_| LegacyImportRepositoryError::Storage)?,
                secret: LegacyPendingProviderSecret::ApiKey,
            },
            destination_ref: SecretRef::from_uuid(
                uuid::Uuid::parse_str(&destination_id)
                    .map_err(|_| LegacyImportRepositoryError::Storage)?,
            ),
        }),
        "provider_secret_header" => Ok(LegacyImportAssignment::ProviderSecret {
            source: LegacyImportProviderSecretSource {
                provider_account_id: ProviderAccountId::from_str(&source_key)
                    .map_err(|_| LegacyImportRepositoryError::Storage)?,
                secret: LegacyPendingProviderSecret::Header {
                    name: HeaderName::new(source_detail)
                        .map_err(|_| LegacyImportRepositoryError::Storage)?,
                },
            },
            destination_ref: SecretRef::from_uuid(
                uuid::Uuid::parse_str(&destination_id)
                    .map_err(|_| LegacyImportRepositoryError::Storage)?,
            ),
        }),
        "persona" => Ok(LegacyImportAssignment::Persona {
            legacy_id: PersonaId::from_str(&source_key)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
            destination_id: PersonaId::from_str(&destination_id)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
        }),
        "lorebook" => Ok(LegacyImportAssignment::Lorebook {
            legacy_id: LorebookId::from_str(&source_key)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
            destination_id: LorebookId::from_str(&destination_id)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
        }),
        "lorebook_entry" => Ok(LegacyImportAssignment::LorebookEntry {
            legacy_id: LorebookEntryId::from_str(&source_key)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
            destination_id: LorebookEntryId::from_str(&destination_id)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
        }),
        "asr_vocabulary" => Ok(LegacyImportAssignment::AsrVocabulary {
            legacy_id: source_key
                .parse()
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
            destination_id: AsrVocabularyTermId::from_str(&destination_id)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
        }),
        "asr_correction" => Ok(LegacyImportAssignment::AsrCorrection {
            legacy_id: source_key
                .parse()
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
            destination_id: AsrCorrectionId::from_str(&destination_id)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
        }),
        "asr_ignored_suggestion" => Ok(LegacyImportAssignment::AsrIgnoredSuggestion {
            legacy_id: source_key
                .parse()
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
            destination_id: AsrIgnoredSuggestionId::from_str(&destination_id)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
        }),
        "asr_voice_example" => Ok(LegacyImportAssignment::AsrVoiceExample {
            legacy_id: source_key
                .parse()
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
            destination_id: AsrVoiceExampleId::from_str(&destination_id)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
        }),
        "media" => Ok(LegacyImportAssignment::Media {
            relative_path: source_key,
            destination_id: AssetId::from_str(&destination_id)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
            byte_len: u64::try_from(expected_byte_len.ok_or(LegacyImportRepositoryError::Storage)?)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
            content_hash: ContentHash::parse(
                expected_content_hash.ok_or(LegacyImportRepositoryError::Storage)?,
            )
            .map_err(|_| LegacyImportRepositoryError::Storage)?,
        }),
        _ => Err(LegacyImportRepositoryError::Storage),
    }
}

fn assignment_sources(assignments: &[LegacyImportAssignment]) -> LegacyImportSources {
    let mut sources = LegacyImportSources {
        provider_account_ids: Vec::new(),
        model_profile_ids: Vec::new(),
        prompt_ids: Vec::new(),
        provider_secrets: Vec::new(),
        persona_ids: Vec::new(),
        lorebook_ids: Vec::new(),
        lorebook_entry_ids: Vec::new(),
        asr_vocabulary_ids: Vec::new(),
        asr_correction_ids: Vec::new(),
        asr_ignored_suggestion_ids: Vec::new(),
        asr_voice_example_ids: Vec::new(),
        media: Vec::new(),
    };
    for assignment in assignments {
        match assignment {
            LegacyImportAssignment::ProviderAccount { legacy_id, .. } => {
                sources.provider_account_ids.push(*legacy_id);
            }
            LegacyImportAssignment::ModelProfile { legacy_id, .. } => {
                sources.model_profile_ids.push(*legacy_id);
            }
            LegacyImportAssignment::Prompt { legacy_id, .. } => {
                sources.prompt_ids.push(legacy_id.clone());
            }
            LegacyImportAssignment::ProviderSecret { source, .. } => {
                sources.provider_secrets.push(source.clone());
            }
            LegacyImportAssignment::Persona { legacy_id, .. } => {
                sources.persona_ids.push(*legacy_id);
            }
            LegacyImportAssignment::Lorebook { legacy_id, .. } => {
                sources.lorebook_ids.push(*legacy_id);
            }
            LegacyImportAssignment::LorebookEntry { legacy_id, .. } => {
                sources.lorebook_entry_ids.push(*legacy_id);
            }
            LegacyImportAssignment::AsrVocabulary { legacy_id, .. } => {
                sources.asr_vocabulary_ids.push(*legacy_id);
            }
            LegacyImportAssignment::AsrCorrection { legacy_id, .. } => {
                sources.asr_correction_ids.push(*legacy_id);
            }
            LegacyImportAssignment::AsrIgnoredSuggestion { legacy_id, .. } => {
                sources.asr_ignored_suggestion_ids.push(*legacy_id);
            }
            LegacyImportAssignment::AsrVoiceExample { legacy_id, .. } => {
                sources.asr_voice_example_ids.push(*legacy_id);
            }
            LegacyImportAssignment::Media {
                relative_path,
                byte_len,
                content_hash,
                ..
            } => {
                sources.media.push(LegacyImportMediaSource {
                    relative_path: relative_path.clone(),
                    byte_len: *byte_len,
                    content_hash: content_hash.clone(),
                });
            }
        }
    }
    sources
}

#[cfg(test)]
mod tests {
    use std::fs;

    use lettuce_settings::HeaderName;
    use lettuce_transfer::{
        LEGACY_DATABASE_SCHEMA_VERSION, LegacyImportAdmissionRequest, LegacyImportAssignment,
        LegacyImportProviderSecretSource, LegacyImportRepository, LegacyImportRepositoryError,
        LegacyImportRunStatus, LegacyImportSources, LegacyPendingProviderSecret,
    };
    use lettuce_types::{
        ContentHash, LegacyImportRunId, LorebookEntryId, LorebookId, ModelProfileId, PersonaId,
        ProviderAccountId, TimestampMillis,
    };

    use crate::Database;

    fn database_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "lettuce-legacy-import-{label}-{}.sqlite3",
            LegacyImportRunId::new()
        ))
    }

    fn request(run_id: LegacyImportRunId) -> LegacyImportAdmissionRequest {
        let provider_account_id = ProviderAccountId::new();
        LegacyImportAdmissionRequest {
            run_id,
            source_schema_version: LEGACY_DATABASE_SCHEMA_VERSION,
            inventory_fingerprint: ContentHash::parse("ab".repeat(32)).expect("inventory hash"),
            plan_fingerprint: ContentHash::parse("cd".repeat(32)).expect("plan hash"),
            sources: LegacyImportSources {
                provider_account_ids: vec![provider_account_id],
                model_profile_ids: vec![ModelProfileId::new()],
                prompt_ids: Vec::new(),
                provider_secrets: vec![
                    LegacyImportProviderSecretSource {
                        provider_account_id,
                        secret: LegacyPendingProviderSecret::Header {
                            name: HeaderName::new("x-alpha-key").expect("header name"),
                        },
                    },
                    LegacyImportProviderSecretSource {
                        provider_account_id,
                        secret: LegacyPendingProviderSecret::ApiKey,
                    },
                    LegacyImportProviderSecretSource {
                        provider_account_id,
                        secret: LegacyPendingProviderSecret::Header {
                            name: HeaderName::new("x-zeta-key").expect("header name"),
                        },
                    },
                ],
                persona_ids: vec![PersonaId::new()],
                lorebook_ids: vec![LorebookId::new()],
                lorebook_entry_ids: vec![LorebookEntryId::new()],
                asr_vocabulary_ids: vec![3],
                asr_correction_ids: vec![4],
                asr_ignored_suggestion_ids: vec![5],
                asr_voice_example_ids: vec![6],
                media: vec![lettuce_transfer::LegacyImportMediaSource {
                    relative_path: "images/avatar.png".to_owned(),
                    byte_len: 42,
                    content_hash: ContentHash::parse("12".repeat(32)).expect("media hash"),
                }],
            },
            admitted_at: TimestampMillis::new(100),
        }
    }

    #[test]
    fn admission_replays_assignments_and_conflicts_after_reopen() {
        let path = database_path("replay");
        let run_id = LegacyImportRunId::new();
        let original = request(run_id);
        let first_database = Database::open(&path).expect("open database");
        let first = first_database
            .admit(original.clone())
            .expect("admit import");
        assert_eq!(first.status, LegacyImportRunStatus::Admitted);
        assert!(!first.replayed);
        assert_eq!(first.assignments.len(), 13);
        assert!(first.assignments.iter().any(|assignment| matches!(
            assignment,
            LegacyImportAssignment::AsrVocabulary { legacy_id: 3, .. }
        )));
        assert!(first.assignments.iter().any(|assignment| matches!(
            assignment,
            LegacyImportAssignment::AsrCorrection { legacy_id: 4, .. }
        )));
        assert!(first.assignments.iter().any(|assignment| matches!(
            assignment,
            LegacyImportAssignment::AsrIgnoredSuggestion { legacy_id: 5, .. }
        )));
        assert!(first.assignments.iter().any(|assignment| matches!(
            assignment,
            LegacyImportAssignment::AsrVoiceExample { legacy_id: 6, .. }
        )));
        let provider_assignments = first
            .assignments
            .iter()
            .filter(|assignment| {
                matches!(
                    assignment,
                    LegacyImportAssignment::ProviderAccount { .. }
                        | LegacyImportAssignment::ModelProfile { .. }
                        | LegacyImportAssignment::ProviderSecret { .. }
                )
            })
            .count();
        assert_eq!(provider_assignments, 5);
        let secret_order = first
            .assignments
            .iter()
            .filter_map(|assignment| match assignment {
                LegacyImportAssignment::ProviderSecret { source, .. } => {
                    Some(match &source.secret {
                        LegacyPendingProviderSecret::ApiKey => "api_key",
                        LegacyPendingProviderSecret::Header { name } => name.as_str(),
                    })
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(secret_order, vec!["api_key", "x-alpha-key", "x-zeta-key"]);
        let replay = first_database
            .admit(original.clone())
            .expect("replay import");
        assert!(replay.replayed);
        assert_eq!(replay.assignments, first.assignments);
        assert_eq!(replay.admitted_at, first.admitted_at);
        drop(first_database);

        let reopened = Database::open(&path).expect("reopen database");
        let reopened_replay = reopened
            .admit(original.clone())
            .expect("replay after reopen");
        assert!(reopened_replay.replayed);
        assert_eq!(reopened_replay.assignments, first.assignments);

        let mut changed_source = original.clone();
        changed_source.sources.persona_ids = vec![PersonaId::new()];
        assert_eq!(
            reopened.admit(changed_source),
            Err(LegacyImportRepositoryError::Conflict)
        );
        let mut changed_plan = original;
        changed_plan.plan_fingerprint =
            ContentHash::parse("ef".repeat(32)).expect("changed plan hash");
        assert_eq!(
            reopened.admit(changed_plan),
            Err(LegacyImportRepositoryError::Conflict)
        );
        let connection = reopened.connection().expect("database lock");
        assert!(
            connection
                .execute(
                    "UPDATE legacy_import_runs SET plan_fingerprint=?2 WHERE id=?1",
                    rusqlite::params![run_id.to_string(), "11".repeat(32)],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "INSERT INTO legacy_import_assignments (run_id,source_kind,source_key,destination_id) VALUES (?1,'media','images/late.png',?2)",
                    rusqlite::params![run_id.to_string(), lettuce_types::AssetId::new().to_string()],
                )
                .is_err()
        );
        let domain_rows: u32 = connection
            .query_row(
                "SELECT (SELECT count(*) FROM personas) + (SELECT count(*) FROM lorebooks) + (SELECT count(*) FROM lorebook_entries) + (SELECT count(*) FROM media_assets)",
                [],
                |row| row.get(0),
            )
            .expect("domain row count");
        assert_eq!(domain_rows, 0);
        let secret_columns: u32 = connection
            .query_row(
                "SELECT count(*) FROM pragma_table_info('legacy_import_assignments') WHERE lower(name) LIKE '%value%' OR lower(name) LIKE '%bytes%'",
                [],
                |row| row.get(0),
            )
            .expect("secret-bearing columns");
        assert_eq!(secret_columns, 0);
        drop(connection);
        drop(reopened);
        fs::remove_file(path).expect("remove database");
    }

    #[test]
    fn assignment_failure_rolls_back_the_entire_admission() {
        let path = database_path("rollback");
        let run_id = LegacyImportRunId::new();
        let database = Database::open(&path).expect("open database");
        database
            .connection()
            .expect("database lock")
            .execute_batch(
                "CREATE TRIGGER reject_legacy_lorebook_assignment BEFORE INSERT ON legacy_import_assignments WHEN NEW.source_kind='lorebook' BEGIN SELECT RAISE(ABORT, 'test rollback'); END;",
            )
            .expect("install rollback trigger");
        assert_eq!(
            database.admit(request(run_id)),
            Err(LegacyImportRepositoryError::Storage)
        );
        let connection = database.connection().expect("database lock");
        let run_count: u32 = connection
            .query_row(
                "SELECT count(*) FROM legacy_import_runs WHERE id=?1",
                [run_id.to_string()],
                |row| row.get(0),
            )
            .expect("run count");
        let assignment_count: u32 = connection
            .query_row(
                "SELECT count(*) FROM legacy_import_assignments WHERE run_id=?1",
                [run_id.to_string()],
                |row| row.get(0),
            )
            .expect("assignment count");
        assert_eq!((run_count, assignment_count), (0, 0));
        drop(connection);
        drop(database);
        fs::remove_file(path).expect("remove database");
    }

    #[test]
    fn assignment_identity_collisions_are_rejected() {
        let path = database_path("identity-collision");
        let run_id = LegacyImportRunId::new();
        let database = Database::open(&path).expect("open database");
        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "INSERT INTO legacy_import_runs (id,source_schema_version,inventory_fingerprint,plan_fingerprint,status,admitted_at,updated_at) VALUES (?1,92,?2,?3,'admitting',1,1)",
                rusqlite::params![run_id.to_string(), "ab".repeat(32), "cd".repeat(32)],
            )
            .expect("insert admitting run");
        let destination_id = ProviderAccountId::new().to_string();
        let secret_owner_id = uuid::Uuid::new_v4().to_string();
        connection
            .execute(
                "INSERT INTO legacy_import_assignments (run_id,source_kind,source_key,source_detail,destination_id,auxiliary_id) VALUES (?1,'provider_account',?2,'',?3,?4)",
                rusqlite::params![
                    run_id.to_string(),
                    ProviderAccountId::new().to_string(),
                    destination_id,
                    secret_owner_id,
                ],
            )
            .expect("insert first assignment");
        assert!(
            connection
                .execute(
                    "INSERT INTO legacy_import_assignments (run_id,source_kind,source_key,source_detail,destination_id,auxiliary_id) VALUES (?1,'provider_account',?2,'',?3,?4)",
                    rusqlite::params![
                        run_id.to_string(),
                        ProviderAccountId::new().to_string(),
                        destination_id,
                        uuid::Uuid::new_v4().to_string(),
                    ],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "INSERT INTO legacy_import_assignments (run_id,source_kind,source_key,source_detail,destination_id,auxiliary_id) VALUES (?1,'provider_account',?2,'',?3,?4)",
                    rusqlite::params![
                        run_id.to_string(),
                        ProviderAccountId::new().to_string(),
                        ProviderAccountId::new().to_string(),
                        secret_owner_id,
                    ],
                )
                .is_err()
        );
        drop(connection);
        drop(database);
        fs::remove_file(path).expect("remove database");
    }
}
