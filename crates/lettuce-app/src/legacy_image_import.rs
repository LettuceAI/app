use std::collections::BTreeMap;

use lettuce_transfer::{
    LegacyBackupImagePlan, LegacyImageMaterializationRequest, LegacyImportAdmission,
    LegacyImportAssignment, LegacyImportPlan, LegacyImportRepository, LegacyImportRepositoryError,
    LegacyImportStageReceipt, LegacyMediaUse, LegacyPlaygroundImport,
};
use lettuce_types::AssetId;
use lettuce_types::TimestampMillis;

#[derive(Debug)]
pub struct LegacyImageImportCoordinator<'a, R: ?Sized> {
    repository: &'a R,
}

impl<'a, R: LegacyImportRepository + ?Sized> LegacyImageImportCoordinator<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    /// Writes the legacy LoRA library rows and playground history of an
    /// admitted run, linking each playground image to its imported asset.
    pub fn execute(
        &self,
        admission: &LegacyImportAdmission,
        plan: &LegacyImportPlan,
        images: &LegacyBackupImagePlan,
        completed_at: TimestampMillis,
    ) -> Result<LegacyImportStageReceipt, LegacyImportRepositoryError> {
        let plan_fingerprint = crate::legacy_import::plan_fingerprint(plan);
        if plan_fingerprint != admission.plan_fingerprint {
            return Err(LegacyImportRepositoryError::Conflict);
        }
        let source_fingerprint = plan
            .source_fingerprint
            .clone()
            .ok_or(LegacyImportRepositoryError::InvalidInput)?;
        let playground = playground_imports(
            admission,
            plan,
            images,
            &lettuce_transfer::LegacyIdScope::new(&source_fingerprint),
        );
        self.repository
            .materialize_images(LegacyImageMaterializationRequest {
                run_id: admission.run_id,
                plan_fingerprint,
                source_fingerprint,
                loras: images.loras.clone(),
                playground,
                completed_at,
            })
    }

    pub fn execute_database_import(
        &self,
        admission: &LegacyImportAdmission,
        import: &crate::LegacyDatabaseImportPlan,
        completed_at: TimestampMillis,
    ) -> Result<LegacyImportStageReceipt, LegacyImportRepositoryError> {
        self.execute(
            admission,
            &import.plan,
            &import.compatibility.images,
            completed_at,
        )
    }
}

