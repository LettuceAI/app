use std::{collections::BTreeMap, str::FromStr};

use lettuce_characters::{
    Crop, ImageRecommendation, LifecycleStatus, Persona, PersonaMedia, PersonaMediaLink,
    PersonaMediaSlot,
};
use lettuce_context::{
    DetectionPolicy, KeywordMatchMode, LifecycleStatus as LorebookLifecycleStatus, Lorebook,
    LorebookBehaviorVersion, LorebookBinding, LorebookDetails, LorebookEntry,
};
use lettuce_transfer::{
    LEGACY_DATABASE_SCHEMA_VERSION, LegacyImportAdmission, LegacyImportAdmissionRequest,
    LegacyImportAssignment, LegacyImportExecutionRequest, LegacyImportMediaCompletion,
    LegacyImportMediaCompletionRequest, LegacyImportMediaSource, LegacyImportReceipt,
    LegacyImportRepository, LegacyImportRepositoryError, LegacyImportRunStatus,
    LegacyImportSources, LegacyKeywordMatchMode, LegacyLorebookDetectionPolicy, LegacyMediaUse,
};
use lettuce_types::{
    AssetId, ContentHash, LegacyImportRunId, LorebookEntryId, LorebookId, PersonaId, Revision,
    TimestampMillis,
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
            if admission.status != LegacyImportRunStatus::Completed {
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
        let changed = transaction
            .execute(
                "UPDATE legacy_import_runs SET status='completed',updated_at=?2 WHERE id=?1 AND status='importing'",
                params![request.run_id.to_string(), request.completed_at.get()],
            )
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        if changed != 1 {
            return Err(LegacyImportRepositoryError::Conflict);
        }
        let receipt = load_receipt(&transaction, request.run_id)?
            .ok_or(LegacyImportRepositoryError::Storage)?;
        transaction
            .commit()
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        Ok(receipt)
    }
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
    personas: BTreeMap<PersonaId, PersonaId>,
    lorebooks: BTreeMap<LorebookId, LorebookId>,
    entries: BTreeMap<LorebookEntryId, LorebookEntryId>,
    media: BTreeMap<String, (AssetId, u64, ContentHash)>,
}

impl AssignmentMaps {
    fn from_admission(
        admission: &LegacyImportAdmission,
    ) -> Result<Self, LegacyImportRepositoryError> {
        let mut maps = Self {
            personas: BTreeMap::new(),
            lorebooks: BTreeMap::new(),
            entries: BTreeMap::new(),
            media: BTreeMap::new(),
        };
        for assignment in &admission.assignments {
            let duplicate = match assignment {
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

fn normalize_sources(sources: &mut LegacyImportSources) -> Result<(), LegacyImportRepositoryError> {
    sources.persona_ids.sort_unstable();
    sources.lorebook_ids.sort_unstable();
    sources.lorebook_entry_ids.sort_unstable();
    sources.media.sort();
    if has_duplicates(&sources.persona_ids)
        || has_duplicates(&sources.lorebook_ids)
        || has_duplicates(&sources.lorebook_entry_ids)
        || has_duplicates(&sources.media)
        || sources.media.iter().any(|source| {
            !valid_media_path(&source.relative_path) || i64::try_from(source.byte_len).is_err()
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
    for source_id in &sources.persona_ids {
        insert_assignment(
            transaction,
            run_id,
            "persona",
            &source_id.to_string(),
            PersonaId::new().to_string(),
        )?;
    }
    for source_id in &sources.lorebook_ids {
        insert_assignment(
            transaction,
            run_id,
            "lorebook",
            &source_id.to_string(),
            LorebookId::new().to_string(),
        )?;
    }
    for source_id in &sources.lorebook_entry_ids {
        insert_assignment(
            transaction,
            run_id,
            "lorebook_entry",
            &source_id.to_string(),
            LorebookEntryId::new().to_string(),
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
    destination_id: String,
) -> Result<(), LegacyImportRepositoryError> {
    transaction
        .execute(
            "INSERT INTO legacy_import_assignments (run_id,source_kind,source_key,destination_id,expected_byte_len,expected_content_hash) VALUES (?1,?2,?3,?4,NULL,NULL)",
            params![run_id.to_string(), source_kind, source_key, destination_id],
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
            "INSERT INTO legacy_import_assignments (run_id,source_kind,source_key,destination_id,expected_byte_len,expected_content_hash) VALUES (?1,'media',?2,?3,?4,?5)",
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
            "SELECT source_kind,source_key,destination_id,expected_byte_len,expected_content_hash FROM legacy_import_assignments WHERE run_id=?1 ORDER BY CASE source_kind WHEN 'persona' THEN 1 WHEN 'lorebook' THEN 2 WHEN 'lorebook_entry' THEN 3 ELSE 4 END,source_key",
        )
        .map_err(|_| LegacyImportRepositoryError::Storage)?;
    statement
        .query_map([run_id.to_string()], |row| {
            let source_kind: String = row.get(0)?;
            let source_key: String = row.get(1)?;
            let destination_id: String = row.get(2)?;
            let expected_byte_len: Option<i64> = row.get(3)?;
            let expected_content_hash: Option<String> = row.get(4)?;
            parse_assignment(
                &source_kind,
                source_key,
                destination_id,
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
    destination_id: String,
    expected_byte_len: Option<i64>,
    expected_content_hash: Option<String>,
) -> Result<LegacyImportAssignment, LegacyImportRepositoryError> {
    match source_kind {
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
        persona_ids: Vec::new(),
        lorebook_ids: Vec::new(),
        lorebook_entry_ids: Vec::new(),
        media: Vec::new(),
    };
    for assignment in assignments {
        match assignment {
            LegacyImportAssignment::Persona { legacy_id, .. } => {
                sources.persona_ids.push(*legacy_id);
            }
            LegacyImportAssignment::Lorebook { legacy_id, .. } => {
                sources.lorebook_ids.push(*legacy_id);
            }
            LegacyImportAssignment::LorebookEntry { legacy_id, .. } => {
                sources.lorebook_entry_ids.push(*legacy_id);
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

    use lettuce_transfer::{
        LEGACY_DATABASE_SCHEMA_VERSION, LegacyImportAdmissionRequest, LegacyImportRepository,
        LegacyImportRepositoryError, LegacyImportRunStatus, LegacyImportSources,
    };
    use lettuce_types::{
        ContentHash, LegacyImportRunId, LorebookEntryId, LorebookId, PersonaId, TimestampMillis,
    };

    use crate::Database;

    fn database_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "lettuce-legacy-import-{label}-{}.sqlite3",
            LegacyImportRunId::new()
        ))
    }

    fn request(run_id: LegacyImportRunId) -> LegacyImportAdmissionRequest {
        LegacyImportAdmissionRequest {
            run_id,
            source_schema_version: LEGACY_DATABASE_SCHEMA_VERSION,
            inventory_fingerprint: ContentHash::parse("ab".repeat(32)).expect("inventory hash"),
            plan_fingerprint: ContentHash::parse("cd".repeat(32)).expect("plan hash"),
            sources: LegacyImportSources {
                persona_ids: vec![PersonaId::new()],
                lorebook_ids: vec![LorebookId::new()],
                lorebook_entry_ids: vec![LorebookEntryId::new()],
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
        assert_eq!(first.assignments.len(), 4);
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
}
