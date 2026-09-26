use std::sync::Arc;

use lettuce_app::api::{self, ApiContext, JobEventSink};
use lettuce_contracts::{
    ApiError, JobCancelRequest, JobEvent, JobGetRequest, JobPage, JobView, JobWatchRequest,
    JobsListRequest,
};
use tauri::{State, ipc::Channel};

#[tauri::command]
#[specta::specta]
pub async fn jobs_list(
    context: State<'_, ApiContext>,
    request: JobsListRequest,
) -> Result<JobPage, ApiError> {
    api::jobs_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn job_get(
    context: State<'_, ApiContext>,
    request: JobGetRequest,
) -> Result<JobView, ApiError> {
    api::job_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn job_cancel(
    context: State<'_, ApiContext>,
    request: JobCancelRequest,
) -> Result<(), ApiError> {
    api::job_cancel(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn job_watch(
    context: State<'_, ApiContext>,
    request: JobWatchRequest,
    on_event: Channel<JobEvent>,
) -> Result<JobView, ApiError> {
    api::job_watch(&context, request, Arc::new(ChannelSink(on_event))).await
}

/// A watched job's stream, delivered over the command's IPC channel.
struct ChannelSink(Channel<JobEvent>);

impl JobEventSink for ChannelSink {
    fn emit(&self, event: JobEvent) {
        if let Err(error) = self.0.send(event) {
            tracing::debug!(%error, "job channel is closed");
        }
    }
}
