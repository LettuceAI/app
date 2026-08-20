use std::path::Path;

use lettuce_database::{Database, DatabaseError};

use crate::{
    BuiltInPromptIds, BuiltInPromptService, BuiltInPromptServiceError, ConversationLaunchPlanner,
    DirectConversationLaunchRequest, DirectLaunchError,
};

/// The application composition root. Opening an application database through
/// this type always applies migrations and reconciles the bundled prompt
/// catalog before any caller can use the database.
#[derive(Debug)]
pub struct AppBackend {
    database: Database,
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
            database,
            built_in_prompt_ids,
        })
    }

    #[must_use]
    pub const fn database(&self) -> &Database {
        &self.database
    }

    #[must_use]
    pub const fn built_in_prompt_ids(&self) -> &BuiltInPromptIds {
        &self.built_in_prompt_ids
    }

    #[must_use]
    pub const fn conversation_launch_planner(&self) -> ConversationLaunchPlanner<'_, Database> {
        ConversationLaunchPlanner::new(&self.database)
    }

    pub fn launch_direct_conversation(
        &self,
        request: &DirectConversationLaunchRequest,
        now: lettuce_types::TimestampMillis,
    ) -> Result<lettuce_conversations::CreateConversationResult, DirectLaunchError> {
        self.conversation_launch_planner()
            .launch_direct(request, now)
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
    use super::*;
    use crate::BuiltInPromptId;
    use lettuce_context::PromptRepository;
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
