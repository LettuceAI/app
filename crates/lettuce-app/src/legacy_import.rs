use lettuce_transfer::{
    LEGACY_DATABASE_SCHEMA_VERSION, LegacyCrop, LegacyDatabaseInventory, LegacyImageRecommendation,
    LegacyImportAdmission, LegacyImportAdmissionRequest, LegacyImportExecutionRequest,
    LegacyImportReceipt, LegacyImportRepository, LegacyImportRepositoryError, LegacyImportSources,
    LegacyKeywordMatchMode, LegacyLorebookDetectionPolicy, LegacyLorebookPlan, LegacyMediaPlan,
    LegacyMediaUse, LegacyPersonaPlan,
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
        personas: &LegacyPersonaPlan,
        lorebooks: &LegacyLorebookPlan,
        media: &LegacyMediaPlan,
        completed_at: TimestampMillis,
    ) -> Result<LegacyImportReceipt, LegacyImportRepositoryError> {
        let fingerprint = plan_fingerprint(personas, lorebooks, media);
        if fingerprint != admission.plan_fingerprint {
            return Err(LegacyImportRepositoryError::Conflict);
        }
        self.repository.materialize(LegacyImportExecutionRequest {
            run_id: admission.run_id,
            plan_fingerprint: fingerprint,
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
        personas: &LegacyPersonaPlan,
        lorebooks: &LegacyLorebookPlan,
        media: &LegacyMediaPlan,
        admitted_at: TimestampMillis,
    ) -> Result<LegacyImportAdmission, LegacyImportRepositoryError> {
        validate_plan(inventory, personas, lorebooks, media)?;
        self.repository.admit(LegacyImportAdmissionRequest {
            run_id,
            source_schema_version: inventory.schema_version,
            inventory_fingerprint: inventory_fingerprint(inventory),
            plan_fingerprint: plan_fingerprint(personas, lorebooks, media),
            sources: LegacyImportSources {
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
    personas: &LegacyPersonaPlan,
    lorebooks: &LegacyLorebookPlan,
    media: &LegacyMediaPlan,
) -> Result<(), LegacyImportRepositoryError> {
    let persona_count = u64::try_from(personas.personas.len())
        .map_err(|_| LegacyImportRepositoryError::InvalidInput)?;
    let lorebook_count = u64::try_from(lorebooks.lorebooks.len())
        .map_err(|_| LegacyImportRepositoryError::InvalidInput)?;
    let total_bytes = media.media.iter().try_fold(0_u64, |total, candidate| {
        total.checked_add(candidate.byte_len)
    });
    if inventory.schema_version != LEGACY_DATABASE_SCHEMA_VERSION
        || inventory.personas != persona_count
        || inventory.lorebooks != lorebook_count
        || total_bytes != Some(media.total_bytes)
    {
        return Err(LegacyImportRepositoryError::InvalidInput);
    }
    Ok(())
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

fn plan_fingerprint(
    personas: &LegacyPersonaPlan,
    lorebooks: &LegacyLorebookPlan,
    media: &LegacyMediaPlan,
) -> ContentHash {
    let mut hash = Fingerprint::new("lettuce-legacy-persona-lorebook-plan-v1");
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
    use std::fs;

    use lettuce_characters::{Persona, PersonaRepository};
    use lettuce_context::LorebookRepository;
    use lettuce_transfer::{
        LegacyDatabaseInventory, LegacyImportAssignment, LegacyImportRepositoryError,
        LegacyImportRunStatus, LegacyLorebookCandidate, LegacyLorebookDetectionPolicy,
        LegacyLorebookPlan, LegacyMediaPlan, LegacyPersonaCandidate, LegacyPersonaPlan,
    };
    use lettuce_types::{LegacyImportRunId, LorebookId, PersonaId, TimestampMillis};

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
                &personas,
                &lorebooks,
                &media,
                TimestampMillis::new(20),
            )
            .expect("admit import");
        let replay = backend
            .legacy_import_admission()
            .admit(
                run_id,
                &inventory,
                &personas,
                &lorebooks,
                &media,
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
                &changed,
                &lorebooks,
                &media,
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
                &empty_personas,
                &empty_books,
                &empty_media,
                TimestampMillis::new(20),
            )
            .expect("admit empty media import");
        let empty_receipt = backend
            .legacy_import_executor()
            .execute(
                &empty_admission,
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
                &collision_personas,
                &collision_books,
                &empty_media,
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
                &collision_personas,
                &collision_books,
                &empty_media,
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
