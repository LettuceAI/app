use lettuce_app::api::ApiEventSink;
use lettuce_contracts::ApiEvent;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
use tauri_specta::Event;

/// The application-wide event every window receives.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct AppEvent(pub ApiEvent);

pub(crate) struct TauriEventSink<R: Runtime>(pub(crate) AppHandle<R>);

impl<R: Runtime> ApiEventSink for TauriEventSink<R> {
    fn emit(&self, event: ApiEvent) {
        if let Err(error) = AppEvent(event).emit(&self.0) {
            tracing::warn!(%error, "application event could not be emitted");
        }
    }
}
