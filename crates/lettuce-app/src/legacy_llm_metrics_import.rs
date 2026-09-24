use lettuce_transfer::{
    LegacyBackupLlmMetricsPlan, LegacyIdScope, LegacyImportAdmission, LegacyImportPlan,
    LegacyImportRepository, LegacyImportRepositoryError, LegacyImportStageReceipt,
    LegacyLlmMetricImport, LegacyLlmMetricsMaterializationRequest,
};
use lettuce_types::{MessageId, TimestampMillis};

#[derive(Debug)]
pub struct LegacyLlmMetricsImportCoordinator<'a, R: ?Sized> {
    repository: &'a R,
}

impl<'a, R: LegacyImportRepository + ?Sized> LegacyLlmMetricsImportCoordinator<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    /// Writes the legacy local generation metrics of an admitted run after its
    /// conversations, naming for each row the imported message it was attached
    /// to (direct and group messages keep their legacy id within the run's
    /// scope).
    pub fn execute(
        &self,
        admission: &LegacyImportAdmission,
        plan: &LegacyImportPlan,
        metrics: &LegacyBackupLlmMetricsPlan,
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
        let scope = LegacyIdScope::new(&source_fingerprint);
        self.repository
            .materialize_llm_metrics(LegacyLlmMetricsMaterializationRequest {
                run_id: admission.run_id,
                plan_fingerprint,
                source_fingerprint,
                metrics: metrics
                    .metrics
                    .iter()
                    .map(|metric| LegacyLlmMetricImport {
                        message_id: metric
                            .message_source_id
                            .as_deref()
                            .map(|source| MessageId::from_uuid(scope.source(source))),
                        metric: metric.clone(),
                    })
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
            &import.compatibility.llm_metrics,
            completed_at,
        )
    }
}

#[cfg(test)]
mod tests {
    use lettuce_transfer::{
        LegacyAsrPlan, LegacyDatabaseInventory, LegacyLlmMetricRecord, LegacyLorebookPlan,
        LegacyMediaPlan, LegacyPersonaPlan, LegacyPromptPlan, LegacyProviderModelPlan,
    };
    use lettuce_types::{ContentHash, LegacyImportRunId};

    use super::*;
    use crate::AppBackend;

    #[tokio::test]
    async fn metrics_wait_for_both_conversation_stages() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-app-legacy-metrics-{}.sqlite3",
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
            source_fingerprint: Some(ContentHash::parse("97".repeat(32)).expect("source hash")),
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
        let metrics = LegacyBackupLlmMetricsPlan {
            metrics: vec![LegacyLlmMetricRecord {
                id: "gen-1".to_owned(),
                created_at: 5,
                model_path: None,
                summary_json: "{}".to_owned(),
                samples_json: "[]".to_owned(),
                message_source_id: None,
            }],
            ..LegacyBackupLlmMetricsPlan::default()
        };
        assert_eq!(
            backend.legacy_llm_metrics_importer().execute(
                &admission,
                &plan,
                &metrics,
                TimestampMillis::new(40)
            ),
            Err(LegacyImportRepositoryError::Conflict)
        );
        assert!(
            backend
                .database()
                .llm_generation_metrics(None)
                .expect("metrics")
                .is_empty()
        );
        drop(backend);
        let _ = std::fs::remove_file(path);
    }
}
