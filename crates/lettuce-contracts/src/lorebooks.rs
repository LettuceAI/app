use serde::{Deserialize, Serialize};

/// Which messages a lorebook scans for keywords: the latest messages up to
/// the scan depth setting, or only the newest user message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LorebookDetection {
    RecentMessages,
    LatestUserMessage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LorebookKeywordMode {
    Literal,
    Regex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LorebookStatus {
    Active,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookSummary {
    pub id: String,
    pub name: String,
    pub status: LorebookStatus,
    pub detection: LorebookDetection,
    pub icon: Option<crate::AssetRef>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookEntryView {
    pub id: String,
    pub title: String,
    pub enabled: bool,
    pub always_active: bool,
    pub keywords: Vec<String>,
    pub case_sensitive: bool,
    pub keyword_mode: LorebookKeywordMode,
    pub content: String,
    pub priority: i32,
    pub ordinal: u32,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookView {
    pub lorebook: LorebookSummary,
    pub entries: Vec<LorebookEntryView>,
}

/// Lorebooks most recently changed first; `query` keeps those whose name
/// contains it, ignoring case.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebooksListRequest {
    pub query: Option<String>,
    pub lifecycle: Option<crate::LifecycleFilter>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookPage {
    pub items: Vec<LorebookSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookGetRequest {
    pub lorebook_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookMetadataInput {
    pub name: String,
    pub detection: LorebookDetection,
    pub icon_asset_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookEntryInput {
    pub title: String,
    pub enabled: bool,
    pub always_active: bool,
    pub keywords: Vec<String>,
    pub case_sensitive: bool,
    pub keyword_mode: LorebookKeywordMode,
    pub content: String,
    pub priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookCreateRequest {
    pub client_operation_id: String,
    pub metadata: LorebookMetadataInput,
    pub entries: Vec<LorebookEntryInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookUpdateMetadataRequest {
    pub client_operation_id: String,
    pub lorebook_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub metadata: LorebookMetadataInput,
}

/// One change to a book's entries. `index` is the position after the
/// mutations before it; an add without one appends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum LorebookEntryMutationInput {
    Add {
        entry: LorebookEntryInput,
        index: Option<u32>,
    },
    Update {
        entry_id: String,
        entry: LorebookEntryInput,
    },
    Remove {
        entry_id: String,
    },
    Reorder {
        entry_id: String,
        index: u32,
    },
}

/// Applies every mutation in order under one revision check; nothing is
/// written when one of them fails.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookEntriesMutateRequest {
    pub client_operation_id: String,
    pub lorebook_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub mutations: Vec<LorebookEntryMutationInput>,
}

/// Archive, restore and hard delete of a lorebook.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookRevisionRequest {
    pub client_operation_id: String,
    pub lorebook_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
}

/// The owners whose configuration a hard delete changed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SourceDeleteResult {
    pub character_ids: Vec<String>,
    pub persona_ids: Vec<String>,
    pub group_ids: Vec<String>,
    pub conversation_ids: Vec<String>,
    pub settings_changed: bool,
}

/// What the preview evaluates: the next turn of a conversation (with the
/// composer text as its newest user message, and for a group the given
/// speaker's books), or one book against a text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum LorebookTriggerPreviewRequest {
    Conversation {
        conversation_id: String,
        composer_text: Option<String>,
        speaker_character_id: Option<String>,
    },
    Editor {
        lorebook_id: String,
        text: String,
    },
}

/// Where an active book came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum LorebookSourceTier {
    Conversation,
    Character { character_id: String },
    Persona { persona_id: String },
    Group { group_id: String },
    Editor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LorebookSkipReason {
    Missing,
    Archived,
    Duplicate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookPreviewEntry {
    /// The entry's place in the injected lore, from 0.
    pub position: u32,
    pub source: LorebookSourceTier,
    pub lorebook_id: String,
    pub lorebook_name: String,
    pub entry_id: String,
    pub title: String,
    pub matched_keywords: Vec<String>,
    pub always_active: bool,
    pub token_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookSkippedSource {
    pub source: LorebookSourceTier,
    pub lorebook_id: String,
    pub reason: LorebookSkipReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookTriggerPreview {
    pub entries: Vec<LorebookPreviewEntry>,
    pub skipped: Vec<LorebookSkippedSource>,
    pub scan_depth: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct TokensCountRequest {
    pub texts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct TokensCount {
    pub counts: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LorebookEntryDraftSource {
    Messages,
    Memory,
    Mixed,
}

/// Drafts one entry for `lorebook_id` from a direct conversation. The result
/// is the job's `LorebookEntryDraft` or `LorebookNoEntry`; nothing is
/// written to the book.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookEntryDraftRequest {
    pub client_operation_id: String,
    pub conversation_id: String,
    pub lorebook_id: String,
    pub source: LorebookEntryDraftSource,
    pub message_ids: Vec<String>,
    pub memory_ids: Vec<String>,
    pub use_summary: bool,
    pub direction: Option<String>,
    pub force: bool,
}

/// Drafts keywords for an entry's content; the result is the job's
/// `LorebookKeywords`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookKeywordsDraftRequest {
    pub client_operation_id: String,
    pub lorebook_id: String,
    pub entry_id: Option<String>,
    pub title: Option<String>,
    pub content: String,
    pub existing_keywords: Vec<String>,
    pub direction: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookEntryDraftResult {
    pub title: String,
    pub content: String,
    pub keywords: Vec<String>,
    pub always_active: bool,
}

/// One staged generator source: pasted text with a label, or a document the
/// app ingested (`assets_ingest`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum LorebookProjectSourceInput {
    Text { label: String, text: String },
    Document { asset_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookProjectCreateRequest {
    pub client_operation_id: String,
    pub brief: String,
    pub lorebook_name: Option<String>,
    pub target_count: Option<u32>,
    pub sources: Vec<LorebookProjectSourceInput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LorebookProjectStage {
    Created,
    Planning,
    /// The planner job failed; `lorebook_project_plan` retries it.
    PlanFailed,
    AwaitingOutlineApproval,
    Drafting,
    DraftsReady,
    CoherenceReview,
    Committed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookProjectSourceView {
    pub source_id: String,
    pub label: String,
    pub document: Option<crate::AssetRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookPlanView {
    pub plan_id: String,
    pub title: String,
    pub category: String,
    pub proposed_keys: Vec<String>,
    pub rationale: String,
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookPlanInput {
    /// An existing plan keeps its id; a new one gets one.
    pub plan_id: Option<String>,
    pub title: String,
    pub category: String,
    pub proposed_keys: Vec<String>,
    pub rationale: String,
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LorebookDraftStatus {
    Pending,
    Drafting,
    Drafted,
    Approved,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookDraftRevisionView {
    pub feedback: String,
    pub content: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookDraftView {
    pub plan_id: String,
    pub title: String,
    pub keywords: Vec<String>,
    pub content: String,
    pub always_active: bool,
    pub status: LorebookDraftStatus,
    pub revisions: Vec<LorebookDraftRevisionView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LorebookCoherenceChangeView {
    MergeKeys {
        id: String,
        plan_id: String,
        remove_keys: Vec<String>,
        reason: String,
    },
    RenameTerm {
        id: String,
        old_term: String,
        new_term: String,
        plan_ids: Option<Vec<String>>,
        reason: String,
    },
    FlagContradiction {
        id: String,
        plan_ids: Vec<String>,
        description: String,
    },
    ToggleAlwaysActive {
        id: String,
        plan_id: String,
        new_value: bool,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookProjectCommitView {
    pub lorebook_id: String,
    pub lorebook_name: String,
    pub entry_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookProjectView {
    pub project_id: String,
    pub brief: String,
    pub lorebook_name: Option<String>,
    pub target_count: u32,
    pub stage: LorebookProjectStage,
    pub sources: Vec<LorebookProjectSourceView>,
    pub outline: Vec<LorebookPlanView>,
    pub drafts: Vec<LorebookDraftView>,
    pub coherence_changes: Vec<LorebookCoherenceChangeView>,
    pub commit: Option<LorebookProjectCommitView>,
    /// The jobs still queued or running for the project.
    pub active_job_ids: Vec<String>,
    /// Why the last planner job failed, while the stage is `PlanFailed`.
    pub plan_failure: Option<crate::JobFailureDto>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookProjectGetRequest {
    pub project_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookProjectsListRequest {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookProjectSummary {
    pub project_id: String,
    pub brief: String,
    pub lorebook_name: Option<String>,
    pub stage: LorebookProjectStage,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookProjectPage {
    pub items: Vec<LorebookProjectSummary>,
    pub next_cursor: Option<String>,
}

/// A project write checked against the project revision the caller saw.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookProjectRevisionRequest {
    pub client_operation_id: String,
    pub project_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookProjectOutlineUpdateRequest {
    pub client_operation_id: String,
    pub project_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub outline: Vec<LorebookPlanInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookProjectDraftUpdateRequest {
    pub client_operation_id: String,
    pub project_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub plan_id: String,
    pub title: String,
    pub keywords: Vec<String>,
    pub content: String,
    pub always_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookProjectDraftApprovalRequest {
    pub client_operation_id: String,
    pub project_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub plan_id: String,
    pub approved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookProjectRefineRequest {
    pub client_operation_id: String,
    pub project_id: String,
    pub plan_id: String,
    pub feedback: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookProjectJobRequest {
    pub client_operation_id: String,
    pub project_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
}

/// The batch of writer jobs one `lorebook_project_draft_next` started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookProjectBatchAccepted {
    pub job_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookProjectCoherenceApplyRequest {
    pub client_operation_id: String,
    pub project_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub accepted_change_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum LorebookProjectCommitTarget {
    NewLorebook {
        name: Option<String>,
    },
    ExistingLorebook {
        lorebook_id: String,
        #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
        expected_revision: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookProjectCommitRequest {
    pub client_operation_id: String,
    pub project_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub target: LorebookProjectCommitTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookGeneratorDefaults {
    pub target_count: u32,
    pub min_target_count: u32,
    pub max_target_count: u32,
}
