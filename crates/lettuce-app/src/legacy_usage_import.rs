use lettuce_transfer::{
    LegacyBackupUsageRecord, LegacyImportAdmission, LegacyImportPlan, LegacyImportRepository,
    LegacyImportRepositoryError, LegacyImportStageReceipt, LegacyUsageMaterializationRequest,
};
use lettuce_types::TimestampMillis;

#[derive(Debug)]
pub struct LegacyUsageImportCoordinator<'a, R: ?Sized> {
    repository: &'a R,
}

impl<'a, R: LegacyImportRepository + ?Sized> LegacyUsageImportCoordinator<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    /// Writes every legacy usage record as the historical row legacy stored,
    /// keeping its source ids and linking the imported model when it exists.
    pub fn execute(
        &self,
        admission: &LegacyImportAdmission,
        plan: &LegacyImportPlan,
        records: &[LegacyBackupUsageRecord],
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
            .materialize_usage_records(LegacyUsageMaterializationRequest {
                run_id: admission.run_id,
                plan_fingerprint,
                source_fingerprint,
                records: records.to_vec(),
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
            &import.compatibility.usage_records().records,
            completed_at,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use lettuce_transfer::{
        LegacyAsrPlan, LegacyBackupUsageAggregation, LegacyDatabaseInventory, LegacyLorebookPlan,
        LegacyMediaPlan, LegacyPersonaPlan, LegacyPromptPlan, LegacyProviderModelPlan,
    };
    use lettuce_types::{ContentHash, LegacyImportRunId};

    use super::*;
    use crate::AppBackend;

    #[tokio::test]
    async fn legacy_usage_records_are_written_once_and_replay() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-app-legacy-usage-{}.sqlite3",
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
            source_fingerprint: Some(ContentHash::parse("99".repeat(32)).expect("source hash")),
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
        let record = LegacyBackupUsageRecord {
            source_id: "usage-1".to_owned(),
            timestamp: 42,
            session_id: "session-1".to_owned(),
            character_id: "character-1".to_owned(),
            character_name: "Mira".to_owned(),
            model_id: "model-1".to_owned(),
            model_name: "Example".to_owned(),
            provider_id: "openai".to_owned(),
            provider_label: "Primary".to_owned(),
            operation_type: Some("chat".to_owned()),
            finish_reason: Some("stop".to_owned()),
            prompt_tokens: Some(12),
            completion_tokens: Some(4),
            total_tokens: Some(16),
            memory_tokens: None,
            summary_tokens: None,
            reasoning_tokens: None,
            image_tokens: None,
            audio_tokens: None,
            prompt_cost: Some(0.001),
            completion_cost: Some(0.002),
            total_cost: Some(0.003),
            success: true,
            error_message: None,
            metadata: BTreeMap::from([("route".to_owned(), "direct".to_owned())]),
            aggregation: LegacyBackupUsageAggregation::HistoricalOnly,
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

        let receipt = backend
            .legacy_usage_importer()
            .execute(
                &admission,
                &plan,
                std::slice::from_ref(&record),
                TimestampMillis::new(40),
            )
            .expect("materialize usage records");
        assert_eq!(receipt.record_count, 1);
        assert!(!receipt.replayed);
        let replay = backend
            .legacy_usage_importer()
            .execute(
                &admission,
                &plan,
                std::slice::from_ref(&record),
                TimestampMillis::new(50),
            )
            .expect("replay usage records");
        assert!(replay.replayed);
        assert_eq!(replay.completed_at, TimestampMillis::new(40));
        let reimport = backend
            .legacy_import_admission()
            .admit(
                LegacyImportRunId::new(),
                &inventory,
                &plan,
                TimestampMillis::new(60),
            )
            .expect("importing the same legacy source again");
        assert_eq!(reimport.run_id, admission.run_id);
        assert!(reimport.replayed);
        drop(backend);
        fs::remove_file(path).expect("remove database");
    }
}
