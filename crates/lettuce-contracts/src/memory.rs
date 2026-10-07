use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum MemoryCategory {
    CharacterTrait,
    Relationship,
    PlotEvent,
    Preference,
    WorldDetail,
    Other,
    Milestone,
    Boundary,
    Profile,
    Routine,
    Episodic,
    EmotionalSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum MemoryTemperature {
    Hot,
    Cold,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum MemoryOrigin {
    User,
    Model,
    Import,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum MemoryCategoryChange {
    #[default]
    Keep,
    Set(Option<MemoryCategory>),
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum MemoryObservedAtChange {
    #[default]
    Keep,
    Set(Option<i64>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum MemorySummaryEdit {
    Set { text: String },
    Clear,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemoryAddRequest {
    pub conversation_id: String,
    pub text: String,
    pub category: Option<MemoryCategory>,
    pub observed_at: Option<i64>,
    pub expected_revision: u64,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemoryUpdateRequest {
    pub conversation_id: String,
    pub memory_id: String,
    pub text: Option<String>,
    #[serde(default)]
    pub category: MemoryCategoryChange,
    #[serde(default)]
    pub observed_at: MemoryObservedAtChange,
    pub expected_revision: u64,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemoryDeleteRequest {
    pub conversation_id: String,
    pub memory_id: String,
    pub expected_revision: u64,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemoryPinRequest {
    pub conversation_id: String,
    pub memory_id: String,
    pub pinned: bool,
    pub expected_revision: u64,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemoryTemperatureRequest {
    pub conversation_id: String,
    pub memory_id: String,
    pub temperature: MemoryTemperature,
    pub expected_revision: u64,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemorySummaryUpdateRequest {
    pub conversation_id: String,
    pub summary: MemorySummaryEdit,
    pub expected_revision: u64,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemoryEditResult {
    pub revision: u64,
    pub memory_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemoryItemView {
    pub id: String,
    pub short_id: String,
    pub text: String,
    pub category: Option<MemoryCategory>,
    pub origin: MemoryOrigin,
    pub pinned: bool,
    pub temperature: MemoryTemperature,
    pub observed_at: Option<i64>,
    pub observed_time_precision: Option<String>,
    pub token_count: Option<u32>,
    pub cycle_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemorySummaryView {
    pub text: String,
    pub origin: MemoryOrigin,
    pub token_count: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_update_fields_keep_nullable_values() {
        let request = serde_json::from_str::<MemoryUpdateRequest>(r#"{"conversation_id":"conversation","memory_id":"memory","text":"Changed","expected_revision":1,"client_operation_id":"operation"}"#).expect("omitted fields keep their values");
        assert_eq!(request.category, MemoryCategoryChange::Keep);
        assert_eq!(request.observed_at, MemoryObservedAtChange::Keep);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum MemoryRunMode {
    Auto,
    AskFirst,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum MemoryCycleStatus {
    Queued,
    Processing,
    Complete,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum MemoryFailureCode {
    EmbeddingUnavailable,
    ModelMissing,
    ModelInvalid,
    PromptMissing,
    SettingsInvalid,
    ProviderUnavailable,
    ProviderRejected,
    EmptyResponse,
    TimedOut,
    RoundLimit,
    ToolFailed,
    StorageFailure,
    LeaseLost,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum MemoryPausedReason {
    LeaseLost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemoryStatusView {
    pub run_mode: MemoryRunMode,
    pub interval: u32,
    pub messages_since_last_cycle: u64,
    pub messages_until_next_cycle: u64,
    pub total_conversation_messages: u64,
    pub pending_approval_count: Option<u64>,
    pub skipped: bool,
    pub latest_cycle_status: Option<MemoryCycleStatus>,
    pub latest_job_id: Option<String>,
    pub failure: Option<MemoryFailureCode>,
    pub paused_reason: Option<MemoryPausedReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemoryView {
    pub items: Vec<MemoryItemView>,
    pub summary: Option<MemorySummaryView>,
    pub status: MemoryStatusView,
    pub revision: u64,
}
