use lettuce_transfer::{
    LegacyBackupCreationHelperSession, LegacyBackupCreationMaterialization,
    LegacyCreationMaterializationRequest, LegacyImportAdmission, LegacyImportPlan,
    LegacyImportRepository, LegacyImportRepositoryError, LegacyImportStageReceipt,
};
use lettuce_types::TimestampMillis;

#[derive(Debug)]
pub struct LegacyCreationImportCoordinator<'a, R: ?Sized> {
    repository: &'a R,
}

impl<'a, R: LegacyImportRepository + ?Sized> LegacyCreationImportCoordinator<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    /// Seeds a creation workflow for every legacy creation helper session whose
    /// untouched draft fits the rewrite's draft; other sessions stay in sealed
    /// evidence because their chat, tool history or images have no destination.
    pub fn execute(
        &self,
        admission: &LegacyImportAdmission,
        plan: &LegacyImportPlan,
        sessions: &[LegacyBackupCreationHelperSession],
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
        self.repository
            .materialize_creation_helper(LegacyCreationMaterializationRequest {
                run_id: admission.run_id,
                plan_fingerprint,
                source_fingerprint,
                sessions: sessions
                    .iter()
                    .filter(|session| {
                        session.materialization
                            == LegacyBackupCreationMaterialization::InitialDraftSeed
                    })
                    .cloned()
                    .collect(),
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
            &import.compatibility.creation_helpers.sessions,
            completed_at,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use lettuce_creation::CreationWorkflowRepository;
    use lettuce_transfer::{
        LegacyAsrPlan, LegacyBackupCreationDraft, LegacyBackupCreationGoal,
        LegacyBackupCreationMode, LegacyBackupCreationScene, LegacyBackupCreationSessionState,
        LegacyBackupCreationStatus, LegacyDatabaseInventory, LegacyLorebookPlan, LegacyMediaPlan,
        LegacyPersonaPlan, LegacyPromptPlan, LegacyProviderModelPlan,
    };
    use lettuce_types::{ContentHash, CreationWorkflowId, LegacyImportRunId};

    use super::*;
    use crate::AppBackend;

    #[tokio::test]
    async fn untouched_legacy_creation_drafts_seed_workflows_once() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-app-legacy-creation-{}.sqlite3",
            LegacyImportRunId::new()
        ));
        let plan = LegacyImportPlan {
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
            source_fingerprint: Some(ContentHash::parse("aa".repeat(32)).expect("source hash")),
            later_skips: Vec::new(),
        };
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
        let seed = LegacyBackupCreationHelperSession {
            ordinal: 0,
            source_id: "creation-1".to_owned(),
            creation_goal: LegacyBackupCreationGoal::Character,
            status: LegacyBackupCreationStatus::Active,
            session_json: "{}".to_owned(),
            uploaded_images_json: "[]".to_owned(),
            session: LegacyBackupCreationSessionState {
                messages: Vec::new(),
                draft: LegacyBackupCreationDraft {
                    name: Some("Mira".to_owned()),
                    definition: Some("A lighthouse keeper".to_owned()),
                    description: None,
                    scenes: vec![LegacyBackupCreationScene {
                        source_id: "scene-1".to_owned(),
                        content: "The lamp flickers".to_owned(),
                        direction: None,
                    }],
                    default_scene_source_id: None,
                    avatar_locator: None,
                    background_locator: None,
                    disable_avatar_gradient: false,
                    default_model_source_id: None,
                    prompt_source_id: None,
                },
                draft_history: Vec::new(),
                creation_mode: LegacyBackupCreationMode::Create,
                target_type: None,
                target_source_id: None,
            },
            uploaded_images: Vec::new(),
            created_at: 5,
            updated_at: 6,
            materialization: LegacyBackupCreationMaterialization::InitialDraftSeed,
        };
        let evidence = LegacyBackupCreationHelperSession {
            source_id: "creation-2".to_owned(),
            materialization: LegacyBackupCreationMaterialization::RetainedEvidence,
            ..seed.clone()
        };
        let backend = AppBackend::open(&path, TimestampMillis::new(1)).expect("open backend");
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
        let sessions = vec![seed, evidence];

        let receipt = backend
            .legacy_creation_importer()
            .execute(&admission, &plan, &sessions, TimestampMillis::new(40))
            .expect("materialize creation workflows");
        assert_eq!(receipt.record_count, 1);
        let workflow_id = CreationWorkflowId::from_uuid(
            lettuce_transfer::LegacyIdScope::new(
                plan.source_fingerprint
                    .as_ref()
                    .expect("source fingerprint"),
            )
            .derived("creation-1", "creation-workflow"),
        );
        let workflow = CreationWorkflowRepository::load_workflow(backend.database(), workflow_id)
            .expect("seeded workflow");
        assert_eq!(workflow.created_at, TimestampMillis::new(5));
        let replay = backend
            .legacy_creation_importer()
            .execute(&admission, &plan, &sessions, TimestampMillis::new(50))
            .expect("replay creation workflows");
        assert!(replay.replayed);
        drop(backend);
        fs::remove_file(path).expect("remove database");
    }
}