fn playground_imports(
    admission: &LegacyImportAdmission,
    plan: &LegacyImportPlan,
    images: &LegacyBackupImagePlan,
    scope: &lettuce_transfer::LegacyIdScope,
) -> Vec<LegacyPlaygroundImport> {
    let assets = admission
        .assignments
        .iter()
        .filter_map(|assignment| match assignment {
            LegacyImportAssignment::Media {
                relative_path,
                destination_id,
                ..
            } => Some((relative_path.as_str(), *destination_id)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut by_image = BTreeMap::<(&str, u32), AssetId>::new();
    for candidate in &plan.media.media {
        let Some(asset_id) = assets.get(candidate.relative_path.as_str()) else {
            continue;
        };
        for media_use in &candidate.uses {
            if let LegacyMediaUse::PlaygroundImage {
                generation_id,
                ordinal,
            } = media_use
            {
                by_image.insert((generation_id.as_str(), *ordinal), *asset_id);
            }
        }
    }
    images
        .playground
        .iter()
        .map(|generation| LegacyPlaygroundImport {
            id: scope
                .derived(&generation.source_id, "playground-history")
                .to_string(),
            assets: (0..generation.images.len())
                .map(|ordinal| {
                    u32::try_from(ordinal).ok().and_then(|ordinal| {
                        by_image
                            .get(&(generation.source_id.as_str(), ordinal))
                            .copied()
                    })
                })
                .collect(),
            generation: generation.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use lettuce_image_generation::sd_runtime::lora_library::{
        LoraArchitectureSource, LoraKeywordSource, LoraLibraryRepository, LoraRecord,
    };
    use lettuce_transfer::{
        LegacyAsrPlan, LegacyDatabaseInventory, LegacyImageLoraRecord, LegacyLorebookPlan,
        LegacyMediaPlan, LegacyPersonaPlan, LegacyPromptPlan, LegacyProviderModelPlan,
    };
    use lettuce_types::{ContentHash, LegacyImportRunId};

    use super::*;
    use crate::AppBackend;

    fn plan() -> LegacyImportPlan {
        LegacyImportPlan {
            provider_models: LegacyProviderModelPlan {
                skipped: Vec::new(),
                provider_accounts: Vec::new(),
                model_profiles: Vec::new(),
                default_provider_account_id: None,
                default_model_profile_id: None,
            },
            prompts: LegacyPromptPlan {
                skipped: Vec::new(),
                prompts: Vec::new(),
                default_prompt_source_id: None,
                deprecated_system_prompt: None,
            },
            personas: LegacyPersonaPlan {
                skipped: Vec::new(),
                personas: Vec::new(),
                default_persona_id: None,
            },
            lorebooks: LegacyLorebookPlan {
                skipped: Vec::new(),
                lorebooks: Vec::new(),
            },
            asr: LegacyAsrPlan {
                vocabulary: Vec::new(),
                corrections: Vec::new(),
                ignored_suggestions: Vec::new(),
                voice_examples: Vec::new(),
            },
            media: LegacyMediaPlan {
                media: Vec::new(),
                total_bytes: 0,
                skipped: Vec::new(),
            },
            source_fingerprint: Some(ContentHash::parse("98".repeat(32)).expect("source hash")),
            later_skips: Vec::new(),
        }
    }

    fn lora(path: &str, keywords: &[&str], updated_at: i64) -> LegacyImageLoraRecord {
        LegacyImageLoraRecord {
            path: path.to_owned(),
            filename: path.rsplit('/').next().unwrap_or(path).to_owned(),
            bytes_on_disk: 2048,
            modified_at: 7,
            sha256: Some("ab".repeat(32)),
            keywords: keywords
                .iter()
                .map(|keyword| (*keyword).to_owned())
                .collect(),
            keyword_source: "manual".to_owned(),
            architecture: Some("flux".to_owned()),
            architecture_source: "metadata".to_owned(),
            created_at: 5,
            updated_at,
        }
    }

    #[tokio::test]
    async fn legacy_loras_are_written_once_and_newer_library_rows_are_kept() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-app-legacy-images-{}.sqlite3",
            LegacyImportRunId::new()
        ));
        let plan = plan();
        let inventory = LegacyDatabaseInventory {
            schema_version: 92,
            provider_accounts: 0,
            models: 0,
            prompts: 0,
            personas: 0,
            characters: 0,
            lorebooks: 0,
            chat_templates: 0,
            direct_conversations: 0,
            group_profiles: 0,
            group_conversations: 0,
        };
        let backend = AppBackend::open(&path, TimestampMillis::new(1)).expect("open backend");
        let kept = LoraRecord {
            path: "/loras/kept.safetensors".to_owned(),
            filename: "kept.safetensors".to_owned(),
            bytes_on_disk: 1,
            modified_at: 1,
            sha256: None,
            keywords: vec!["current".to_owned()],
            keyword_source: LoraKeywordSource::Manual,
            architecture: None,
            architecture_source: LoraArchitectureSource::None,
        };
        backend
            .database()
            .save_lora(&kept, TimestampMillis::new(10_000))
            .expect("current library row");
        let admission = backend
            .legacy_import_admission()
            .admit(
                LegacyImportRunId::new(),
                &inventory,
                &plan,
                TimestampMillis::new(10),
            )
            .expect("admit import");
        backend
            .legacy_import_executor()
            .execute(&admission, &plan, TimestampMillis::new(20))
            .expect("materialize authored graph");
        backend
            .legacy_provider_model_importer(&lettuce_settings::InMemorySecretStore::new())
            .execute(&admission, &plan, TimestampMillis::new(30))
            .await
            .expect("materialize providers");
        let images = LegacyBackupImagePlan {
            loras: vec![
                lora("/loras/style.safetensors", &["ink", "wash"], 9),
                lora("/loras/kept.safetensors", &["legacy"], 9),
            ],
            ..LegacyBackupImagePlan::default()
        };
        let receipt = backend
            .legacy_image_importer()
            .execute(&admission, &plan, &images, TimestampMillis::new(40))
            .expect("materialize images");
        assert_eq!(receipt.record_count, 2);
        assert!(!receipt.replayed);
        let style = backend
            .database()
            .lora("/loras/style.safetensors")
            .expect("read")
            .expect("imported lora");
        assert_eq!(style.keywords, vec!["ink".to_owned(), "wash".to_owned()]);
        assert_eq!(style.keyword_source, LoraKeywordSource::Manual);
        assert_eq!(style.architecture.as_deref(), Some("flux"));
        assert_eq!(style.architecture_source, LoraArchitectureSource::Metadata);
        assert_eq!(style.sha256, Some("ab".repeat(32)));
        assert_eq!(
            backend
                .database()
                .lora("/loras/kept.safetensors")
                .expect("read")
                .expect("kept lora")
                .keywords,
            vec!["current".to_owned()]
        );
        let replay = backend
            .legacy_image_importer()
            .execute(&admission, &plan, &images, TimestampMillis::new(50))
            .expect("replay");
        assert!(replay.replayed);
        drop(backend);
        let _ = std::fs::remove_file(path);
    }
}
