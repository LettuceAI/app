use std::collections::HashSet;

use lettuce_conversations::ResolvedInferenceProfile;
use lettuce_conversations::{ProposedToolCall, ToolChoice, ToolDefinition, ToolRequest};
use lettuce_types::{
    CreationWorkflowId, JobId, LorebookEntryId, PromptDocumentId, RequestId, Revision,
    TimestampMillis,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

pub const MIN_STAGED_LOREBOOK_TARGET_COUNT: u32 = 5;
pub const MAX_STAGED_LOREBOOK_TARGET_COUNT: u32 = 50;
pub const MAX_STAGED_LOREBOOK_EXCERPT_CHARS: usize = 20_000;
pub const STAGED_LOREBOOK_DRAFT_BATCH_SIZE: usize = 3;
pub const STAGED_LOREBOOK_PLANNER_TOOL_NAME: &str = "propose_lorebook_outline";
pub const STAGED_LOREBOOK_PLANNER_FINAL_INSTRUCTION: &str =
    "Call propose_lorebook_outline now with exactly the requested number of entries.";
pub const STAGED_LOREBOOK_WRITER_TOOL_NAME: &str = "write_lorebook_entry";
pub const STAGED_LOREBOOK_WRITER_FINAL_INSTRUCTION: &str =
    "Call write_lorebook_entry now with the final entry.";

#[must_use]
pub fn staged_lorebook_planner_tool_request() -> ToolRequest {
    ToolRequest {
        definitions: vec![ToolDefinition {
            name: STAGED_LOREBOOK_PLANNER_TOOL_NAME.to_owned(),
            description: Some("Propose the full outline of lorebook entries to draft.".into()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "entries": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "title": { "type": "string" },
                                "category": { "type": "string" },
                                "proposedKeys": { "type": "array", "items": { "type": "string" } },
                                "rationale": { "type": "string" },
                                "sourceRefs": { "type": "array", "items": { "type": "string" } }
                            },
                            "required": ["title", "category", "rationale"]
                        }
                    }
                },
                "required": ["entries"]
            }),
            version: 1,
        }],
        choice: ToolChoice::Required,
    }
}

#[must_use]
pub fn staged_lorebook_writer_tool_request() -> ToolRequest {
    ToolRequest {
        definitions: vec![ToolDefinition {
            name: STAGED_LOREBOOK_WRITER_TOOL_NAME.to_owned(),
            description: Some("Write the body of a single lorebook entry.".into()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "keywords": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "content": { "type": "string" },
                    "alwaysActive": { "type": "boolean" }
                },
                "required": ["title", "content"]
            }),
            version: 1,
        }],
        choice: ToolChoice::Required,
    }
}

pub fn reduce_staged_lorebook_writer_calls(
    plan_id: LorebookEntryId,
    calls: &[ProposedToolCall],
) -> Result<StagedLorebookEntryDraft, StagedLorebookError> {
    let call = calls
        .iter()
        .find(|call| call.name == STAGED_LOREBOOK_WRITER_TOOL_NAME)
        .ok_or(StagedLorebookError::InvalidDraft)?;
    let object = call
        .arguments
        .as_object()
        .ok_or(StagedLorebookError::InvalidDraft)?;
    let title = required_draft_text(object.get("title"))?;
    let content = required_draft_text(object.get("content"))?;
    let always_active = object
        .get("alwaysActive")
        .or_else(|| object.get("always_active"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let keywords = string_array(object.get("keywords"), true);
    Ok(StagedLorebookEntryDraft {
        plan_id,
        title,
        keywords,
        content,
        always_active,
        status: StagedLorebookDraftStatus::Drafted,
        revisions: Vec::new(),
    })
}

pub fn reduce_staged_lorebook_planner_calls(
    project: &StagedLorebookProject,
    calls: &[ProposedToolCall],
) -> Result<Vec<StagedLorebookEntryPlan>, StagedLorebookError> {
    let call = calls
        .iter()
        .find(|call| call.name == STAGED_LOREBOOK_PLANNER_TOOL_NAME)
        .ok_or(StagedLorebookError::InvalidOutline)?;
    let entries = call
        .arguments
        .get("entries")
        .and_then(Value::as_array)
        .ok_or(StagedLorebookError::InvalidOutline)?;
    let mut outline = Vec::with_capacity(entries.len());
    for (ordinal, value) in entries.iter().enumerate() {
        let object = value
            .as_object()
            .ok_or(StagedLorebookError::InvalidOutline)?;
        let title = required_text(object.get("title"))?;
        let category = object
            .get("category")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("other")
            .to_owned();
        let rationale = object
            .get("rationale")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("")
            .to_owned();
        let proposed_keys = string_array(
            object
                .get("proposedKeys")
                .or_else(|| object.get("proposed_keys")),
            true,
        );
        let source_refs = string_array(
            object
                .get("sourceRefs")
                .or_else(|| object.get("source_refs")),
            false,
        );
        outline.push(StagedLorebookEntryPlan {
            id: LorebookEntryId::from_uuid(Uuid::new_v5(
                &project.id.as_uuid(),
                format!("outline-{ordinal}").as_bytes(),
            )),
            ordinal: u32::try_from(ordinal).map_err(|_| StagedLorebookError::InvalidOutline)?,
            title,
            category,
            proposed_keys,
            rationale,
            source_refs,
        });
    }
    validate_outline(&outline, &project.excerpts)?;
    Ok(outline)
}

fn required_text(value: Option<&Value>) -> Result<String, StagedLorebookError> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or(StagedLorebookError::InvalidOutline)
}

fn required_draft_text(value: Option<&Value>) -> Result<String, StagedLorebookError> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or(StagedLorebookError::InvalidDraft)
}

