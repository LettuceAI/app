//! The application API the desktop shell exposes as commands. Every call is
//! a plain async function over an `ApiContext` that takes and returns
//! `lettuce-contracts` DTOs and fails with an `ApiError`; repository work
//! runs on the blocking pool.

mod app;
mod assets;
mod characters;
mod context;
mod conversation_feed;
mod conversations;
mod error;
mod events;
mod file_kind;
mod files;
mod jobs;
mod mapping;
mod models;
mod startup;
mod worker;

#[cfg(test)]
mod conversation_list_tests;
#[cfg(test)]
mod foundation_tests;
#[cfg(test)]
mod models_tests;
#[cfg(test)]
mod tests;

pub use app::{app_status, app_ui_state_update, purge_notice_dismiss, purge_notices_list};
pub use assets::{AssetBytes, AssetRange, AssetRead, OPEN_RANGE_CHUNK, read_asset};
pub use characters::characters_list;
pub use context::{ApiContext, ApiContextParts, ApiDatabaseFiles, ApiMediaStore};
pub use conversations::{
    conversation_launch_direct, conversation_launch_group, conversation_messages,
    conversation_open, conversation_send, conversations_latest_by_character,
    conversations_latest_by_group, conversations_list, generation_cancel,
};
pub use events::{ApiEventSink, GenerationEventSink, JobEventSink};
pub use files::{
    FileAccess, FileAccessError, FileDescription, FileReader, assets_ingest, files_inspect,
};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use jobs::CatalogVariant;
pub use jobs::{
    ArtifactInstallHandler, ClaimedJob, InstallFinish, InstallSources, InstallWork, JobHandler,
    JobHandlers, JobLane, JobProgressSink, JobRunner, NetworkInstallSources, admit_install,
    job_cancel, job_get, job_watch, jobs_list,
};
pub use models::{InstalledModels, ModelLoad, ModelLoader, NoModels};
pub use startup::{ApiWorkers, StartupStep, startup};
pub use worker::ConversationGenerationWorker;
