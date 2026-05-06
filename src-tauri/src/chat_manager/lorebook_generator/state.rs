use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SourceInput {
    #[serde(rename_all = "camelCase")]
    Text { label: String, body: String },
    #[serde(rename_all = "camelCase")]
    File {
        name: String,
        data_base64: String,
        kind: SourceFileKind,
    },
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SourceFileKind {
    Txt,
    Md,
    Pdf,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SourceExcerpt {
    pub id: String,
    pub label: String,
    pub content: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WorldDigest {
    pub excerpts: Vec<SourceExcerpt>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EntryPlan {
    pub idx: usize,
    pub title: String,
    pub category: String,
    pub proposed_keys: Vec<String>,
    pub rationale: String,
    #[serde(default)]
    pub source_refs: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EntryDraft {
    pub plan_idx: usize,
    pub title: String,
    pub keywords: Vec<String>,
    pub content: String,
    pub always_active: bool,
    pub status: DraftStatus,
    #[serde(default)]
    pub revisions: Vec<DraftRevision>,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DraftStatus {
    Pending,
    Drafting,
    Drafted,
    Approved,
    Failed,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DraftRevision {
    pub feedback: String,
    pub content: String,
    pub timestamp_ms: u64,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CoherenceChange {
    #[serde(rename_all = "camelCase")]
    MergeKeys {
        id: String,
        entry_idx: usize,
        remove_keys: Vec<String>,
        reason: String,
    },
    #[serde(rename_all = "camelCase")]
    RenameTerm {
        id: String,
        old_term: String,
        new_term: String,
        affected_entry_idxs: Vec<usize>,
        reason: String,
    },
    #[serde(rename_all = "camelCase")]
    FlagContradiction {
        id: String,
        entry_idxs: Vec<usize>,
        description: String,
    },
    #[serde(rename_all = "camelCase")]
    ToggleAlwaysActive {
        id: String,
        entry_idx: usize,
        new_value: bool,
        reason: String,
    },
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum JobStage {
    Created,
    Planning,
    AwaitingOutlineApproval,
    Drafting,
    DraftsReady,
    CoherenceReview,
    Committed,
    Cancelled,
    Failed,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct PromptOverrides {
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub planner_prompt_template_id: Option<String>,
    #[serde(default)]
    pub writer_prompt_template_id: Option<String>,
    #[serde(default)]
    pub refine_prompt_template_id: Option<String>,
    #[serde(default)]
    pub coherence_prompt_template_id: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JobState {
    pub id: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub stage: JobStage,
    pub brief: String,
    pub initial_lorebook_name: Option<String>,
    pub target_count: u32,
    pub digest: WorldDigest,
    pub overrides: PromptOverrides,
    #[serde(default)]
    pub outline: Vec<EntryPlan>,
    #[serde(default)]
    pub drafts: Vec<EntryDraft>,
    #[serde(default)]
    pub coherence_proposals: Vec<CoherenceChange>,
    #[serde(default)]
    pub last_error: Option<String>,
}

#[derive(Default)]
pub struct JobRegistry {
    pub jobs: Mutex<HashMap<String, JobState>>,
}

impl JobRegistry {
    pub fn new() -> Self {
        Self::default()
    }
}
