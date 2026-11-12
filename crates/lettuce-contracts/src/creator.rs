use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum CreatorKind {
    Character,
    Persona,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CreatorScene {
    pub id: String,
    pub content: String,
    pub direction: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CreatorDraft {
    pub name: String,
    pub definition: String,
    pub scenes: Vec<CreatorScene>,
    /// Only `asset_id` is read from a request.
    pub avatar: Option<crate::AssetRef>,
}

/// A character or persona attached to a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CreatorReference {
    pub kind: CreatorKind,
    pub id: String,
}

/// An attached reference as it reads now; `name` is `None` once the item was deleted or archived.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CreatorReferenceView {
    pub kind: CreatorKind,
    pub id: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum CreatorImageOrigin {
    Upload,
    Generated,
    Edited,
}

/// An image of the session: uploaded with a message or made by the creator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CreatorImageView {
    pub asset: crate::AssetRef,
    pub origin: CreatorImageOrigin,
    pub turn_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CreatorStartRequest {
    pub kind: CreatorKind,
    pub target_id: Option<String>,
    pub draft: Option<CreatorDraft>,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CreatorGetRequest {
    pub workflow_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CreatorsListRequest {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CreatorRevisionRequest {
    pub workflow_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub proposal_id: String,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CreatorEditRequest {
    pub workflow_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub draft: CreatorDraft,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CreatorSendRequest {
    pub workflow_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub message: String,
    pub references: Vec<CreatorReference>,
    /// Asset ids of images picked for this message.
    pub images: Vec<String>,
    pub model_profile_id: Option<String>,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CreatorRegenerateRequest {
    pub workflow_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub turn_id: String,
    pub model_profile_id: Option<String>,
    pub client_operation_id: String,
}

/// Where a draft version came from: the session's starting point, a creator reply, a hand edit, or the copy made
/// when the draft was reviewed for saving.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum CreatorProposalSource {
    Start,
    Creator,
    Edit,
    Review,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CreatorProposalView {
    pub id: String,
    pub parent_id: Option<String>,
    pub ordinal: u32,
    pub source: CreatorProposalSource,
    pub draft: CreatorDraft,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum CreatorTurnStatus {
    Pending,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum CreatorToolStatus {
    Running,
    Succeeded,
    Failed,
}

/// One tool call the creator made during a turn, in call order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CreatorToolView {
    pub name: String,
    pub status: CreatorToolStatus,
    /// What an image call made so far: the image, or the job still making it.
    pub image: Option<CreatorToolImage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CreatorToolImage {
    pub asset: Option<crate::AssetRef>,
    pub job_id: Option<String>,
    pub failed: bool,
}

/// One piece of a turn's reply in the order the creator produced it: text it wrote, or the tool call at `index` in
/// `CreatorTurnView::tools`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CreatorTurnStep {
    Text { text: String },
    Tool { index: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CreatorTurnView {
    pub id: String,
    pub message: String,
    pub reply: String,
    /// The model's visible reasoning across the turn's rounds; empty when it showed none.
    pub reasoning: String,
    pub status: CreatorTurnStatus,
    pub job_id: String,
    pub base_proposal_id: String,
    pub result_proposal_id: Option<String>,
    pub regenerated: bool,
    /// Whether this turn can be answered again now: it is the session's latest turn, it has settled, and the draft
    /// still holds what it left behind.
    pub regenerable: bool,
    pub references: Vec<CreatorReferenceView>,
    pub images: Vec<crate::AssetRef>,
    pub tools: Vec<CreatorToolView>,
    pub steps: Vec<CreatorTurnStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CreatorSummary {
    pub id: String,
    pub kind: CreatorKind,
    pub name: String,
    pub target_id: Option<String>,
    pub applied_id: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CreatorPage {
    pub items: Vec<CreatorSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CreatorView {
    pub summary: CreatorSummary,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
    pub proposal: CreatorProposalView,
    pub history: Vec<CreatorProposalView>,
    pub turns: Vec<CreatorTurnView>,
    pub active_job_id: Option<String>,
    pub reviewed: bool,
    /// The saved character or persona changed or was archived after this session started, so this draft can no
    /// longer be saved over it.
    pub target_changed: bool,
    /// The model chosen for the latest reply, when one was picked for this session instead of the default.
    pub model_profile_id: Option<String>,
    pub images: Vec<CreatorImageView>,
}
