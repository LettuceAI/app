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
    Set(
        #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
        Option<i64>,
    ),
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
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub observed_at: Option<i64>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
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
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemoryDeleteRequest {
    pub conversation_id: String,
    pub memory_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
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
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
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
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemorySummaryUpdateRequest {
    pub conversation_id: String,
    pub summary: MemorySummaryEdit,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemoryEditResult {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
    pub memory_id: Option<String>,
}

/// Why a forced memory cycle cannot start: the conversation does not use
/// dynamic memory, the global dynamic memory switch is off, there is no
/// dialogue to summarise yet, or a cycle already runs for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum MemoryGateReason {
    NotDynamic,
    Disabled,
    NothingToSummarise,
    CycleRunning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemoryTriggerRequest {
    pub conversation_id: String,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemoryRetryRequest {
    pub conversation_id: String,
    pub model_profile_id: Option<String>,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemorySkipRequest {
    pub conversation_id: String,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemoryErrorDismissRequest {
    pub conversation_id: String,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemoryCyclesRequest {
    pub conversation_id: String,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemoryCycleRevertRequest {
    pub conversation_id: String,
    pub run_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
}

/// What one tool call of a cycle did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum MemoryCycleActionKind {
    Created,
    DuplicateSkipped,
    Deleted,
    SoftDeleted,
    Pinned,
    Unpinned,
    TargetNotFound,
    Done,
    Rejected,
    Skipped,
    StoppedAfterDone,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemoryCycleAction {
    pub kind: MemoryCycleActionKind,
    pub memory_id: Option<String>,
    pub text: Option<String>,
}

/// One memory cycle of the activity log, newest first. `blocked_by` names
/// the later cycle that started from this one's result and keeps it from
/// being reverted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemoryCycleView {
    pub run_id: String,
    pub job_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub started_at: i64,
    pub label: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub window_start: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub window_end: u64,
    pub status: MemoryCycleStatus,
    pub failure: Option<MemoryFailureCode>,
    pub summary: Option<String>,
    pub actions: Vec<MemoryCycleAction>,
    pub reverted: bool,
    pub revertable: bool,
    pub blocked_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MemoryCyclePage {
    pub items: Vec<MemoryCycleView>,
    pub next_cursor: Option<String>,
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
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
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
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub messages_since_last_cycle: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub messages_until_next_cycle: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub total_conversation_messages: u64,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
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
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
}
