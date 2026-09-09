use std::{path::Path, sync::Arc};

use lettuce_database::{Database, DatabaseError};
use lettuce_inference::InferenceRuntime;
use lettuce_jobs::JobStore;
use lettuce_speech::WhisperCppRuntime;

use crate::{
    BuiltInPromptIds, BuiltInPromptService, BuiltInPromptServiceError, ConversationLaunchError,
    ConversationLaunchPlanner, DirectConversationLaunchRequest, GroupConversationLaunchRequest,
};

/// The application composition root. Opening an application database through
/// this type always applies migrations and reconciles the bundled prompt
/// catalog before any caller can use the database.
#[derive(Debug)]
pub struct AppBackend {
    database: Arc<Database>,
    built_in_prompt_ids: BuiltInPromptIds,
    inference_runtime: Arc<InferenceRuntime>,
    whisper_runtime: Arc<WhisperCppRuntime<Database>>,
}

impl AppBackend {
    pub fn open(
        path: impl AsRef<Path>,
        now: lettuce_types::TimestampMillis,
    ) -> Result<Self, AppInitializationError> {
        let database = Database::open(path).map_err(AppInitializationError::StorageUnavailable)?;
        Self::finish_open(database, now)
    }

    pub fn open_in_memory(
        now: lettuce_types::TimestampMillis,
    ) -> Result<Self, AppInitializationError> {
        let database =
            Database::open_in_memory().map_err(AppInitializationError::StorageUnavailable)?;
        Self::finish_open(database, now)
    }

    fn finish_open(
        database: Database,
        now: lettuce_types::TimestampMillis,
    ) -> Result<Self, AppInitializationError> {
        let built_in_prompt_ids = BuiltInPromptService::new(&database)
            .map_err(AppInitializationError::BuiltInPrompts)?
            .bootstrap(now)
            .map_err(AppInitializationError::BuiltInPrompts)?;
        let database = Arc::new(database);
        Ok(Self {
            whisper_runtime: Arc::new(WhisperCppRuntime::new(database.clone())),
            database,
            built_in_prompt_ids,
            inference_runtime: Arc::new(InferenceRuntime::default()),
        })
    }

    #[must_use]
    pub fn database(&self) -> &Database {
        self.database.as_ref()
    }

