use lettuce_transfer::{
    LegacyBackupSettingsCandidate, LegacyImportAdmission, LegacyImportPlan, LegacyImportRepository,
    LegacyImportRepositoryError, LegacyImportStageReceipt, LegacySettingsMaterializationRequest,
};
use lettuce_types::TimestampMillis;

#[derive(Debug)]
pub struct LegacySettingsImportCoordinator<'a, R: ?Sized> {
    repository: &'a R,
}

impl<'a, R: LegacyImportRepository + ?Sized> LegacySettingsImportCoordinator<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    /// Writes the legacy global settings and their remapped feature model and
    /// prompt selections once the provider and prompt stage completed.
    pub fn execute(
        &self,
        admission: &LegacyImportAdmission,
        plan: &LegacyImportPlan,
        settings: &LegacyBackupSettingsCandidate,
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
            .materialize_settings(LegacySettingsMaterializationRequest {
                run_id: admission.run_id,
                plan_fingerprint,
                source_fingerprint,
                settings: settings.clone(),
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
            &import.compatibility.authored_plan().configuration.settings,
            completed_at,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use lettuce_context::{PromptEntryDraft, PromptEntryPosition, PromptEntryRole, PromptPurpose};
    use lettuce_settings::{GlobalSettings, GlobalSettingsStore, InMemorySecretStore};
    use lettuce_transfer::{
        DynamicMemoryPromptSources, HelpMeReplyPromptSources, LegacyAsrPlan,
        LegacyDatabaseInventory, LegacyImportAssignment, LegacyLorebookPlan, LegacyMediaPlan,
        LegacyPersonaPlan, LegacyPromptCandidate, LegacyPromptEntryCandidate, LegacyPromptPlan,
        LegacyProviderModelPlan, LorebookGeneratorPromptSources,
    };
    use lettuce_types::{ContentHash, LegacyImportRunId};

    use super::*;
    use crate::AppBackend;

    #[tokio::test]
    async fn legacy_settings_import_remaps_feature_prompts_and_replays() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-app-legacy-settings-{}.sqlite3",
            LegacyImportRunId::new()
        ));
        let summarizer_source = "legacy-summarizer".to_owned();
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
                prompts: vec![LegacyPromptCandidate {
                    source_id: summarizer_source.clone(),
                    name: "Imported Summarizer".to_owned(),
                    purpose: PromptPurpose::DynamicMemorySummarizer,
                    entries: vec![LegacyPromptEntryCandidate {
                        source_id: "summary-entry".to_owned(),
                        draft: PromptEntryDraft {
                            built_in_entry_key: None,
                            name: "Summary".to_owned(),
                            role: PromptEntryRole::System,
                            content: "Summarize the chat".to_owned(),
                            enabled: true,
                            injection_position: PromptEntryPosition::Relative,
                            depth: 0,
                            conditional_min_messages: None,
                            interval_turns: None,
                            system_prompt: true,
                            conditions: None,
                            payload: None,
                        },
                    }],
                    condense: false,
                    created_at: TimestampMillis::new(2),
                    updated_at: TimestampMillis::new(2),
                }],
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
            source_fingerprint: Some(ContentHash::parse("77".repeat(32)).expect("source hash")),
            later_skips: Vec::new(),
        };
        let inventory = LegacyDatabaseInventory {
            schema_version: 92,
            provider_accounts: 0,
            models: 0,
            prompts: 1,
            personas: 0,
            characters: 0,
            lorebooks: 0,
            chat_templates: 0,
            direct_conversations: 0,
            group_profiles: 0,
            group_conversations: 0,
        };
        let settings = LegacyBackupSettingsCandidate {
            value: GlobalSettings {
                analytics_enabled: false,
                manual_mode_context_window: 30,
                ..GlobalSettings::default()
            },
            default_provider_account_id: None,
            default_model_profile_id: None,
            default_prompt_source_id: None,
            dynamic_memory_model_profile_id: None,
            group_speaker_model_profile_id: None,
            lorebook_generator_model_profile_id: None,
            lorebook_generator_prompt_source_ids: LorebookGeneratorPromptSources::default(),
            dynamic_memory_prompt_source_ids: DynamicMemoryPromptSources {
                summarizer: Some(summarizer_source.clone()),
                manager: None,
            },
            help_me_reply_model_profile_id: None,
            help_me_reply_prompt_source_ids: HelpMeReplyPromptSources::default(),
            deprecated_system_prompt: None,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        let run_id = LegacyImportRunId::new();
        let secret_store = InMemorySecretStore::new();
        let backend = AppBackend::open(&path, TimestampMillis::new(1)).expect("open backend");
        let admission = backend
            .legacy_import_admission()
            .admit(run_id, &inventory, &plan, TimestampMillis::new(10))
            .expect("admit import");
        let prompt_id = admission
            .assignments
            .iter()
            .find_map(|assignment| match assignment {
                LegacyImportAssignment::Prompt {
                    legacy_id,
                    destination_id,
                } if legacy_id == &summarizer_source => Some(*destination_id),
                _ => None,
            })
            .expect("prompt assignment");
        backend
            .legacy_import_executor()
            .execute(&admission, &plan, TimestampMillis::new(20))
            .expect("materialize authored graph");
        backend
            .legacy_provider_model_importer(&secret_store)
            .execute(&admission, &plan, TimestampMillis::new(30))
            .await
            .expect("materialize providers");
        let before = GlobalSettingsStore::load(backend.database()).expect("settings before");

        let receipt = backend
            .legacy_settings_importer()
            .execute(&admission, &plan, &settings, TimestampMillis::new(40))
            .expect("materialize settings");

        assert_eq!(receipt.record_count, 1);
        assert!(!receipt.replayed);
        let stored = GlobalSettingsStore::load(backend.database()).expect("settings after");
        assert!(!stored.settings.analytics_enabled);
        assert_eq!(stored.settings.manual_mode_context_window, 30);
        assert_eq!(
            stored.settings.dynamic_memory_prompts.summarizer_prompt_id,
            Some(prompt_id)
        );
        assert_eq!(stored.revision, before.revision.next().expect("revision"));
        assert_eq!(stored.updated_at, TimestampMillis::new(40));
        assert_eq!(stored.created_at, TimestampMillis::new(1));

        let replay = backend
            .legacy_settings_importer()
            .execute(&admission, &plan, &settings, TimestampMillis::new(50))
            .expect("replay settings");
        assert!(replay.replayed);
        assert_eq!(replay.completed_at, TimestampMillis::new(40));
        assert_eq!(
            GlobalSettingsStore::load(backend.database())
                .expect("settings after replay")
                .revision,
            stored.revision
        );
        drop(backend);
        fs::remove_file(path).expect("remove database");
    }
}