fn string_array(value: Option<&Value>, trim: bool) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(|value| if trim { value.trim() } else { value })
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StagedLorebookStage {
    Created,
    Planning,
    AwaitingOutlineApproval,
    Drafting,
    DraftsReady,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StagedLorebookSourceExcerpt {
    pub source_id: String,
    pub label: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StagedLorebookEntryPlan {
    pub id: LorebookEntryId,
    pub ordinal: u32,
    pub title: String,
    pub category: String,
    pub proposed_keys: Vec<String>,
    pub rationale: String,
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StagedLorebookDraftStatus {
    Pending,
    Drafting,
    Drafted,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StagedLorebookDraftRevision {
    pub feedback: String,
    pub content: String,
    pub timestamp: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StagedLorebookEntryDraft {
    pub plan_id: LorebookEntryId,
    pub title: String,
    pub keywords: Vec<String>,
    pub content: String,
    pub always_active: bool,
    pub status: StagedLorebookDraftStatus,
    pub revisions: Vec<StagedLorebookDraftRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StagedLorebookProject {
    pub id: CreationWorkflowId,
    pub brief: String,
    pub initial_lorebook_name: Option<String>,
    pub target_count: u32,
    pub excerpts: Vec<StagedLorebookSourceExcerpt>,
    pub outline: Vec<StagedLorebookEntryPlan>,
    #[serde(default)]
    pub drafts: Vec<StagedLorebookEntryDraft>,
    pub stage: StagedLorebookStage,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StagedLorebookError {
    #[error("staged lorebook input is invalid")]
    InvalidInput,
    #[error("staged lorebook transition is invalid")]
    InvalidTransition,
    #[error("staged lorebook outline is invalid")]
    InvalidOutline,
    #[error("staged lorebook draft is invalid")]
    InvalidDraft,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StagedLorebookPlanningRun {
    pub request_id: RequestId,
    pub job_id: JobId,
    pub project: StagedLorebookProject,
    pub planner_profile: ResolvedInferenceProfile,
    pub planner_prompt_id: PromptDocumentId,
    pub planner_prompt_revision: Revision,
    #[serde(default)]
    pub planner_attempt: Option<StagedLorebookPlannerAttempt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum StagedLorebookPlannerDecision {
    Outline(Vec<StagedLorebookEntryPlan>),
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StagedLorebookPlannerUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StagedLorebookPlannerAttempt {
    pub project_revision: Revision,
    pub calls: Vec<ProposedToolCall>,
    pub decision: StagedLorebookPlannerDecision,
    pub usage: Option<StagedLorebookPlannerUsage>,
    pub provider_finish_reason: Option<String>,
    pub provider_request_id: Option<String>,
    pub completed_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StagedLorebookWriterPromptValues {
    pub brief: String,
    pub outline: String,
    pub entry_title: String,
    pub entry_category: String,
    pub entry_proposed_keys: String,
    pub entry_rationale: String,
    pub relevant_excerpts: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StagedLorebookWriterRun {
    pub request_id: RequestId,
    pub job_id: JobId,
    pub project_request_id: RequestId,
    pub project_id: CreationWorkflowId,
    pub project_revision: Revision,
    pub plan_id: LorebookEntryId,
    pub profile: ResolvedInferenceProfile,
    pub prompt_id: PromptDocumentId,
    pub prompt_revision: Revision,
    pub prompt_values: StagedLorebookWriterPromptValues,
    pub created_at: TimestampMillis,
    #[serde(default)]
    pub attempt: Option<StagedLorebookWriterAttempt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum StagedLorebookWriterDecision {
    Draft(StagedLorebookEntryDraft),
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StagedLorebookWriterUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StagedLorebookWriterAttempt {
    pub calls: Vec<ProposedToolCall>,
    pub decision: StagedLorebookWriterDecision,
    pub usage: Option<StagedLorebookWriterUsage>,
    pub provider_finish_reason: Option<String>,
    pub provider_request_id: Option<String>,
    pub completed_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StagedLorebookWriterRunRepositoryError {
    #[error("staged lorebook writer run was not found")]
    NotFound,
    #[error("staged lorebook writer run conflicts with durable state")]
    Conflict,
    #[error("staged lorebook writer run is invalid")]
    Invalid,
    #[error("staged lorebook writer run storage failed")]
    Failure,
    #[error("staged lorebook writer run storage is corrupt")]
    Corrupt,
}

pub trait StagedLorebookWriterRunRepository: Send + Sync {
    fn admit_staged_lorebook_writer_run(
        &self,
        run: StagedLorebookWriterRun,
    ) -> Result<StagedLorebookWriterRun, StagedLorebookWriterRunRepositoryError>;

    fn load_staged_lorebook_writer_run(
        &self,
        request_id: RequestId,
    ) -> Result<StagedLorebookWriterRun, StagedLorebookWriterRunRepositoryError>;

    fn commit_staged_lorebook_writer_attempt(
        &self,
        request_id: RequestId,
        attempt: StagedLorebookWriterAttempt,
    ) -> Result<StagedLorebookWriterRun, StagedLorebookWriterRunRepositoryError>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StagedLorebookRepositoryError {
    #[error("staged lorebook project was not found")]
    NotFound,
    #[error("staged lorebook project conflicts with durable state")]
    Conflict,
    #[error("staged lorebook project is invalid")]
    Invalid,
    #[error("staged lorebook project storage failed")]
    Failure,
    #[error("staged lorebook project storage is corrupt")]
    Corrupt,
}

pub trait StagedLorebookRepository: Send + Sync {
    fn admit_staged_lorebook(
        &self,
        run: StagedLorebookPlanningRun,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError>;

    fn load_staged_lorebook(
        &self,
        request_id: RequestId,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError>;

    fn start_staged_lorebook_planning(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError>;

    fn submit_staged_lorebook_outline(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        outline: Vec<StagedLorebookEntryPlan>,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError>;

    fn commit_staged_lorebook_planner_attempt(
        &self,
        request_id: RequestId,
        attempt: StagedLorebookPlannerAttempt,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError>;

    fn approve_staged_lorebook_outline(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError>;

    fn settle_staged_lorebook_draft(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        draft: StagedLorebookEntryDraft,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError>;

    fn start_staged_lorebook_draft_batch(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError>;

    fn fail_staged_lorebook_draft(
        &self,
        request_id: RequestId,
        plan_id: LorebookEntryId,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError>;
}

impl StagedLorebookPlanningRun {
    pub fn validate(&self) -> Result<(), StagedLorebookRepositoryError> {
        self.project
            .validate()
            .map_err(|_| StagedLorebookRepositoryError::Invalid)?;
        if self.planner_prompt_revision.get() == 0
            || serde_json::to_vec(&self.planner_profile).is_err()
            || self
                .planner_attempt
                .as_ref()
                .is_some_and(|attempt| attempt.validate(&self.project).is_err())
        {
            return Err(StagedLorebookRepositoryError::Invalid);
        }
        Ok(())
    }
}

impl StagedLorebookPlannerAttempt {
    pub fn validate(
        &self,
        project: &StagedLorebookProject,
    ) -> Result<(), StagedLorebookRepositoryError> {
        if self.project_revision.get() == 0
            || self.completed_at.get() < 0
            || self
                .calls
                .iter()
                .any(|call| call.provider_replay.is_some() || call.validate().is_err())
            || matches!(&self.decision, StagedLorebookPlannerDecision::Outline(outline) if validate_outline(outline, &project.excerpts).is_err())
        {
            return Err(StagedLorebookRepositoryError::Invalid);
        }
        Ok(())
    }
}

impl StagedLorebookWriterRun {
    pub fn validate(&self) -> Result<(), StagedLorebookWriterRunRepositoryError> {
        let values = &self.prompt_values;
        if self.project_revision.get() == 0
            || self.prompt_revision.get() == 0
            || self.created_at.get() < 0
            || values.brief.trim().is_empty()
            || values.brief != values.brief.trim()
            || values.outline.is_empty()
            || values.entry_title.trim().is_empty()
            || values.entry_title != values.entry_title.trim()
            || values.entry_category.trim().is_empty()
            || values.entry_category != values.entry_category.trim()
            || values.entry_proposed_keys.is_empty()
            || values.relevant_excerpts.is_empty()
            || serde_json::to_vec(&self.profile).is_err()
            || self
                .attempt
                .as_ref()
                .is_some_and(|attempt| attempt.validate(self.plan_id).is_err())
        {
            return Err(StagedLorebookWriterRunRepositoryError::Invalid);
        }
        Ok(())
    }
}

impl StagedLorebookWriterAttempt {
    pub fn validate(
        &self,
        plan_id: LorebookEntryId,
    ) -> Result<(), StagedLorebookWriterRunRepositoryError> {
        if self.completed_at.get() < 0
            || self
                .calls
                .iter()
                .any(|call| call.provider_replay.is_some() || call.validate().is_err())
            || matches!(&self.decision, StagedLorebookWriterDecision::Draft(draft) if validate_written_draft(draft, plan_id).is_err())
        {
            return Err(StagedLorebookWriterRunRepositoryError::Invalid);
        }
        Ok(())
    }
}

#[must_use]
pub fn clamp_staged_lorebook_target_count(value: u32) -> u32 {
    value.clamp(
        MIN_STAGED_LOREBOOK_TARGET_COUNT,
        MAX_STAGED_LOREBOOK_TARGET_COUNT,
    )
}

impl StagedLorebookProject {
    pub fn create(
        id: CreationWorkflowId,
        brief: String,
        initial_lorebook_name: Option<String>,
        target_count: u32,
        excerpts: Vec<StagedLorebookSourceExcerpt>,
        now: TimestampMillis,
    ) -> Result<Self, StagedLorebookError> {
        let project = Self {
            id,
            brief: brief.trim().to_owned(),
            initial_lorebook_name: initial_lorebook_name
                .map(|name| name.trim().to_owned())
                .filter(|name| !name.is_empty()),
            target_count: clamp_staged_lorebook_target_count(target_count),
            excerpts,
            outline: Vec::new(),
            drafts: Vec::new(),
            stage: StagedLorebookStage::Created,
            revision: Revision::new(1),
            created_at: now,
            updated_at: now,
        };
        project.validate()?;
        Ok(project)
    }

    pub fn start_planning(&self, now: TimestampMillis) -> Result<Self, StagedLorebookError> {
        if self.stage != StagedLorebookStage::Created || now < self.updated_at {
            return Err(StagedLorebookError::InvalidTransition);
        }
        let mut next = self.clone();
        next.stage = StagedLorebookStage::Planning;
        next.updated_at = now;
        next.revision = next
            .revision
            .next()
            .map_err(|_| StagedLorebookError::InvalidTransition)?;
        Ok(next)
    }

    pub fn submit_outline(
        &self,
        outline: Vec<StagedLorebookEntryPlan>,
        now: TimestampMillis,
    ) -> Result<Self, StagedLorebookError> {
        if self.stage != StagedLorebookStage::Planning || now < self.updated_at {
            return Err(StagedLorebookError::InvalidTransition);
        }
        validate_outline(&outline, &self.excerpts)?;
        let mut next = self.clone();
        next.outline = outline;
        next.stage = StagedLorebookStage::AwaitingOutlineApproval;
        next.updated_at = now;
        next.revision = next
            .revision
            .next()
            .map_err(|_| StagedLorebookError::InvalidTransition)?;
        Ok(next)
    }

    pub fn approve_outline(&self, now: TimestampMillis) -> Result<Self, StagedLorebookError> {
        if self.stage != StagedLorebookStage::AwaitingOutlineApproval
            || self.outline.is_empty()
            || now < self.updated_at
        {
            return Err(StagedLorebookError::InvalidTransition);
        }
        let mut next = self.clone();
        next.drafts = self
            .outline
            .iter()
            .map(|plan| StagedLorebookEntryDraft {
                plan_id: plan.id,
                title: plan.title.clone(),
                keywords: plan.proposed_keys.clone(),
                content: String::new(),
                always_active: false,
                status: StagedLorebookDraftStatus::Pending,
                revisions: Vec::new(),
            })
            .collect();
        next.stage = StagedLorebookStage::Drafting;
        next.updated_at = now;
        next.revision = next
            .revision
            .next()
            .map_err(|_| StagedLorebookError::InvalidTransition)?;
        Ok(next)
    }

    pub fn settle_draft(
        &self,
        draft: StagedLorebookEntryDraft,
        now: TimestampMillis,
    ) -> Result<Self, StagedLorebookError> {
        if self.stage != StagedLorebookStage::Drafting || now < self.updated_at {
            return Err(StagedLorebookError::InvalidTransition);
        }
        validate_written_draft(&draft, draft.plan_id)?;
        let position = self
            .drafts
            .iter()
            .position(|stored| stored.plan_id == draft.plan_id)
            .filter(|position| {
                matches!(
                    self.drafts[*position].status,
                    StagedLorebookDraftStatus::Pending | StagedLorebookDraftStatus::Drafting
                )
            })
            .ok_or(StagedLorebookError::InvalidTransition)?;
        let mut next = self.clone();
        next.drafts[position] = draft;
        next.finish_drafting_if_terminal();
        next.updated_at = now;
        next.revision = next
            .revision
            .next()
            .map_err(|_| StagedLorebookError::InvalidTransition)?;
        Ok(next)
    }

    pub fn start_draft_batch(&self, now: TimestampMillis) -> Result<Self, StagedLorebookError> {
        if !matches!(
            self.stage,
            StagedLorebookStage::Drafting | StagedLorebookStage::DraftsReady
        ) || now < self.updated_at
        {
            return Err(StagedLorebookError::InvalidTransition);
        }
        if self
            .drafts
            .iter()
            .any(|draft| draft.status == StagedLorebookDraftStatus::Drafting)
        {
            return Ok(self.clone());
        }
        let mut next = self.clone();
        let mut selected = 0;
        for draft in &mut next.drafts {
            if matches!(
                draft.status,
                StagedLorebookDraftStatus::Pending | StagedLorebookDraftStatus::Failed
            ) && selected < STAGED_LOREBOOK_DRAFT_BATCH_SIZE
            {
                draft.status = StagedLorebookDraftStatus::Drafting;
                selected += 1;
            }
        }
        next.stage = StagedLorebookStage::Drafting;
        if selected == 0 {
            next.finish_drafting_if_terminal();
        }
        next.updated_at = now;
        next.revision = next
            .revision
            .next()
            .map_err(|_| StagedLorebookError::InvalidTransition)?;
        Ok(next)
    }

    pub fn fail_draft(
        &self,
        plan_id: LorebookEntryId,
        now: TimestampMillis,
    ) -> Result<Self, StagedLorebookError> {
        if self.stage != StagedLorebookStage::Drafting || now < self.updated_at {
            return Err(StagedLorebookError::InvalidTransition);
        }
        let position = self
            .drafts
            .iter()
            .position(|draft| {
                draft.plan_id == plan_id && draft.status == StagedLorebookDraftStatus::Drafting
            })
            .ok_or(StagedLorebookError::InvalidTransition)?;
        let mut next = self.clone();
        next.drafts[position].status = StagedLorebookDraftStatus::Failed;
        next.finish_drafting_if_terminal();
        next.updated_at = now;
        next.revision = next
            .revision
            .next()
            .map_err(|_| StagedLorebookError::InvalidTransition)?;
        Ok(next)
    }

    fn finish_drafting_if_terminal(&mut self) {
        if self.drafts.iter().all(|draft| {
            !matches!(
                draft.status,
                StagedLorebookDraftStatus::Pending | StagedLorebookDraftStatus::Drafting
            )
        }) {
            self.stage = StagedLorebookStage::DraftsReady;
        }
    }

    pub fn validate(&self) -> Result<(), StagedLorebookError> {
        if self.brief.is_empty()
            || self.brief != self.brief.trim()
            || self.target_count < MIN_STAGED_LOREBOOK_TARGET_COUNT
            || self.target_count > MAX_STAGED_LOREBOOK_TARGET_COUNT
            || self.revision.get() == 0
            || self.created_at.get() < 0
            || self.updated_at < self.created_at
        {
            return Err(StagedLorebookError::InvalidInput);
        }
        let mut source_ids = HashSet::with_capacity(self.excerpts.len());
        for excerpt in &self.excerpts {
            if excerpt.source_id.trim().is_empty()
                || excerpt.source_id != excerpt.source_id.trim()
                || excerpt.label.trim().is_empty()
                || excerpt.label != excerpt.label.trim()
                || excerpt.content.chars().count() > MAX_STAGED_LOREBOOK_EXCERPT_CHARS
                || !source_ids.insert(excerpt.source_id.as_str())
            {
                return Err(StagedLorebookError::InvalidInput);
            }
        }
        match self.stage {
            StagedLorebookStage::Created | StagedLorebookStage::Planning
                if !self.outline.is_empty() || !self.drafts.is_empty() =>
            {
                Err(StagedLorebookError::InvalidOutline)
            }
            StagedLorebookStage::AwaitingOutlineApproval => {
                validate_outline(&self.outline, &self.excerpts)?;
                if self.drafts.is_empty() {
                    Ok(())
                } else {
                    Err(StagedLorebookError::InvalidOutline)
                }
            }
            StagedLorebookStage::Drafting | StagedLorebookStage::DraftsReady => {
                validate_outline(&self.outline, &self.excerpts)?;
                validate_drafts(&self.drafts, &self.outline)?;
                if self.stage == StagedLorebookStage::DraftsReady
                    && self.drafts.iter().any(|draft| {
                        matches!(
                            draft.status,
                            StagedLorebookDraftStatus::Pending
                                | StagedLorebookDraftStatus::Drafting
                        )
                    })
                {
                    Err(StagedLorebookError::InvalidDraft)
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        }
    }
}

fn validate_drafts(
    drafts: &[StagedLorebookEntryDraft],
    outline: &[StagedLorebookEntryPlan],
) -> Result<(), StagedLorebookError> {
    if drafts.len() != outline.len()
        || drafts.iter().zip(outline).any(|(draft, plan)| {
            draft.plan_id != plan.id
                || match draft.status {
                    StagedLorebookDraftStatus::Pending => {
                        draft.title != plan.title
                            || draft.keywords != plan.proposed_keys
                            || !draft.content.is_empty()
                            || draft.always_active
                            || !draft.revisions.is_empty()
                    }
                    StagedLorebookDraftStatus::Drafted => {
                        validate_written_draft(draft, plan.id).is_err()
                    }
                    StagedLorebookDraftStatus::Drafting | StagedLorebookDraftStatus::Failed => {
                        draft.title != plan.title
                            || draft.keywords != plan.proposed_keys
                            || !draft.content.is_empty()
                            || draft.always_active
                            || !draft.revisions.is_empty()
                    }
                }
        })
    {
        return Err(StagedLorebookError::InvalidOutline);
    }
    Ok(())
}

fn validate_written_draft(
    draft: &StagedLorebookEntryDraft,
    plan_id: LorebookEntryId,
) -> Result<(), StagedLorebookError> {
    if draft.plan_id != plan_id
        || draft.status != StagedLorebookDraftStatus::Drafted
        || draft.title.trim().is_empty()
        || draft.title != draft.title.trim()
        || draft.content.trim().is_empty()
        || draft.content != draft.content.trim()
        || draft
            .keywords
            .iter()
            .any(|keyword| keyword.is_empty() || keyword != keyword.trim())
        || !draft.revisions.is_empty()
    {
        return Err(StagedLorebookError::InvalidDraft);
    }
    Ok(())
}

fn validate_outline(
    outline: &[StagedLorebookEntryPlan],
    excerpts: &[StagedLorebookSourceExcerpt],
) -> Result<(), StagedLorebookError> {
    if outline.is_empty() {
        return Err(StagedLorebookError::InvalidOutline);
    }
    let source_ids = excerpts
        .iter()
        .map(|excerpt| excerpt.source_id.as_str())
        .collect::<HashSet<_>>();
    let mut plan_ids = HashSet::with_capacity(outline.len());
    for (ordinal, plan) in outline.iter().enumerate() {
        if usize::try_from(plan.ordinal).ok() != Some(ordinal)
            || plan.title.trim().is_empty()
            || plan.title != plan.title.trim()
            || plan.category.trim().is_empty()
            || plan.category != plan.category.trim()
            || plan.rationale != plan.rationale.trim()
            || !plan_ids.insert(plan.id)
            || plan
                .source_refs
                .iter()
                .any(|source| !source_ids.contains(source.as_str()))
        {
            return Err(StagedLorebookError::InvalidOutline);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, arguments: Value) -> ProposedToolCall {
        ProposedToolCall {
            provider_call_id: None,
            name: name.into(),
            arguments,
            raw_arguments: None,
            provider_replay: None,
        }
    }

    #[test]
    fn create_and_planner_transitions_preserve_legacy_boundaries() {
        let project = StagedLorebookProject::create(
            CreationWorkflowId::new(),
            "  Harbour world  ".into(),
            Some("  Harbour Canon  ".into()),
            1,
            vec![StagedLorebookSourceExcerpt {
                source_id: "src_01".into(),
                label: "Notes".into(),
                content: "Ada keeps the harbour key.".into(),
            }],
            TimestampMillis::new(10),
        )
        .expect("create project");
        assert_eq!(project.target_count, MIN_STAGED_LOREBOOK_TARGET_COUNT);
        assert_eq!(project.brief, "Harbour world");
        let planning = project
            .start_planning(TimestampMillis::new(11))
            .expect("start planning");
        let reviewed = planning
            .submit_outline(
                vec![StagedLorebookEntryPlan {
                    id: LorebookEntryId::new(),
                    ordinal: 0,
                    title: "Harbour Key".into(),
                    category: "Artifact".into(),
                    proposed_keys: vec!["key".into()],
                    rationale: "Recurring durable object".into(),
                    source_refs: vec!["src_01".into()],
                }],
                TimestampMillis::new(12),
            )
            .expect("submit outline");
        assert_eq!(reviewed.stage, StagedLorebookStage::AwaitingOutlineApproval);
        assert_eq!(reviewed.revision, Revision::new(3));
        let drafting = reviewed
            .approve_outline(TimestampMillis::new(13))
            .expect("approve outline");
        assert_eq!(drafting.stage, StagedLorebookStage::Drafting);
        assert_eq!(drafting.revision, Revision::new(4));
        assert_eq!(drafting.drafts.len(), 1);
        assert_eq!(drafting.drafts[0].plan_id, drafting.outline[0].id);
        assert_eq!(drafting.drafts[0].title, "Harbour Key");
        assert_eq!(drafting.drafts[0].keywords, ["key"]);
        assert!(drafting.drafts[0].content.is_empty());
        assert!(!drafting.drafts[0].always_active);
        assert_eq!(
            drafting.drafts[0].status,
            StagedLorebookDraftStatus::Pending
        );
        assert!(drafting.drafts[0].revisions.is_empty());
    }

    #[test]
    fn outline_requires_stable_order_and_owned_source_refs() {
        let project = StagedLorebookProject::create(
            CreationWorkflowId::new(),
            "World".into(),
            None,
            50,
            Vec::new(),
            TimestampMillis::new(10),
        )
        .expect("create project")
        .start_planning(TimestampMillis::new(11))
        .expect("start planning");
        assert_eq!(
            project.submit_outline(
                vec![StagedLorebookEntryPlan {
                    id: LorebookEntryId::new(),
                    ordinal: 1,
                    title: "Entry".into(),
                    category: "Fact".into(),
                    proposed_keys: Vec::new(),
                    rationale: "Reason".into(),
                    source_refs: vec!["foreign".into()],
                }],
                TimestampMillis::new(12),
            ),
            Err(StagedLorebookError::InvalidOutline)
        );
    }

    #[test]
    fn planner_contract_and_reducer_copy_legacy_shapes_without_an_extra_cap() {
        let request = staged_lorebook_planner_tool_request();
        assert_eq!(request.choice, ToolChoice::Required);
        assert_eq!(request.definitions.len(), 1);
        assert_eq!(
            request.definitions[0].name,
            STAGED_LOREBOOK_PLANNER_TOOL_NAME
        );
        let project = StagedLorebookProject::create(
            CreationWorkflowId::new(),
            "World".into(),
            None,
            5,
            vec![StagedLorebookSourceExcerpt {
                source_id: "src_01".into(),
                label: "Notes".into(),
                content: "Text".into(),
            }],
            TimestampMillis::new(1),
        )
        .expect("project");
        let entries = (0..6)
            .map(|ordinal| {
                json!({
                    "title": format!(" Entry {ordinal} "),
                    "category": if ordinal == 0 { "" } else { "fact" },
                    "proposed_keys": [" key ", ""],
                    "rationale": "",
                    "source_refs": ["src_01"]
                })
            })
            .collect::<Vec<_>>();
        let outline = reduce_staged_lorebook_planner_calls(
            &project,
            &[
                call("ignored", json!({})),
                call(
                    STAGED_LOREBOOK_PLANNER_TOOL_NAME,
                    json!({"entries": entries}),
                ),
            ],
        )
        .expect("outline");
        assert_eq!(outline.len(), 6);
        assert_eq!(outline[0].category, "other");
        assert_eq!(outline[0].proposed_keys, ["key"]);
        assert_eq!(outline[0].source_refs, ["src_01"]);
        assert_eq!(
            outline[0].id,
            LorebookEntryId::from_uuid(Uuid::new_v5(&project.id.as_uuid(), b"outline-0"))
        );
    }

    #[test]
    fn writer_contract_and_reducer_copy_legacy_shape_without_extra_limits() {
        let request = staged_lorebook_writer_tool_request();
        assert_eq!(request.choice, ToolChoice::Required);
        assert_eq!(request.definitions.len(), 1);
        assert_eq!(
            request.definitions[0].name,
            STAGED_LOREBOOK_WRITER_TOOL_NAME
        );
        assert_eq!(
            request.definitions[0].parameters["required"],
            json!(["title", "content"])
        );
        let plan_id = LorebookEntryId::new();
        let keywords = (0..30)
            .map(|ordinal| Value::String(format!(" key {} ", ordinal % 2)))
            .chain([Value::String(String::new())])
            .collect::<Vec<_>>();
        let draft = reduce_staged_lorebook_writer_calls(
            plan_id,
            &[
                call("ignored", json!({})),
                call(
                    STAGED_LOREBOOK_WRITER_TOOL_NAME,
                    json!({
                        "title": " Harbour Key ",
                        "keywords": keywords,
                        "content": " Ada keeps the brass key. ",
                        "always_active": true
                    }),
                ),
                call(
                    STAGED_LOREBOOK_WRITER_TOOL_NAME,
                    json!({"title": "Later", "content": "Ignored"}),
                ),
            ],
        )
        .expect("draft");
        assert_eq!(draft.plan_id, plan_id);
        assert_eq!(draft.title, "Harbour Key");
        assert_eq!(draft.content, "Ada keeps the brass key.");
        assert_eq!(draft.keywords.len(), 30);
        assert_eq!(draft.keywords[0], "key 0");
        assert_eq!(draft.keywords[2], "key 0");
        assert!(draft.always_active);
        assert_eq!(draft.status, StagedLorebookDraftStatus::Drafted);
        assert!(draft.revisions.is_empty());
        assert_eq!(
            reduce_staged_lorebook_writer_calls(
                plan_id,
                &[call(
                    STAGED_LOREBOOK_WRITER_TOOL_NAME,
                    json!({"title": " ", "content": "Body"}),
                )],
            ),
            Err(StagedLorebookError::InvalidDraft)
        );
    }

    #[test]
    fn drafting_batches_keep_the_legacy_three_item_order_and_ready_gate() {
        let project = StagedLorebookProject::create(
            CreationWorkflowId::new(),
            "World".into(),
            None,
            5,
            Vec::new(),
            TimestampMillis::new(1),
        )
        .expect("project")
        .start_planning(TimestampMillis::new(2))
        .expect("planning");
        let outline = (0..5)
            .map(|ordinal| StagedLorebookEntryPlan {
                id: LorebookEntryId::new(),
                ordinal,
                title: format!("Entry {ordinal}"),
                category: "fact".into(),
                proposed_keys: vec![format!("key-{ordinal}")],
                rationale: String::new(),
                source_refs: Vec::new(),
            })
            .collect::<Vec<_>>();
        let mut project = project
            .submit_outline(outline, TimestampMillis::new(3))
            .expect("outline")
            .approve_outline(TimestampMillis::new(4))
            .expect("approve");
        project = project
            .start_draft_batch(TimestampMillis::new(5))
            .expect("first batch");
        assert_eq!(
            project
                .drafts
                .iter()
                .filter(|draft| draft.status == StagedLorebookDraftStatus::Drafting)
                .count(),
            STAGED_LOREBOOK_DRAFT_BATCH_SIZE
        );
        assert!(
            project.drafts[..3]
                .iter()
                .all(|draft| draft.status == StagedLorebookDraftStatus::Drafting)
        );
        assert!(
            project.drafts[3..]
                .iter()
                .all(|draft| draft.status == StagedLorebookDraftStatus::Pending)
        );
        for (timestamp, index) in [(6, 0), (7, 1), (8, 2)] {
            let pending = project.drafts[index].clone();
            project = if index == 1 {
                project
                    .fail_draft(pending.plan_id, TimestampMillis::new(timestamp))
                    .expect("fail")
            } else {
                project
                    .settle_draft(
                        StagedLorebookEntryDraft {
                            plan_id: pending.plan_id,
                            title: pending.title,
                            keywords: pending.keywords,
                            content: "Body".into(),
                            always_active: false,
                            status: StagedLorebookDraftStatus::Drafted,
                            revisions: Vec::new(),
                        },
                        TimestampMillis::new(timestamp),
                    )
                    .expect("settle")
            };
        }
        assert_eq!(project.stage, StagedLorebookStage::Drafting);
        project = project
            .start_draft_batch(TimestampMillis::new(9))
            .expect("retry batch");
        assert_eq!(
            project.drafts[1].status,
            StagedLorebookDraftStatus::Drafting
        );
        assert_eq!(
            project.drafts[3].status,
            StagedLorebookDraftStatus::Drafting
        );
        assert_eq!(
            project.drafts[4].status,
            StagedLorebookDraftStatus::Drafting
        );
        for (timestamp, index) in [(10, 1), (11, 3), (12, 4)] {
            let pending = project.drafts[index].clone();
            project = project
                .settle_draft(
                    StagedLorebookEntryDraft {
                        plan_id: pending.plan_id,
                        title: pending.title,
                        keywords: pending.keywords,
                        content: "Body".into(),
                        always_active: false,
                        status: StagedLorebookDraftStatus::Drafted,
                        revisions: Vec::new(),
                    },
                    TimestampMillis::new(timestamp),
                )
                .expect("settle retry batch");
        }
        assert_eq!(project.stage, StagedLorebookStage::DraftsReady);
    }
}