    pub fn preflight_legacy_database(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<
        lettuce_transfer::LegacyDatabaseInventory,
        lettuce_transfer::LegacyDatabasePreflightError,
    > {
        lettuce_database::preflight_legacy_database(path)
    }

    pub fn plan_legacy_personas(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<lettuce_transfer::LegacyPersonaPlan, lettuce_transfer::LegacyDatabasePreflightError>
    {
        lettuce_database::plan_legacy_personas(path)
    }

    pub fn plan_legacy_lorebooks(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<lettuce_transfer::LegacyLorebookPlan, lettuce_transfer::LegacyDatabasePreflightError>
    {
        lettuce_database::plan_legacy_lorebooks(path)
    }

    pub fn plan_legacy_provider_models(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<
        lettuce_transfer::LegacyProviderModelPlan,
        lettuce_transfer::LegacyDatabasePreflightError,
    > {
        lettuce_database::plan_legacy_provider_models(path)
    }

    pub fn plan_legacy_prompts(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<lettuce_transfer::LegacyPromptPlan, lettuce_transfer::LegacyDatabasePreflightError>
    {
        lettuce_database::plan_legacy_prompts(path)
    }

    pub fn plan_legacy_media(
        &self,
        storage_root: impl AsRef<Path>,
        personas: &lettuce_transfer::LegacyPersonaPlan,
        lorebooks: &lettuce_transfer::LegacyLorebookPlan,
        asr: &lettuce_transfer::LegacyAsrPlan,
    ) -> Result<lettuce_transfer::LegacyMediaPlan, lettuce_transfer::LegacyDatabasePreflightError>
    {
        crate::plan_legacy_media(storage_root, personas, lorebooks, asr)
    }

    #[must_use]
    pub fn legacy_import_admission(&self) -> crate::LegacyImportAdmissionCoordinator<'_, Database> {
        crate::LegacyImportAdmissionCoordinator::new(self.database.as_ref())
    }

    #[must_use]
    pub fn legacy_import_executor(&self) -> crate::LegacyImportExecutionCoordinator<'_, Database> {
        crate::LegacyImportExecutionCoordinator::new(self.database.as_ref())
    }

    #[must_use]
    pub fn legacy_asr_importer(&self) -> crate::LegacyAsrImportCoordinator<'_, Database> {
        crate::LegacyAsrImportCoordinator::new(self.database.as_ref())
    }

    pub fn legacy_provider_secret_importer<'a, S, V>(
        &'a self,
        source: &'a S,
        secret_store: &'a V,
    ) -> crate::LegacyProviderSecretImportCoordinator<'a, Database, S, V>
    where
        S: lettuce_transfer::LegacyProviderSecretSource + ?Sized,
        V: lettuce_settings::SecretStore + ?Sized,
    {
        crate::LegacyProviderSecretImportCoordinator::new(
            self.database.as_ref(),
            source,
            secret_store,
        )
    }

    pub fn legacy_provider_model_importer<'a, V>(
        &'a self,
        secret_store: &'a V,
    ) -> crate::LegacyProviderModelImportCoordinator<'a, Database, V>
    where
        V: lettuce_settings::SecretStore + ?Sized,
    {
        crate::LegacyProviderModelImportCoordinator::new(self.database.as_ref(), secret_store)
    }

