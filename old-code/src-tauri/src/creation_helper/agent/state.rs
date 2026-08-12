#![allow(dead_code)]

use serde::Serialize;
use serde_json::Value;

use crate::creation_helper::types::{CreationGoal, CreationSession, DraftCharacter};

#[derive(Clone, Debug)]
pub struct DraftView {
    pub target_kind: TargetKind,
    pub rendered: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetKind {
    Character,
    Persona,
    Lorebook,
}

impl TargetKind {
    pub fn from_goal(goal: &CreationGoal) -> Self {
        match goal {
            CreationGoal::Character => TargetKind::Character,
            CreationGoal::Persona => TargetKind::Persona,
            CreationGoal::Lorebook => TargetKind::Lorebook,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TargetKind::Character => "character",
            TargetKind::Persona => "persona",
            TargetKind::Lorebook => "lorebook",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct PlanStep {
    pub index: usize,
    pub tool_call_id: String,
    pub verb: String,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub raw_args: Value,
    pub args_text: String,
    pub status: StepStatus,
    pub before_summary: Option<String>,
    pub after_summary: Option<String>,
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub extra: Value,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

#[derive(Clone, Debug)]
pub struct AgentTurn {
    pub plan: Vec<PlanStep>,
    pub iterations: u32,
    pub max_iterations: u32,
    pub finished: bool,
    pub user_segments: Vec<String>,
    pub blocks: Vec<TurnBlock>,
}

#[derive(Clone, Debug)]
pub enum TurnBlock {
    Text(String),
    Tool(String), // tool_call_id
}

impl AgentTurn {
    pub fn new(max_iterations: u32) -> Self {
        Self {
            plan: Vec::new(),
            iterations: 0,
            max_iterations,
            finished: false,
            user_segments: Vec::new(),
            blocks: Vec::new(),
        }
    }

    pub fn push_text(&mut self, text: impl Into<String>) {
        let trimmed = text.into().trim().to_string();
        if trimmed.is_empty() {
            return;
        }
        self.user_segments.push(trimmed.clone());
        if let Some(TurnBlock::Text(t)) = self.blocks.last_mut() {
            t.push_str("\n\n");
            t.push_str(&trimmed);
        } else {
            self.blocks.push(TurnBlock::Text(trimmed));
        }
    }

    pub fn push_tool(&mut self, tool_call_id: impl Into<String>) {
        self.blocks.push(TurnBlock::Tool(tool_call_id.into()));
    }
}

#[derive(Clone, Debug)]
pub struct StepResult {
    pub success: bool,
    pub summary: String,
    pub user_facing: Option<String>,
    pub error: Option<String>,
    pub terminal: bool,
    pub extra: Value,
}

impl StepResult {
    pub fn ok(summary: impl Into<String>) -> Self {
        Self {
            success: true,
            summary: summary.into(),
            user_facing: None,
            error: None,
            terminal: false,
            extra: Value::Null,
        }
    }

    pub fn failed(error: impl Into<String>) -> Self {
        let err = error.into();
        Self {
            success: false,
            summary: format!("error: {}", err),
            user_facing: None,
            error: Some(err),
            terminal: false,
            extra: Value::Null,
        }
    }

    pub fn terminal_ok(summary: impl Into<String>, user_facing: Option<String>) -> Self {
        Self {
            success: true,
            summary: summary.into(),
            user_facing,
            error: None,
            terminal: true,
            extra: Value::Null,
        }
    }

    pub fn with_extra(mut self, extra: Value) -> Self {
        self.extra = extra;
        self
    }
}

pub fn build_draft_view(session: &CreationSession) -> DraftView {
    let kind = TargetKind::from_goal(&session.creation_goal);
    let mut rendered = render_draft(kind, &session.draft);
    let images = crate::creation_helper::service::get_all_uploaded_images_metadata(&session.id)
        .unwrap_or_default();
    if !images.is_empty() {
        let active_avatar = session.draft.avatar_path.as_deref();
        rendered.push_str("  generated_images:\n");
        for (idx, img) in images.iter().enumerate() {
            let active = match active_avatar {
                Some(active) if active == img.id => " [active avatar]",
                _ => "",
            };
            rendered.push_str(&format!(
                "    [{}] id={} mime={}{}\n",
                idx + 1,
                img.id,
                img.mime_type,
                active
            ));
        }
    }
    DraftView {
        target_kind: kind,
        rendered,
    }
}

fn render_draft(kind: TargetKind, draft: &DraftCharacter) -> String {
    let mut out = String::new();
    out.push_str(&format!("DRAFT ({}):\n", kind.label()));
    out.push_str(&format!(
        "  name: {}\n",
        draft.name.as_deref().unwrap_or("<unset>")
    ));
    if let Some(def) = draft.definition.as_deref() {
        out.push_str(&format!(
            "  definition: <{} chars> {}\n",
            def.chars().count(),
            preview(def, 80)
        ));
    } else {
        out.push_str("  definition: <unset>\n");
    }
    if !draft.scenes.is_empty() {
        out.push_str("  scenes:\n");
        for sc in &draft.scenes {
            out.push_str(&format!(
                "    {} <{} chars> {}\n",
                sc.id,
                sc.content.chars().count(),
                preview(&sc.content, 60)
            ));
        }
    }
    if let Some(p) = draft.avatar_path.as_deref() {
        out.push_str(&format!("  avatar: {}\n", p));
    }
    if let Some(p) = draft.background_image_path.as_deref() {
        out.push_str(&format!("  background: {}\n", p));
    }
    if let Some(m) = draft.default_model_id.as_deref() {
        out.push_str(&format!("  model: {}\n", m));
    }
    if let Some(p) = draft.prompt_template_id.as_deref() {
        out.push_str(&format!("  prompt: {}\n", p));
    }
    if draft.disable_avatar_gradient {
        out.push_str("  avatar_gradient: off\n");
    }
    out
}

fn preview(s: &str, max: usize) -> String {
    let trimmed: String = s.chars().take(max).collect();
    if s.chars().count() > max {
        format!("\"{}…\"", trimmed.replace('\n', " "))
    } else {
        format!("\"{}\"", trimmed.replace('\n', " "))
    }
}
