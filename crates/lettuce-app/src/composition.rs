use std::{path::Path, sync::Arc};

use lettuce_database::{Database, DatabaseError};
use lettuce_jobs::JobStore;

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
        Ok(Self {
            database: Arc::new(database),
            built_in_prompt_ids,
        })
    }

    #[must_use]
    pub fn database(&self) -> &Database {
        self.database.as_ref()
    }

    #[must_use]
    pub fn job_store(&self) -> &dyn JobStore {
        self.database.as_ref()
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
        crate::ProviderRuntime::new(Arc::clone(&self.database), secret_store, tls_policy)
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
