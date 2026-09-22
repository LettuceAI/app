//! Playground history: the app's own playground generations and the entries
//! imported from the old app, listed and deleted like the old playground.

use lettuce_types::{AssetId, JobId, ModelProfileId, TimestampMillis};
use serde::Serialize;

/// The old playground listed 30 entries unless asked otherwise, 1 to 200.
pub const PLAYGROUND_DEFAULT_PAGE: u32 = 30;
pub const PLAYGROUND_MAX_PAGE: u32 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaygroundOrigin {
    Generated,
    Imported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaygroundHistoryImage {
    pub asset_id: Option<AssetId>,
    /// The old app's asset id of an imported image.
    pub source_asset_id: Option<String>,
    pub mime_type: Option<String>,
    pub url: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaygroundHistoryEntry {
    pub id: String,
    pub origin: PlaygroundOrigin,
    pub job_id: Option<JobId>,
    pub created_at: TimestampMillis,
    pub provider_kind: String,
    /// The old app's model id of an imported entry.
    pub source_model_id: Option<String>,
    pub model_profile_id: Option<ModelProfileId>,
    pub model_name: String,
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub seed: Option<i64>,
    /// The generation parameters as JSON (the old app's own object for
    /// imported entries, verbatim).
    pub params_json: String,
    /// `pending`, `complete`, `failed` or `cancelled`, like the old playground.
    pub status: String,
    pub error: Option<String>,
    pub images: Vec<PlaygroundHistoryImage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PlaygroundHistoryError {
    #[error("playground history entry not found")]
    NotFound,
    #[error("playground history is unavailable")]
    Storage,
}

/// The clamped page size for a requested limit.
#[must_use]
pub fn playground_page_size(limit: Option<u32>) -> u32 {
    limit
        .unwrap_or(PLAYGROUND_DEFAULT_PAGE)
        .clamp(1, PLAYGROUND_MAX_PAGE)
}

pub trait PlaygroundHistoryRepository: Send + Sync {
    /// Newest first; `before` keeps entries created strictly earlier.
    fn list_playground_history(
        &self,
        limit: u32,
        before: Option<TimestampMillis>,
    ) -> Result<Vec<PlaygroundHistoryEntry>, PlaygroundHistoryError>;

    /// Removes the entry (a missing one is already gone). With
    /// `delete_images` its image assets are deleted too, like the old app
    /// deleted the files, unless the library keeps them or something else
    /// still uses them; returns the assets deleted.
    fn delete_playground_history(
        &self,
        id: &str,
        delete_images: bool,
    ) -> Result<Vec<AssetId>, PlaygroundHistoryError>;
}