    pub fn legacy_media_importer<'a, BR, AR>(
        &'a self,
        media_store: &'a lettuce_media::LocalMediaBlobStore<BR, AR>,
    ) -> crate::LegacyMediaImportCoordinator<'a, Database, BR, AR>
    where
        BR: lettuce_media::MediaBlobRepository,
        AR: lettuce_media::MediaAssetRepository,
    {
        crate::LegacyMediaImportCoordinator::new(self.database.as_ref(), media_store)
    }

    pub fn legacy_asr_learning_transfer<'a, BR, AR>(
        &'a self,
        media_store: &'a lettuce_media::LocalMediaBlobStore<BR, AR>,
    ) -> crate::LegacyAsrLearningTransferCoordinator<'a, Database, BR, AR>
    where
        BR: lettuce_media::MediaBlobRepository,
        AR: lettuce_media::MediaAssetRepository,
    {
        crate::LegacyAsrLearningTransferCoordinator::new(self.database.as_ref(), media_store)
    }

    #[must_use]
    pub fn job_store(&self) -> &dyn JobStore {
        self.database.as_ref()
    }

    pub fn usage_costs<'a, P: crate::OpenRouterBillingPort + ?Sized>(
        &'a self,
        provider: &'a P,
    ) -> crate::UsageCostCoordinator<'a, Database, P> {
        crate::UsageCostCoordinator::new(self.database.as_ref(), provider)
    }

    pub fn tts_configuration<'a, S: lettuce_settings::SecretStore + ?Sized>(
        &'a self,
        secret_store: &'a S,
    ) -> crate::TtsConfigurationCoordinator<'a, Database, S> {
        crate::TtsConfigurationCoordinator::new(self.database.as_ref(), secret_store)
    }

    #[must_use]
    pub fn tts_syntheses(&self) -> crate::TtsSynthesisCoordinator<'_, Database, Database> {
        crate::TtsSynthesisCoordinator::new(self.database.as_ref(), self.database.as_ref())
    }

    pub fn remote_tts_runtime(
        &self,
        tls_policy: &lettuce_network::TlsPolicy,
    ) -> Result<lettuce_speech::RemoteTtsRuntime, lettuce_network::JsonClientError> {
        let network = Arc::new(lettuce_network::JsonClient::with_tls(tls_policy)?);
        Ok(lettuce_speech::RemoteTtsRuntime::new(network))
    }

    pub fn tts_voice_refresh<'a, S: lettuce_settings::SecretStore + ?Sized>(
        &'a self,
        secrets: &'a S,
    ) -> crate::TtsVoiceRefreshCoordinator<'a, Database, S> {
        crate::TtsVoiceRefreshCoordinator::new(self.database.as_ref(), secrets)
    }

    pub fn tts_voice_design<'a, S: lettuce_settings::SecretStore + ?Sized>(
        &'a self,
        secrets: &'a S,
    ) -> crate::TtsVoiceDesignCoordinator<'a, Database, S> {
        crate::TtsVoiceDesignCoordinator::new(self.database.as_ref(), secrets)
    }

    pub fn tts_provider_verification<'a, S: lettuce_settings::SecretStore + ?Sized>(
        &'a self,
        secrets: &'a S,
    ) -> crate::TtsProviderVerificationCoordinator<'a, Database, S> {
        crate::TtsProviderVerificationCoordinator::new(self.database.as_ref(), secrets)
    }

    #[must_use]
    pub fn speech_transcriptions(
        &self,
    ) -> crate::SpeechTranscriptionCoordinator<'_, Database, Database> {
        crate::SpeechTranscriptionCoordinator::new(self.database.as_ref(), self.database.as_ref())
    }

    #[must_use]
    pub fn whisper_models(&self) -> crate::WhisperModelCoordinator<'_, Database> {
        crate::WhisperModelCoordinator::new(self.database.as_ref())
    }

    pub fn whisper_remote_catalog(
        &self,
    ) -> Result<crate::WhisperRemoteCatalog, lettuce_network::JsonClientError> {
        lettuce_network::JsonClient::new().map(crate::WhisperRemoteCatalog::new)
    }

    pub fn kokoro_remote_voice_catalog(
        &self,
    ) -> Result<crate::KokoroRemoteVoiceCatalog, lettuce_network::JsonClientError> {
        lettuce_network::JsonClient::new().map(crate::KokoroRemoteVoiceCatalog::new)
    }

    pub fn whisper_downloads(
        &self,
        install_root: impl AsRef<Path>,
    ) -> Result<crate::WhisperDownloadCoordinator<'_, Database, Database>, crate::WhisperDownloadError>
    {
        crate::WhisperDownloadCoordinator::new(
            self.database.as_ref(),
            self.database.as_ref(),
            install_root,
        )
    }

    #[must_use]
    pub fn kokoro_downloads(
        &self,
        installs: lettuce_model_hub::KokoroInstallStore,
    ) -> crate::KokoroDownloadCoordinator<'_, Database> {
        crate::KokoroDownloadCoordinator::new(self.database.as_ref(), installs)
    }

    #[must_use]
    pub fn kokoro_voice_downloads(
        &self,
        installs: lettuce_model_hub::KokoroVoiceInstallStore,
    ) -> crate::KokoroVoiceDownloadCoordinator<'_, Database> {
        crate::KokoroVoiceDownloadCoordinator::new(self.database.as_ref(), installs)
    }

    pub fn remove_managed_whisper_model(
        &self,
        install_root: impl AsRef<Path>,
        model_id: &str,
    ) -> Result<crate::WhisperModelRemoval, crate::WhisperModelCoordinatorError> {
        let installs = lettuce_model_hub::WhisperInstallStore::open(install_root)?;
        self.whisper_models()
            .remove_managed(&installs, self.whisper_runtime(), model_id)
    }

    #[must_use]
    pub fn whisper_runtime(&self) -> &WhisperCppRuntime<Database> {
        self.whisper_runtime.as_ref()
    }

    #[must_use]
    pub fn asr_learning(&self) -> lettuce_speech::AsrLearningLibrary<'_, Database> {
        lettuce_speech::AsrLearningLibrary::new(self.database.as_ref())
    }

    #[must_use]
    pub fn asr_learning_transfer(&self) -> crate::AsrLearningTransferCoordinator<'_, Database> {
        crate::AsrLearningTransferCoordinator::new(self.database.as_ref())
    }

    pub fn run_speech_transcription<A: lettuce_speech::AsrAudioSource + ?Sized>(
        &self,
        work: crate::SpeechTranscriptionClaimedWork,
        audio: &A,
        cancellation_reason: lettuce_jobs::CancellationReason,
        now: lettuce_types::TimestampMillis,
    ) -> Result<crate::SpeechTranscriptionRunResult, crate::SpeechTranscriptionError> {
        self.speech_transcriptions().run(
            work,
            audio,
            &self.asr_learning(),
            self.whisper_runtime(),
            cancellation_reason,
            now,
        )
    }

    #[must_use]
    pub fn startup_job_recovery(&self) -> crate::StartupJobRecoveryCoordinator<'_, Database> {
        crate::StartupJobRecoveryCoordinator::new(self.database.as_ref())
    }

    #[must_use]
    pub fn companion_memory_dispatcher(
        &self,
    ) -> crate::CompanionMemoryDispatchCoordinator<'_, Database, Database> {
        crate::CompanionMemoryDispatchCoordinator::new(
            self.database.as_ref(),
            self.database.as_ref(),
        )
    }

    #[must_use]
    pub fn conversation_generation_dispatcher(
        &self,
    ) -> crate::ConversationGenerationDispatchCoordinator<'_, Database, Database> {
        crate::ConversationGenerationDispatchCoordinator::new(
            self.database.as_ref(),
            self.database.as_ref(),
        )
    }

    #[must_use]
    pub fn conversation_generation_cancellation(
        &self,
    ) -> crate::ConversationGenerationCancellationCoordinator<'_, Database, Database> {
        crate::ConversationGenerationCancellationCoordinator::new(
            self.database.as_ref(),
            self.database.as_ref(),
            self.inference_runtime.as_ref(),
        )
    }

    pub fn prepared_conversation_generation_runner<'a, E: ?Sized, I: ?Sized>(
        &'a self,
        embedding: &'a E,
        inference: &'a I,
    ) -> crate::PreparedConversationGenerationJobRunner<'a, E, Database, I> {
        crate::PreparedConversationGenerationJobRunner::new(
            embedding,
            self.database.as_ref(),
            inference,
        )
        .with_inference_runtime(self.inference_runtime.as_ref())
    }

    #[must_use]
    pub fn companion_growth_admission(
        &self,
    ) -> crate::CompanionGrowthJobAdmissionCoordinator<'_, Database, Database> {
        crate::CompanionGrowthJobAdmissionCoordinator::new(
            self.database.as_ref(),
            self.database.as_ref(),
        )
    }

    #[must_use]
    pub fn companion_growth_dispatcher(
        &self,
    ) -> crate::CompanionGrowthDispatchCoordinator<'_, Database, Database> {
        crate::CompanionGrowthDispatchCoordinator::new(
            self.database.as_ref(),
            self.database.as_ref(),
        )
    }

    #[must_use]
    pub fn companion_consolidation_admission(
        &self,
    ) -> crate::CompanionConsolidationJobAdmissionCoordinator<'_, Database, Database> {
        crate::CompanionConsolidationJobAdmissionCoordinator::new(
            self.database.as_ref(),
            self.database.as_ref(),
        )
    }

    #[must_use]
    pub fn companion_consolidation_dispatcher(
        &self,
    ) -> crate::CompanionConsolidationDispatchCoordinator<'_, Database, Database> {
        crate::CompanionConsolidationDispatchCoordinator::new(
            self.database.as_ref(),
            self.database.as_ref(),
        )
    }

    #[must_use]
    pub fn companion_soul_writer_admission(
        &self,
    ) -> crate::CompanionSoulWriterAdmissionCoordinator<'_, Database, Database> {
        crate::CompanionSoulWriterAdmissionCoordinator::new(
            self.database.as_ref(),
            self.database.as_ref(),
        )
    }

    #[must_use]
    pub fn companion_soul_writer_dispatcher(
        &self,
    ) -> crate::CompanionSoulWriterDispatchCoordinator<'_, Database, Database> {
        crate::CompanionSoulWriterDispatchCoordinator::new(
            self.database.as_ref(),
            self.database.as_ref(),
        )
    }

    #[must_use]
    pub fn lorebook_entry_preparation(
        &self,
    ) -> crate::LorebookEntryPreparationCoordinator<'_, Database, Database> {
        crate::LorebookEntryPreparationCoordinator::new(
            self.database.as_ref(),
            self.database.as_ref(),
        )
    }

    #[must_use]
    pub fn lorebook_entry_dispatcher(
        &self,
    ) -> crate::LorebookEntryDispatchCoordinator<'_, Database, Database> {
        crate::LorebookEntryDispatchCoordinator::new(self.database.as_ref(), self.database.as_ref())
    }

    #[must_use]
    pub fn lorebook_keyword_coordinator(
        &self,
    ) -> crate::LorebookKeywordCoordinator<'_, Database, Database> {
        crate::LorebookKeywordCoordinator::new(self.database.as_ref(), self.database.as_ref())
    }

    #[must_use]
    pub fn lorebook_keyword_dispatcher(
        &self,
    ) -> crate::LorebookKeywordDispatchCoordinator<'_, Database, Database> {
        crate::LorebookKeywordDispatchCoordinator::new(
            self.database.as_ref(),
            self.database.as_ref(),
        )
    }

    #[must_use]
    pub fn staged_lorebook_coordinator(
        &self,
    ) -> crate::StagedLorebookCoordinator<'_, Database, Database> {
        crate::StagedLorebookCoordinator::new(self.database.as_ref(), self.database.as_ref())
    }

    #[must_use]
    pub fn staged_lorebook_planner_dispatcher(
        &self,
    ) -> crate::StagedLorebookPlannerDispatchCoordinator<'_, Database, Database> {
        crate::StagedLorebookPlannerDispatchCoordinator::new(
            self.database.as_ref(),
            self.database.as_ref(),
        )
    }

    #[must_use]
    pub fn staged_lorebook_writer_coordinator(
        &self,
    ) -> crate::StagedLorebookWriterCoordinator<'_, Database, Database, Database> {
        crate::StagedLorebookWriterCoordinator::new(
            self.database.as_ref(),
            self.database.as_ref(),
            self.database.as_ref(),
        )
    }

    #[must_use]
    pub fn staged_lorebook_writer_dispatcher(
        &self,
    ) -> crate::StagedLorebookWriterDispatchCoordinator<'_, Database, Database, Database> {
        crate::StagedLorebookWriterDispatchCoordinator::new(
            self.database.as_ref(),
            self.database.as_ref(),
            self.database.as_ref(),
        )
    }

    #[must_use]
    pub fn staged_lorebook_coherence_dispatcher(
        &self,
    ) -> crate::StagedLorebookCoherenceDispatchCoordinator<'_, Database, Database> {
        crate::StagedLorebookCoherenceDispatchCoordinator::new(
            self.database.as_ref(),
            self.database.as_ref(),
        )
    }

    #[must_use]
    pub const fn built_in_prompt_ids(&self) -> &BuiltInPromptIds {
        &self.built_in_prompt_ids
    }

    #[must_use]
    pub fn conversation_launch_planner(&self) -> ConversationLaunchPlanner<'_, Database> {
        ConversationLaunchPlanner::new(&self.database)
    }

    #[must_use]
    pub fn conversation_context_assembler(
        &self,
    ) -> crate::ConversationContextAssembler<'_, Database> {
        crate::ConversationContextAssembler::new(self.database.as_ref())
    }

    #[must_use]
    pub fn dynamic_memory_handler(&self) -> crate::DynamicMemoryHandler<'_, Database> {
        crate::DynamicMemoryHandler::new(self.database.as_ref())
    }

    pub fn launch_direct_conversation(
        &self,
        request: &DirectConversationLaunchRequest,
        now: lettuce_types::TimestampMillis,
    ) -> Result<lettuce_conversations::CreateConversationResult, ConversationLaunchError> {
        self.conversation_launch_planner()
            .launch_direct(request, now)
    }

    pub fn launch_group_conversation(
        &self,
        request: &GroupConversationLaunchRequest,
        now: lettuce_types::TimestampMillis,
    ) -> Result<lettuce_conversations::CreateConversationResult, ConversationLaunchError> {
        self.conversation_launch_planner()
            .launch_group(request, now)
    }

    /// Builds the reusable remote-provider application service with the
    /// host's real secret backend and current TLS trust policy. No in-memory
    /// credential fallback is created here.
    pub fn provider_runtime<S: lettuce_settings::SecretStore + ?Sized>(
        &self,
        secret_store: Arc<S>,
        tls_policy: &lettuce_network::TlsPolicy,
    ) -> Result<crate::ProviderRuntime<S>, crate::ProviderRuntimeInitializationError> {
        crate::ProviderRuntime::with_inference_runtime(
            Arc::clone(&self.database),
            secret_store,
            tls_policy,
            Arc::clone(&self.inference_runtime),
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AppInitializationError {
    #[error("application storage is unavailable: {0}")]
    StorageUnavailable(DatabaseError),
    #[error("built-in prompt initialization failed: {0}")]
    BuiltInPrompts(BuiltInPromptServiceError),
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{BuiltInPromptId, MAX_COMPANION_POST_TURN_EFFECTS};
    use lettuce_context::PromptRepository;
    use lettuce_jobs::{ResourceAvailability, WorkerId};
    use lettuce_memory::DynamicMemoryRunMode;
    use lettuce_types::TimestampMillis;

    #[test]
    fn first_open_bootstraps_and_reopen_keeps_stable_database_ids() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-app-bootstrap-{}.sqlite3",
            lettuce_types::OperationId::new()
        ));
        let first = AppBackend::open(&path, TimestampMillis::new(1)).expect("first open");
        let first_id = first.built_in_prompt_ids().get(BuiltInPromptId::AppDefault);
        assert!(
            PromptRepository::get(first.database(), first_id)
                .expect("read default")
                .is_some()
        );
        drop(first);

        let reopened = AppBackend::open(&path, TimestampMillis::new(2)).expect("reopen");
        assert_eq!(
            reopened
                .built_in_prompt_ids()
                .get(BuiltInPromptId::AppDefault),
            first_id
        );
        drop(reopened);
        std::fs::remove_file(&path).expect("remove test database");
    }

    #[test]
    fn in_memory_open_is_fully_initialized() {
        let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("open");
        assert!(
            backend
                .job_store()
                .list(lettuce_jobs::JobQuery::default())
                .expect("list jobs")
                .items
                .is_empty()
        );
        assert!(
            backend
                .companion_memory_dispatcher()
                .discover_and_claim(
                    MAX_COMPANION_POST_TURN_EFFECTS,
                    1,
                    DynamicMemoryRunMode::Auto,
                    WorkerId::new(),
                    TimestampMillis::new(2),
                    Duration::from_secs(60),
                    &ResourceAvailability::all(),
                )
                .expect("discover jobs")
                .is_empty()
        );
        assert!(
            PromptRepository::get(
                backend.database(),
                backend
                    .built_in_prompt_ids()
                    .get(BuiltInPromptId::Companion)
            )
            .expect("read")
            .is_some()
        );
    }
}
