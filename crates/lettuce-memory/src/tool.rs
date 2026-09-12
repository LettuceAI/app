use std::collections::HashSet;

use lettuce_conversations::{ToolChoice, ToolDefinition, ToolRequest};
use lettuce_types::{MemoryId, MessageId, TimestampMillis, ToolExecutionId};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    MemoryCategory, MemoryChangeSet, MemoryItem, MemoryPolicy, MemoryShortId, MemorySpaceSnapshot,
    MemoryValidationError, Score,
};

const TOOL_VERSION: u32 = 1;
const MAX_DONE_SUMMARY_BYTES: usize = 4096;
const MAX_SUPERSEDED_MEMORIES: usize = 40;

/// Runtime catalog keys for the text the memory tool contract sends to a model.
pub const DYNAMIC_MEMORY_TOOL_TEXT_KEYS: [&str; 18] = [
    "memory_create_tool",
    "memory_create_tool_group",
    "memory_text_parameter",
    "memory_important_parameter",
    "memory_category_parameter",
    "memory_source_message_parameter",
    "memory_supersedes_parameter",
    "memory_delete_tool",
    "memory_delete_text_parameter",
    "memory_delete_text_parameter_group",
    "memory_delete_confidence_parameter",
    "memory_pin_tool",
    "memory_pin_tool_group",
    "memory_pin_id_parameter",
    "memory_unpin_tool",
    "memory_unpin_id_parameter",
    "memory_done_tool",
    "memory_done_summary_parameter",
];

/// Which legacy memory tool contract a run uses. Group chats use the legacy
/// group contract, which has no source attribution or supersession.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DynamicMemoryToolOptions {
    pub group: bool,
    pub supersession_enabled: bool,
    pub require_source_message_id: bool,
}

/// The legacy memory tool contract; `text` resolves a runtime catalog key.
#[must_use]
pub fn dynamic_memory_tool_request_for_run(
    options: DynamicMemoryToolOptions,
    text: &dyn Fn(&str) -> String,
) -> ToolRequest {
    let variant = |direct: &str, group: &str| text(if options.group { group } else { direct });
    let mut create_properties = json!({
        "text": { "type": "string", "description": text("memory_text_parameter") },
        "important": { "type": "boolean", "description": text("memory_important_parameter") },
        "category": {
            "type": "string",
            "enum": ["character_trait", "relationship", "plot_event", "world_detail", "preference", "other"],
            "description": text("memory_category_parameter")
        }
    });
    if !options.group {
        create_properties["source_message_id"] = json!({
            "type": "string",
            "description": text("memory_source_message_parameter")
        });
    }
    if options.supersession_enabled && !options.group {
        create_properties["supersedes"] = json!({
            "type": "array",
            "items": { "type": "string" },
            "description": text("memory_supersedes_parameter")
        });
    }
    let mut create_required = vec!["text", "category"];
    if options.require_source_message_id && !options.group {
        create_required.push("source_message_id");
    }
    let id_parameter = |key: &str| {
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": text(key) }
            },
            "required": ["id"]
        })
    };
    ToolRequest {
        definitions: vec![
            ToolDefinition {
                name: "create_memory".to_string(),
                description: Some(variant("memory_create_tool", "memory_create_tool_group")),
                parameters: json!({
                    "type": "object",
                    "properties": create_properties,
                    "required": create_required
                }),
                version: TOOL_VERSION,
            },
            ToolDefinition {
                name: "delete_memory".to_string(),
                description: Some(text("memory_delete_tool")),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": variant(
                                "memory_delete_text_parameter",
                                "memory_delete_text_parameter_group",
                            )
                        },
                        "confidence": {
                            "type": "number",
                            "description": text("memory_delete_confidence_parameter")
                        }
                    },
                    "required": ["text"]
                }),
                version: TOOL_VERSION,
            },
            ToolDefinition {
                name: "pin_memory".to_string(),
                description: Some(variant("memory_pin_tool", "memory_pin_tool_group")),
                parameters: id_parameter("memory_pin_id_parameter"),
                version: TOOL_VERSION,
            },
            ToolDefinition {
                name: "unpin_memory".to_string(),
                description: Some(text("memory_unpin_tool")),
                parameters: id_parameter("memory_unpin_id_parameter"),
                version: TOOL_VERSION,
            },
            ToolDefinition {
                name: "done".to_string(),
                description: Some(text("memory_done_tool")),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "summary": {
                            "type": "string",
                            "description": text("memory_done_summary_parameter")
                        }
                    },
                    "required": []
                }),
                version: TOOL_VERSION,
            },
        ],
        choice: ToolChoice::Required,
    }
}

/// The contract with every description removed, for comparing a frozen run
/// request against the options it was built from.
#[must_use]
pub fn dynamic_memory_tool_shape(request: &ToolRequest) -> ToolRequest {
    fn strip(value: &mut Value) {
        match value {
            Value::Object(object) => {
                object.remove("description");
                object.values_mut().for_each(strip);
            }
            Value::Array(values) => values.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let mut shape = request.clone();
    for definition in &mut shape.definitions {
        definition.description = None;
        strip(&mut definition.parameters);
    }
    shape
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "name", content = "arguments", rename_all = "snake_case")]
pub enum MemoryToolArguments {
    CreateMemory {
        text: String,
        category: CategoryArgument,
        important: bool,
        source_message_id: Option<MessageId>,
        supersedes: Vec<MemoryReference>,
    },
    DeleteMemory {
        target: MemoryReference,
        confidence: Option<Score>,
    },
    PinMemory {
        target: MemoryReference,
    },
    UnpinMemory {
        target: MemoryReference,
    },
    Done {
        summary: Option<String>,
    },
    /// A call whose arguments cannot be applied; it settles as `Skipped`
    /// instead of failing the round, as legacy skipped such calls.
    Unusable {
        reason: MemoryToolSkipReason,
    },
}

/// The category a create named. Legacy validated it only after the duplicate
/// check, so an untagged or mistagged create is still reported as a duplicate
/// when its text already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CategoryArgument {
    Tagged { category: MemoryCategory },
    Missing,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryToolSkipReason {
    MissingText,
    InvalidText,
    EmptyText,
    TextTooLong,
    RefusalText,
    MetaText,
    MissingCategory,
    InvalidCategory,
    MissingTarget,
    UnsupportedTool,
    MalformedArguments,
}

/// A memory as the model named it: a six-digit short id, a stable id, or (for
/// deletes) the exact memory text. It resolves against the items current when
/// the call applies, as legacy did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MemoryReference(pub String);

impl MemoryReference {
    fn cleaned(&self) -> &str {
        self.0
            .trim()
            .trim_matches(|character| {
                matches!(character, '#' | '*' | '"' | '\'' | '[' | ']' | '(' | ')')
            })
            .trim()
    }

    fn resolve_id(&self, items: &[MemoryItem]) -> Option<usize> {
        let cleaned = self.cleaned();
        if let Some(short_id) = MemoryShortId::parse(cleaned) {
            return items.iter().position(|item| item.short_id == short_id);
        }
        let id = cleaned.parse::<MemoryId>().ok()?;
        items.iter().position(|item| item.id == id)
    }

    fn resolve_id_or_text(&self, items: &[MemoryItem]) -> Option<usize> {
        self.resolve_id(items)
            .or_else(|| items.iter().position(|item| item.text == self.0))
    }
}

impl MemoryToolArguments {
    /// Like `parse`, but an unusable call becomes `Unusable` with its reason.
    #[must_use]
    pub fn parse_or_skip(name: &str, arguments: &Value) -> Self {
        Self::parse(name, arguments).unwrap_or_else(|error| Self::Unusable {
            reason: match error {
                MemoryToolError::MissingField("text") if name == "delete_memory" => {
                    MemoryToolSkipReason::MissingTarget
                }
                MemoryToolError::MissingField("text") => MemoryToolSkipReason::MissingText,
                MemoryToolError::MissingField("category") => MemoryToolSkipReason::MissingCategory,
                MemoryToolError::MissingField("id") => MemoryToolSkipReason::MissingTarget,
                MemoryToolError::InvalidCategory => MemoryToolSkipReason::InvalidCategory,
                MemoryToolError::UnsupportedTool => MemoryToolSkipReason::UnsupportedTool,
                MemoryToolError::Validation(_) => MemoryToolSkipReason::InvalidText,
                MemoryToolError::Text(problem) => match problem {
                    crate::MemoryTextProblem::Empty => MemoryToolSkipReason::EmptyText,
                    crate::MemoryTextProblem::TooLong => MemoryToolSkipReason::TextTooLong,
                    crate::MemoryTextProblem::Refusal => MemoryToolSkipReason::RefusalText,
                    crate::MemoryTextProblem::Meta => MemoryToolSkipReason::MetaText,
                },
                _ => MemoryToolSkipReason::MalformedArguments,
            },
        })
    }

    /// Reads arguments as leniently as legacy: unknown keys are ignored and
    /// optional fields with the wrong shape fall back to their defaults.
    pub fn parse(name: &str, arguments: &Value) -> Result<Self, MemoryToolError> {
        let object = arguments
            .as_object()
            .ok_or(MemoryToolError::ArgumentsMustBeObject)?;
        match name {
            "create_memory" => {
                let text = crate::normalize_memory_text(&required_string(object, "text")?)
                    .map_err(MemoryToolError::Text)?;
                let category = match object
                    .get("category")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|category| !category.is_empty())
                {
                    None => CategoryArgument::Missing,
                    Some(category) => MemoryCategory::parse(category)
                        .map_or(CategoryArgument::Invalid, |category| {
                            CategoryArgument::Tagged { category }
                        }),
                };
                let important = object
                    .get("important")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let source_message_id = object
                    .get("source_message_id")
                    .and_then(Value::as_str)
                    .and_then(|value| value.trim().parse().ok());
                let supersedes = object
                    .get("supersedes")
                    .and_then(Value::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(Value::as_str)
                            .map(|value| MemoryReference(value.to_owned()))
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(Self::CreateMemory {
                    text: text.trim().to_string(),
                    category,
                    important,
                    source_message_id,
                    supersedes,
                })
            }
            "delete_memory" => {
                let target = MemoryReference(required_string(object, "text")?);
                let confidence = object
                    .get("confidence")
                    .and_then(Value::as_f64)
                    .filter(|value| value.is_finite())
                    .and_then(|value| Score::from_ratio(value.clamp(0.0, 1.0)).ok());
                Ok(Self::DeleteMemory { target, confidence })
            }
            "pin_memory" => Ok(Self::PinMemory {
                target: MemoryReference(required_string(object, "id")?),
            }),
            "unpin_memory" => Ok(Self::UnpinMemory {
                target: MemoryReference(required_string(object, "id")?),
            }),
            "done" => {
                let summary = object.get("summary").and_then(Value::as_str).map(|value| {
                    let mut summary = value.trim();
                    while summary.len() > MAX_DONE_SUMMARY_BYTES {
                        let mut end = MAX_DONE_SUMMARY_BYTES;
                        while !summary.is_char_boundary(end) {
                            end -= 1;
                        }
                        summary = &summary[..end];
                    }
                    summary.to_string()
                });
                Ok(Self::Done { summary })
            }
            _ => Err(MemoryToolError::UnsupportedTool),
        }
    }
}

fn required_string(
    object: &serde_json::Map<String, Value>,
    key: &'static str,
) -> Result<String, MemoryToolError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or(MemoryToolError::MissingField(key))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateMemoryPreparation {
    pub id: MemoryId,
    pub token_count: u32,
    pub created_at: TimestampMillis,
    /// Optional qualified evidence supplied by the embedding coordinator. The
    /// reducer verifies both its policy qualification and live target.
    pub semantic_duplicate: Option<SemanticDuplicateEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticDuplicateEvidence {
    pub existing_id: MemoryId,
    pub source_revision: String,
    pub dimensions: u16,
    pub cosine_score: Score,
    pub threshold: Score,
}

impl SemanticDuplicateEvidence {
    fn is_qualified(&self) -> bool {
        !self.source_revision.trim().is_empty()
            && self.source_revision.len() <= 128
            && matches!(self.dimensions, 64 | 128 | 256 | 512 | 768)
            && self.cosine_score >= self.threshold
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryToolCall {
    pub execution_id: ToolExecutionId,
    pub arguments: MemoryToolArguments,
    pub create: Option<CreateMemoryPreparation>,
    pub source_role: Option<lettuce_conversations::MessageRole>,
    pub observed_at: Option<TimestampMillis>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoftDeleteReason {
    LowConfidence,
    HardDeleteLimitReached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryToolRejection {
    CreateNotPrepared,
    PreparedIdAlreadyExists,
    InvalidSemanticDuplicateEvidence,
}

/// One memory as the model sees it in a tool result: the six-digit id and the
/// text, in item order, superseded items excluded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListedMemory {
    pub short_id: MemoryShortId,
    pub text: String,
}

#[must_use]
pub fn list_memories(items: &[MemoryItem]) -> Vec<ListedMemory> {
    items
        .iter()
        .filter(|item| item.superseded_by.is_none())
        .map(|item| ListedMemory {
            short_id: item.short_id,
            text: item.text.clone(),
        })
        .collect()
}

/// Which duplicate check matched, as legacy reported it back to the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DuplicateKind {
    NormalizedText,
    Semantic { cosine: Score, threshold: Score },
    LexicalOverlap,
}

/// What a call did, with the facts legacy echoed to the model afterwards: the
/// six-digit id, a deleted memory's text and the memory list right after the
/// call applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MemoryToolOutcome {
    Created {
        id: MemoryId,
        short_id: MemoryShortId,
        memories: Vec<ListedMemory>,
    },
    DuplicateSkipped {
        existing_id: MemoryId,
        kind: DuplicateKind,
    },
    Deleted {
        id: MemoryId,
        short_id: MemoryShortId,
        text: String,
        memories: Vec<ListedMemory>,
    },
    SoftDeleted {
        id: MemoryId,
        short_id: MemoryShortId,
        text: String,
        reason: SoftDeleteReason,
        memories: Vec<ListedMemory>,
    },
    Pinned {
        id: MemoryId,
        short_id: MemoryShortId,
    },
    Unpinned {
        id: MemoryId,
        short_id: MemoryShortId,
    },
    TargetNotFound {
        reference: MemoryReference,
    },
    Done {
        summary: Option<String>,
    },
    Rejected {
        reason: MemoryToolRejection,
    },
    Skipped {
        reason: MemoryToolSkipReason,
    },
    StoppedAfterDone,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryToolResult {
    pub execution_id: ToolExecutionId,
    pub outcome: MemoryToolOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryBatchResult {
    pub change: Option<MemoryChangeSet>,
    pub results: Vec<MemoryToolResult>,
}

/// Legacy allowed `floor(initial_count * ratio).max(1)` hard deletes per cycle,
/// counting every memory (cold included) at cycle start and every hard delete
/// across the cycle's rounds; a cycle over an empty space allows none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryCycleBudget {
    pub hard_delete_limit: usize,
    pub hard_deletes_used: usize,
}

impl MemoryCycleBudget {
    #[must_use]
    pub fn new(initial_count: usize, ratio: Score, hard_deletes_used: usize) -> Self {
        let hard_delete_limit = if initial_count == 0 {
            0
        } else {
            (initial_count.saturating_mul(usize::from(ratio.basis_points())) / 10_000).max(1)
        };
        Self {
            hard_delete_limit,
            hard_deletes_used,
        }
    }

    /// The budget of a cycle starting from `snapshot` with no hard delete yet.
    #[must_use]
    pub fn fresh(snapshot: &MemorySpaceSnapshot, policy: &MemoryPolicy) -> Self {
        Self::new(
            snapshot.items.len(),
            policy.max_hard_delete_ratio_per_cycle,
            0,
        )
    }

    /// How many hard deletes the settled results already spent.
    #[must_use]
    pub fn count_hard_deletes(results: &[MemoryToolResult]) -> usize {
        results
            .iter()
            .filter(|result| matches!(result.outcome, MemoryToolOutcome::Deleted { .. }))
            .count()
    }
}

/// The once-per-cycle policy pass legacy ran after the loop and the repair
/// pass: trim to `max_entries`, then demote to the hot token budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryCycleFinish {
    pub change: Option<MemoryChangeSet>,
    pub trimmed_ids: Vec<MemoryId>,
    pub demoted_ids: Vec<MemoryId>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MemoryToolReducer;

impl MemoryToolReducer {
    /// One round of a cycle that starts at `snapshot`.
    pub fn reduce(
        &self,
        snapshot: &MemorySpaceSnapshot,
        policy: &MemoryPolicy,
        calls: &[MemoryToolCall],
    ) -> Result<MemoryBatchResult, MemoryToolError> {
        self.reduce_round(
            snapshot,
            policy,
            MemoryCycleBudget::fresh(snapshot, policy),
            calls,
        )
    }

    pub fn reduce_round(
        &self,
        snapshot: &MemorySpaceSnapshot,
        policy: &MemoryPolicy,
        budget: MemoryCycleBudget,
        calls: &[MemoryToolCall],
    ) -> Result<MemoryBatchResult, MemoryToolError> {
        snapshot.validate()?;
        policy.validate()?;

        let original_items = snapshot.items.clone();
        let mut items = original_items.clone();
        let hard_delete_limit = budget.hard_delete_limit;
        let mut hard_delete_count = budget.hard_deletes_used;
        let mut stopped = false;
        let mut results = Vec::with_capacity(calls.len());

        for call in calls {
            let outcome = if stopped {
                MemoryToolOutcome::StoppedAfterDone
            } else {
                match &call.arguments {
                    MemoryToolArguments::CreateMemory {
                        text,
                        category,
                        important,
                        source_message_id,
                        supersedes,
                    } => apply_create(
                        &mut items,
                        text,
                        *category,
                        *important,
                        supersedes,
                        (*source_message_id, call.source_role, call.observed_at),
                        call.create.clone(),
                    ),
                    MemoryToolArguments::DeleteMemory { target, confidence } => apply_delete(
                        &mut items,
                        target,
                        confidence.unwrap_or(policy.delete_confidence_default),
                        policy,
                        hard_delete_limit,
                        &mut hard_delete_count,
                    ),
                    MemoryToolArguments::PinMemory { target } => {
                        apply_pin(&mut items, target, true)
                    }
                    MemoryToolArguments::UnpinMemory { target } => {
                        apply_pin(&mut items, target, false)
                    }
                    MemoryToolArguments::Done { summary } => {
                        stopped = true;
                        MemoryToolOutcome::Done {
                            summary: summary.clone(),
                        }
                    }
                    MemoryToolArguments::Unusable { reason } => {
                        MemoryToolOutcome::Skipped { reason: *reason }
                    }
                }
            };
            results.push(MemoryToolResult {
                execution_id: call.execution_id,
                outcome,
            });
        }

        ensure_pinned_hot(&mut items);
        let change = (items != original_items).then_some(MemoryChangeSet {
            space_id: snapshot.id,
            expected_revision: snapshot.revision,
            items,
        });
        if let Some(change) = &change {
            change.validate()?;
        }

        Ok(MemoryBatchResult { change, results })
    }

    pub fn finish_cycle(
        &self,
        snapshot: &MemorySpaceSnapshot,
        policy: &MemoryPolicy,
    ) -> Result<MemoryCycleFinish, MemoryToolError> {
        snapshot.validate()?;
        policy.validate()?;
        let mut items = snapshot.items.clone();
        ensure_pinned_hot(&mut items);
        let trimmed_ids = trim_to_capacity(&mut items, policy.max_entries);
        let demoted_ids = enforce_hot_budget(&mut items, policy.hot_token_budget);
        let change = (items != snapshot.items).then_some(MemoryChangeSet {
            space_id: snapshot.id,
            expected_revision: snapshot.revision,
            items,
        });
        if let Some(change) = &change {
            change.validate()?;
        }
        Ok(MemoryCycleFinish {
            change,
            trimmed_ids,
            demoted_ids,
        })
    }
}

fn apply_create(
    items: &mut Vec<MemoryItem>,
    text: &str,
    category: CategoryArgument,
    important: bool,
    requested_supersedes: &[MemoryReference],
    observed_context: (
        Option<MessageId>,
        Option<lettuce_conversations::MessageRole>,
        Option<TimestampMillis>,
    ),
    preparation: Option<CreateMemoryPreparation>,
) -> MemoryToolOutcome {
    let (source_message_id, source_role, observed_at) = observed_context;
    let Some(preparation) = preparation else {
        return MemoryToolOutcome::Rejected {
            reason: MemoryToolRejection::CreateNotPrepared,
        };
    };
    if items.iter().any(|item| item.id == preparation.id) {
        return MemoryToolOutcome::Rejected {
            reason: MemoryToolRejection::PreparedIdAlreadyExists,
        };
    }
    if preparation
        .semantic_duplicate
        .as_ref()
        .is_some_and(|evidence| !evidence.is_qualified())
    {
        return MemoryToolOutcome::Rejected {
            reason: MemoryToolRejection::InvalidSemanticDuplicateEvidence,
        };
    }
    if let Some((existing_id, kind)) =
        duplicate_id(text, preparation.semantic_duplicate.as_ref(), items)
    {
        return MemoryToolOutcome::DuplicateSkipped { existing_id, kind };
    }
    let category = match category {
        CategoryArgument::Tagged { category } => category,
        CategoryArgument::Missing => {
            return MemoryToolOutcome::Skipped {
                reason: MemoryToolSkipReason::MissingCategory,
            };
        }
        CategoryArgument::Invalid => {
            return MemoryToolOutcome::Skipped {
                reason: MemoryToolSkipReason::InvalidCategory,
            };
        }
    };

    let mut supersedes = Vec::new();
    for reference in requested_supersedes {
        if let Some(index) = reference.resolve_id(items) {
            let id = items[index].id;
            if id != preparation.id
                && items[index].superseded_by.is_none()
                && !supersedes.contains(&id)
            {
                supersedes.push(id);
            }
        }
    }
    let short_id = MemoryShortId::allocate(preparation.id, |candidate| {
        items.iter().any(|item| item.short_id == candidate)
    });
    items.push(MemoryItem {
        id: preparation.id,
        short_id,
        text: text.to_string(),
        category,
        source_message_id,
        source_role,
        observed_at,
        observed_time_precision: observed_at.map(|_| "turn".to_owned()),
        superseded_by: None,
        superseded_at: None,
        supersedes: supersedes.clone(),
        token_count: preparation.token_count,
        is_cold: false,
        is_pinned: important,
        importance: Score::FULL,
        persistence_importance: Score::FULL,
        prompt_importance: Score::FULL,
        volatility: Score::LEGACY_VOLATILITY,
        access_count: 0,
        created_at: preparation.created_at,
        last_accessed_at: preparation.created_at,
    });
    if !supersedes.is_empty() {
        for item in items.iter_mut() {
            if item.id != preparation.id
                && item.superseded_by.is_none()
                && supersedes.contains(&item.id)
            {
                item.superseded_by = Some(preparation.id);
                item.superseded_at = Some(preparation.created_at);
            }
        }
        enforce_superseded_cap(items, MAX_SUPERSEDED_MEMORIES);
    }
    MemoryToolOutcome::Created {
        id: preparation.id,
        short_id,
        memories: list_memories(items),
    }
}

fn enforce_superseded_cap(items: &mut Vec<MemoryItem>, cap: usize) {
    let count = items
        .iter()
        .filter(|item| item.superseded_by.is_some())
        .count();
    if count <= cap {
        return;
    }
    let mut order = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.superseded_by.is_some())
        .map(|(index, item)| {
            (
                index,
                item.superseded_at.unwrap_or(TimestampMillis::UNIX_EPOCH),
            )
        })
        .collect::<Vec<_>>();
    order.sort_by_key(|(_, at)| *at);
    let drop = order
        .into_iter()
        .take(count - cap)
        .map(|(index, _)| index)
        .collect::<HashSet<_>>();
    let mut index = 0;
    items.retain(|_| {
        let keep = !drop.contains(&index);
        index += 1;
        keep
    });
}

fn duplicate_id(
    candidate: &str,
    semantic_duplicate: Option<&SemanticDuplicateEvidence>,
    items: &[MemoryItem],
) -> Option<(MemoryId, DuplicateKind)> {
    if let Some(evidence) = semantic_duplicate {
        if items.iter().any(|item| item.id == evidence.existing_id) {
            return Some((
                evidence.existing_id,
                DuplicateKind::Semantic {
                    cosine: evidence.cosine_score,
                    threshold: evidence.threshold,
                },
            ));
        }
    }
    let normalized_candidate = normalize_text(candidate);
    let candidate_word_count = normalized_candidate.split_whitespace().count();
    items.iter().find_map(|item| {
        let normalized_existing = normalize_text(&item.text);
        if !normalized_candidate.is_empty() && normalized_candidate == normalized_existing {
            return Some((item.id, DuplicateKind::NormalizedText));
        }
        (candidate_word_count >= 3 && lexical_overlap(candidate, &item.text) >= 0.9)
            .then_some((item.id, DuplicateKind::LexicalOverlap))
    })
}

fn normalize_text(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut last_space = false;
    for character in value.chars() {
        if character.is_alphanumeric() {
            normalized.extend(character.to_lowercase());
            last_space = false;
        } else if !last_space {
            normalized.push(' ');
            last_space = true;
        }
    }
    normalized.trim().to_string()
}

fn lexical_overlap(left: &str, right: &str) -> f64 {
    let left = keywords(left);
    let right = keywords(right);
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let shared = left.intersection(&right).count();
    shared as f64 / left.len().max(right.len()) as f64
}

fn keywords(value: &str) -> HashSet<String> {
    normalize_text(value)
        .split_whitespace()
        .filter(|word| word.len() >= 3)
        .map(str::to_string)
        .collect()
}

fn apply_delete(
    items: &mut Vec<MemoryItem>,
    target: &MemoryReference,
    confidence: Score,
    policy: &MemoryPolicy,
    hard_delete_limit: usize,
    hard_delete_count: &mut usize,
) -> MemoryToolOutcome {
    let Some(index) = target.resolve_id_or_text(items) else {
        return MemoryToolOutcome::TargetNotFound {
            reference: target.clone(),
        };
    };
    let id = items[index].id;
    let short_id = items[index].short_id;
    let text = items[index].text.clone();
    let hard_requested = confidence >= Score::HARD_DELETE_THRESHOLD;
    if !hard_requested || *hard_delete_count >= hard_delete_limit {
        items[index].is_cold = true;
        items[index].importance = policy.cold_threshold;
        return MemoryToolOutcome::SoftDeleted {
            id,
            short_id,
            text,
            reason: if hard_requested {
                SoftDeleteReason::HardDeleteLimitReached
            } else {
                SoftDeleteReason::LowConfidence
            },
            memories: list_memories(items),
        };
    }
    items.remove(index);
    *hard_delete_count += 1;
    MemoryToolOutcome::Deleted {
        id,
        short_id,
        text,
        memories: list_memories(items),
    }
}

fn apply_pin(
    items: &mut [MemoryItem],
    target: &MemoryReference,
    pinned: bool,
) -> MemoryToolOutcome {
    let Some(index) = target.resolve_id(items) else {
        return MemoryToolOutcome::TargetNotFound {
            reference: target.clone(),
        };
    };
    let item = &mut items[index];
    let id = item.id;
    let short_id = item.short_id;
    item.is_pinned = pinned;
    if pinned {
        item.is_cold = false;
        item.importance = Score::FULL;
        MemoryToolOutcome::Pinned { id, short_id }
    } else {
        MemoryToolOutcome::Unpinned { id, short_id }
    }
}

fn ensure_pinned_hot(items: &mut [MemoryItem]) {
    for item in items {
        if item.is_pinned && item.is_cold {
            item.is_cold = false;
            item.importance = Score::FULL;
        }
    }
}

fn enforce_hot_budget(items: &mut [MemoryItem], budget: u32) -> Vec<MemoryId> {
    let mut current = items
        .iter()
        .filter(|item| !item.is_cold)
        .fold(0u32, |total, item| total.saturating_add(item.token_count));
    if current <= budget {
        return Vec::new();
    }
    let mut candidates = items
        .iter()
        .enumerate()
        .filter(|(_, item)| !item.is_cold && !item.is_pinned)
        .map(|(index, item)| (index, item.last_accessed_at, item.id))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(_, accessed, id)| (*accessed, *id));

    let mut demoted = Vec::new();
    for (index, _, id) in candidates {
        if current <= budget {
            break;
        }
        items[index].is_cold = true;
        current = current.saturating_sub(items[index].token_count);
        demoted.push(id);
    }
    demoted
}

fn trim_to_capacity(items: &mut Vec<MemoryItem>, max_entries: usize) -> Vec<MemoryId> {
    if items.len() <= max_entries {
        return Vec::new();
    }
    let min_time = items
        .iter()
        .filter(|item| !item.is_pinned)
        .map(|item| item.last_accessed_at.get())
        .min()
        .unwrap_or(0);
    let max_time = items
        .iter()
        .filter(|item| !item.is_pinned)
        .map(|item| item.last_accessed_at.get())
        .max()
        .unwrap_or(min_time);
    let range = max_time.saturating_sub(min_time).max(1) as i128;
    let mut candidates = items
        .iter()
        .filter(|item| !item.is_pinned)
        .map(|item| {
            let elapsed = item.last_accessed_at.get().saturating_sub(min_time) as i128;
            let recency_basis_points = (elapsed * 10_000 / range).clamp(0, 10_000);
            let score = i128::from(item.importance.basis_points()) * 70 + recency_basis_points * 30;
            (score, item.id)
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(score, id)| (*score, *id));

    let remove_count = items.len().saturating_sub(max_entries);
    let remove_ids = candidates
        .into_iter()
        .take(remove_count)
        .map(|(_, id)| id)
        .collect::<HashSet<_>>();
    let mut removed = Vec::with_capacity(remove_ids.len());
    items.retain(|item| {
        if remove_ids.contains(&item.id) {
            removed.push(item.id);
            false
        } else {
            true
        }
    });
    removed
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MemoryToolError {
    #[error("tool arguments must be an object")]
    ArgumentsMustBeObject,
    #[error("tool arguments contain an unknown field")]
    UnknownField,
    #[error("tool argument is missing: {0}")]
    MissingField(&'static str),
    #[error("tool argument is invalid: {0}")]
    InvalidField(&'static str),
    #[error("memory category is invalid")]
    InvalidCategory,
    #[error("memory text was not kept: {0:?}")]
    Text(crate::MemoryTextProblem),
    #[error("done summary is too large")]
    SummaryTooLarge,
    #[error("tool is unsupported")]
    UnsupportedTool,
    #[error("memory validation failed: {0}")]
    Validation(#[from] MemoryValidationError),
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use lettuce_types::{
        MemoryId, MemorySpaceId, MessageId, Revision, TimestampMillis, ToolExecutionId,
    };
    use serde_json::{Value, json};

    use super::{
        CategoryArgument, CreateMemoryPreparation, DuplicateKind, DynamicMemoryToolOptions,
        MemoryCycleBudget, MemoryReference, MemoryToolArguments, MemoryToolCall, MemoryToolError,
        MemoryToolOutcome, MemoryToolReducer, MemoryToolSkipReason, SoftDeleteReason,
        dynamic_memory_tool_request_for_run,
    };
    use crate::{
        MemoryCategory, MemoryItem, MemoryPolicy, MemoryShortId, MemorySpaceSnapshot, Score,
    };

    fn tool_request(
        group: bool,
        supersession_enabled: bool,
        require_source_message_id: bool,
    ) -> lettuce_conversations::ToolRequest {
        dynamic_memory_tool_request_for_run(
            DynamicMemoryToolOptions {
                group,
                supersession_enabled,
                require_source_message_id,
            },
            &|key| key.to_owned(),
        )
    }

    fn reference(id: MemoryId) -> MemoryReference {
        MemoryReference(id.to_string())
    }

    fn score(points: u16) -> Score {
        match Score::from_basis_points(points) {
            Some(score) => score,
            None => panic!("test score must be valid"),
        }
    }

    fn policy() -> MemoryPolicy {
        MemoryPolicy {
            max_entries: 3,
            hot_token_budget: 20,
            cold_threshold: score(2_000),
            delete_confidence_default: score(5_000),
            max_hard_delete_ratio_per_cycle: score(5_000),
        }
    }

    fn item(text: &str, tokens: u32, accessed: i64, pinned: bool) -> MemoryItem {
        let id = MemoryId::new();
        MemoryItem {
            id,
            short_id: MemoryShortId::derived(id),
            text: text.to_string(),
            category: MemoryCategory::Other,
            source_message_id: None,
            source_role: None,
            observed_at: None,
            observed_time_precision: None,
            superseded_by: None,
            superseded_at: None,
            supersedes: Vec::new(),
            token_count: tokens,
            is_cold: false,
            is_pinned: pinned,
            importance: Score::FULL,
            persistence_importance: Score::FULL,
            prompt_importance: Score::FULL,
            volatility: Score::LEGACY_VOLATILITY,
            access_count: 0,
            created_at: TimestampMillis::new(accessed),
            last_accessed_at: TimestampMillis::new(accessed),
        }
    }

    fn snapshot(items: Vec<MemoryItem>) -> MemorySpaceSnapshot {
        MemorySpaceSnapshot {
            id: MemorySpaceId::new(),
            revision: Revision::INITIAL,
            items,
        }
    }

    fn call(arguments: MemoryToolArguments) -> MemoryToolCall {
        MemoryToolCall {
            execution_id: ToolExecutionId::new(),
            arguments,
            create: None,
            source_role: None,
            observed_at: None,
        }
    }

    #[test]
    fn declarations_are_versioned_and_required() {
        let request = tool_request(false, false, false);
        assert!(request.validate().is_ok());
        assert!(
            request
                .definitions
                .iter()
                .all(|definition| definition.version == 1)
        );
        assert!(matches!(
            request.choice,
            lettuce_conversations::ToolChoice::Required
        ));

        let create_required = request.definitions[0].parameters["required"]
            .as_array()
            .expect("required fields");
        assert!(
            !create_required
                .iter()
                .any(|field| field == "source_message_id")
        );

        let time_aware = tool_request(false, false, true);
        let create_required = time_aware.definitions[0].parameters["required"]
            .as_array()
            .expect("required fields");
        assert!(
            create_required
                .iter()
                .any(|field| field == "source_message_id")
        );

        assert!(
            tool_request(false, true, false).definitions[0].parameters["properties"]
                .get("supersedes")
                .is_some()
        );
        assert!(
            tool_request(false, false, false).definitions[0].parameters["properties"]
                .get("supersedes")
                .is_none()
        );
    }

    #[test]
    fn legacy_references_resolve_by_short_id_or_exact_text() {
        let first = item("Mira likes tea.", 2, 1, false);
        let second = item("Mira keeps a brass key.", 2, 2, false);
        let third = item("Mira fears storms.", 2, 3, false);
        let (first_short, second_id, third_short) = (first.short_id, second.id, third.short_id);
        let state = snapshot(vec![first, second, third]);
        let calls = vec![
            call(MemoryToolArguments::PinMemory {
                target: MemoryReference(format!("#{first_short}")),
            }),
            call(MemoryToolArguments::DeleteMemory {
                target: MemoryReference("Mira keeps a brass key.".into()),
                confidence: Some(Score::FULL),
            }),
            call(MemoryToolArguments::DeleteMemory {
                target: MemoryReference(format!("[{third_short}]")),
                confidence: Some(score(1_000)),
            }),
            call(MemoryToolArguments::UnpinMemory {
                target: MemoryReference("999999x".into()),
            }),
        ];
        let result = MemoryToolReducer
            .reduce(&state, &policy(), &calls)
            .expect("reduce");
        assert!(matches!(
            result.results[0].outcome,
            MemoryToolOutcome::Pinned { .. }
        ));
        assert!(matches!(
            result.results[1].outcome,
            MemoryToolOutcome::Deleted { id, .. } if id == second_id
        ));
        assert!(matches!(
            result.results[2].outcome,
            MemoryToolOutcome::SoftDeleted { .. }
        ));
        assert!(matches!(
            result.results[3].outcome,
            MemoryToolOutcome::TargetNotFound { .. }
        ));
    }

    #[test]
    fn short_ids_are_six_digits_and_probe_past_collisions() {
        let id = MemoryId::new();
        let derived = MemoryShortId::derived(id);
        assert_eq!(derived.to_string().len(), 6);
        assert_eq!(MemoryShortId::parse(&derived.to_string()), Some(derived));
        assert_eq!(MemoryShortId::parse("12345"), None);
        assert_eq!(MemoryShortId::parse("12345a"), None);
        let next = MemoryShortId::allocate(id, |candidate| candidate == derived);
        assert_eq!(next.get(), (derived.get() + 1) % MemoryShortId::SPACE);
        let group = tool_request(true, true, true);
        assert!(
            group.definitions[0].parameters["properties"]
                .get("source_message_id")
                .is_none()
        );
        assert!(
            group.definitions[0].parameters["properties"]
                .get("supersedes")
                .is_none()
        );
        assert_eq!(
            group.definitions[0].parameters["required"],
            json!(["text", "category"])
        );
        assert_eq!(
            MemoryToolArguments::parse(
                "delete_memory",
                &json!({ "text": "123456", "confidence": 1.5, "reason": "stale" })
            ),
            Ok(MemoryToolArguments::DeleteMemory {
                target: MemoryReference("123456".into()),
                confidence: Some(Score::FULL),
            })
        );
        assert!(matches!(
            MemoryToolArguments::parse(
                "create_memory",
                &json!({
                    "text": "Mira likes tea.",
                    "category": "preference",
                    "source_message_id": "1",
                    "important": "yes",
                    "note": "extra"
                })
            ),
            Ok(MemoryToolArguments::CreateMemory {
                source_message_id: None,
                important: false,
                ..
            })
        ));
    }

    #[test]
    fn created_memory_text_is_normalized_like_legacy() {
        let parsed = MemoryToolArguments::parse(
            "create_memory",
            &json!({
                "text": "```\n<think>plan</think> Mira   likes\n tea \n```",
                "category": "other"
            }),
        )
        .expect("create");
        assert!(matches!(
            parsed,
            MemoryToolArguments::CreateMemory { ref text, .. } if text == "Mira likes tea"
        ));
    }

    #[test]
    fn unusable_calls_settle_as_skipped_instead_of_failing() {
        let cases = [
            (
                "create_memory",
                json!({ "category": "other" }),
                MemoryToolSkipReason::MissingText,
            ),
            (
                "create_memory",
                json!({ "text": "   ", "category": "other" }),
                MemoryToolSkipReason::EmptyText,
            ),
            (
                "create_memory",
                json!({ "text": "a".repeat(281), "category": "other" }),
                MemoryToolSkipReason::TextTooLong,
            ),
            (
                "create_memory",
                json!({ "text": "I can't help with that.", "category": "other" }),
                MemoryToolSkipReason::RefusalText,
            ),
            (
                "create_memory",
                json!({ "text": "As an AI, Mira likes tea.", "category": "other" }),
                MemoryToolSkipReason::MetaText,
            ),
            ("pin_memory", json!({}), MemoryToolSkipReason::MissingTarget),
            (
                "retag_memory",
                json!({}),
                MemoryToolSkipReason::UnsupportedTool,
            ),
            (
                "delete_memory",
                json!("123456"),
                MemoryToolSkipReason::MalformedArguments,
            ),
        ];
        let calls = cases
            .iter()
            .map(|(name, arguments, _)| call(MemoryToolArguments::parse_or_skip(name, arguments)))
            .collect::<Vec<_>>();
        let result = MemoryToolReducer
            .reduce(
                &snapshot(vec![item("Mira likes tea.", 2, 1, false)]),
                &policy(),
                &calls,
            )
            .expect("reduce");
        assert_eq!(result.results.len(), cases.len());
        assert_eq!(
            MemoryToolArguments::parse_or_skip("delete_memory", &json!({ "confidence": 0.9 })),
            MemoryToolArguments::Unusable {
                reason: MemoryToolSkipReason::MissingTarget
            }
        );
        for (result, (_, _, reason)) in result.results.iter().zip(&cases) {
            assert_eq!(
                result.outcome,
                MemoryToolOutcome::Skipped { reason: *reason }
            );
        }
        assert!(result.change.is_none());
    }

    #[test]
    fn arguments_keep_legacy_references() {
        let parsed = MemoryToolArguments::parse(
            "delete_memory",
            &json!({ "text": "[123456]", "confidence": 0.75 }),
        );
        assert_eq!(
            parsed,
            Ok(MemoryToolArguments::DeleteMemory {
                target: MemoryReference("[123456]".into()),
                confidence: Some(score(7_500)),
            })
        );
        assert!(MemoryToolArguments::parse("delete_memory", &json!({ "id": "123456" })).is_err());

        let source_message_id = MessageId::new();
        assert_eq!(
            MemoryToolArguments::parse(
                "create_memory",
                &json!({
                    "text": "Mira made a promise.",
                    "category": "relationship",
                    "source_message_id": source_message_id,
                }),
            ),
            Ok(MemoryToolArguments::CreateMemory {
                text: "Mira made a promise.".to_owned(),
                category: CategoryArgument::Tagged {
                    category: MemoryCategory::Relationship,
                },
                important: false,
                source_message_id: Some(source_message_id),
                supersedes: Vec::new(),
            })
        );
    }

    #[test]
    fn create_preserves_source_message_attribution() {
        let source_message_id = MessageId::new();
        let created_id = MemoryId::new();
        let mut create = call(MemoryToolArguments::CreateMemory {
            text: "Mira made a promise.".to_owned(),
            category: CategoryArgument::Tagged {
                category: MemoryCategory::Relationship,
            },
            important: false,
            source_message_id: Some(source_message_id),
            supersedes: Vec::new(),
        });
        create.create = Some(CreateMemoryPreparation {
            id: created_id,
            token_count: 4,
            created_at: TimestampMillis::new(2),
            semantic_duplicate: None,
        });

        let result = MemoryToolReducer
            .reduce(&snapshot(vec![]), &policy(), &[create])
            .expect("reduce attributed create");
        let stored = result
            .change
            .expect("create change")
            .items
            .into_iter()
            .find(|item| item.id == created_id)
            .expect("created memory");
        assert_eq!(stored.source_message_id, Some(source_message_id));
    }

    #[test]
    fn create_probes_past_an_existing_short_id() {
        let created_id = MemoryId::new();
        let mut existing = item("Mira likes tea.", 2, 1, false);
        existing.short_id = MemoryShortId::derived(created_id);
        let mut create = call(MemoryToolArguments::CreateMemory {
            text: "Mira owns a lighthouse.".to_owned(),
            category: CategoryArgument::Tagged {
                category: MemoryCategory::WorldDetail,
            },
            important: false,
            source_message_id: None,
            supersedes: Vec::new(),
        });
        create.create = Some(CreateMemoryPreparation {
            id: created_id,
            token_count: 4,
            created_at: TimestampMillis::new(2),
            semantic_duplicate: None,
        });
        let result = MemoryToolReducer
            .reduce(&snapshot(vec![existing]), &policy(), &[create])
            .expect("reduce create");
        let created = result
            .change
            .expect("create change")
            .items
            .into_iter()
            .find(|item| item.id == created_id)
            .expect("created memory");
        assert_eq!(
            created.short_id.get(),
            (MemoryShortId::derived(created_id).get() + 1) % MemoryShortId::SPACE
        );
    }

    #[test]
    fn create_supersedes_only_existing_active_memories() {
        let active = item("Mira lives in Ankara.", 4, 1, false);
        let active_id = active.id;
        let mut already_superseded = item("Mira lived in Izmir.", 4, 1, false);
        let already_superseded_id = already_superseded.id;
        already_superseded.superseded_by = Some(MemoryId::new());
        already_superseded.superseded_at = Some(TimestampMillis::new(1));
        let existing_replacement = already_superseded.superseded_by;
        let created_id = MemoryId::new();
        let mut create = call(MemoryToolArguments::CreateMemory {
            text: "Mira lives in Berlin.".to_owned(),
            category: CategoryArgument::Tagged {
                category: MemoryCategory::WorldDetail,
            },
            important: false,
            source_message_id: None,
            supersedes: vec![
                reference(active_id),
                reference(already_superseded_id),
                reference(MemoryId::new()),
            ],
        });
        create.create = Some(CreateMemoryPreparation {
            id: created_id,
            token_count: 4,
            created_at: TimestampMillis::new(2),
            semantic_duplicate: None,
        });

        let result = MemoryToolReducer
            .reduce(
                &snapshot(vec![active, already_superseded]),
                &policy(),
                &[create],
            )
            .expect("supersede");
        let items = result.change.expect("change").items;
        let active = items.iter().find(|item| item.id == active_id).expect("old");
        assert_eq!(active.superseded_by, Some(created_id));
        assert_eq!(active.superseded_at, Some(TimestampMillis::new(2)));
        let created = items
            .iter()
            .find(|item| item.id == created_id)
            .expect("new");
        assert_eq!(created.supersedes, vec![active_id]);
        assert_eq!(
            items
                .iter()
                .find(|item| item.id == already_superseded_id)
                .expect("already superseded")
                .superseded_by,
            existing_replacement
        );
    }

    #[test]
    fn superseded_history_keeps_the_latest_forty_entries() {
        let mut items = (0..41)
            .map(|at| {
                let mut item = item("historical memory", 1, at, false);
                item.superseded_by = Some(MemoryId::new());
                item.superseded_at = Some(TimestampMillis::new(at));
                item
            })
            .collect::<Vec<_>>();
        let oldest = items[0].id;
        let second_oldest = items[1].id;
        let active = item("current location", 1, 42, false);
        let active_id = active.id;
        items.push(active);
        let mut create = call(MemoryToolArguments::CreateMemory {
            text: "new location".to_owned(),
            category: CategoryArgument::Tagged {
                category: MemoryCategory::WorldDetail,
            },
            important: false,
            source_message_id: None,
            supersedes: vec![reference(active_id)],
        });
        create.create = Some(CreateMemoryPreparation {
            id: MemoryId::new(),
            token_count: 1,
            created_at: TimestampMillis::new(43),
            semantic_duplicate: None,
        });
        let result = MemoryToolReducer
            .reduce(
                &snapshot(items),
                &MemoryPolicy {
                    max_entries: 100,
                    hot_token_budget: 1_000,
                    ..policy()
                },
                &[create],
            )
            .expect("cap superseded history");
        let items = result.change.expect("change").items;
        assert_eq!(
            items
                .iter()
                .filter(|item| item.superseded_by.is_some())
                .count(),
            40
        );
        assert!(!items.iter().any(|item| item.id == oldest));
        assert!(!items.iter().any(|item| item.id == second_oldest));
    }

    #[test]
    fn create_skips_normalized_and_semantic_duplicates() {
        let existing = item("Mira likes the old harbor.", 4, 1, false);
        let existing_id = existing.id;
        let state = snapshot(vec![existing]);
        let create_id = MemoryId::new();
        let mut first = call(MemoryToolArguments::CreateMemory {
            text: "mira likes the old harbor".to_string(),
            category: CategoryArgument::Tagged {
                category: MemoryCategory::Preference,
            },
            important: false,
            source_message_id: None,
            supersedes: Vec::new(),
        });
        first.create = Some(CreateMemoryPreparation {
            id: create_id,
            token_count: 5,
            created_at: TimestampMillis::new(2),
            semantic_duplicate: None,
        });
        let result = MemoryToolReducer.reduce(&state, &policy(), &[first]);
        let result = match result {
            Ok(result) => result,
            Err(error) => panic!("reduction failed: {error}"),
        };
        assert!(result.change.is_none());
        assert!(matches!(
            result.results[0].outcome,
            MemoryToolOutcome::DuplicateSkipped {
                existing_id: id,
                kind: DuplicateKind::NormalizedText
            } if id == existing_id
        ));

        let mut semantic = call(MemoryToolArguments::CreateMemory {
            text: "She enjoys visiting the docks".to_string(),
            category: CategoryArgument::Tagged {
                category: MemoryCategory::Preference,
            },
            important: false,
            source_message_id: None,
            supersedes: Vec::new(),
        });
        semantic.create = Some(CreateMemoryPreparation {
            id: MemoryId::new(),
            token_count: 5,
            created_at: TimestampMillis::new(2),
            semantic_duplicate: Some(super::SemanticDuplicateEvidence {
                existing_id,
                source_revision: "v4-test".to_owned(),
                dimensions: 768,
                cosine_score: score(9_500),
                threshold: score(9_000),
            }),
        });
        let result = MemoryToolReducer.reduce(&state, &policy(), &[semantic]);
        let result = match result {
            Ok(result) => result,
            Err(error) => panic!("reduction failed: {error}"),
        };
        assert!(result.change.is_none());
        assert_eq!(
            result.results[0].outcome,
            MemoryToolOutcome::DuplicateSkipped {
                existing_id,
                kind: DuplicateKind::Semantic {
                    cosine: score(9_500),
                    threshold: score(9_000),
                },
            }
        );
    }

    #[test]
    fn category_is_trimmed_before_the_allow_list_and_blank_means_missing() {
        assert!(matches!(
            MemoryToolArguments::parse(
                "create_memory",
                &json!({"text": "Mira likes tea", "category": " preference "})
            ),
            Ok(MemoryToolArguments::CreateMemory {
                category: CategoryArgument::Tagged {
                    category: MemoryCategory::Preference,
                },
                ..
            })
        ));
        assert!(matches!(
            MemoryToolArguments::parse(
                "create_memory",
                &json!({"text": "Mira likes tea", "category": "   "})
            ),
            Ok(MemoryToolArguments::CreateMemory {
                category: CategoryArgument::Missing,
                ..
            })
        ));
        assert!(matches!(
            MemoryToolArguments::parse(
                "create_memory",
                &json!({"text": "Mira likes tea", "category": "mood"})
            ),
            Ok(MemoryToolArguments::CreateMemory {
                category: CategoryArgument::Invalid,
                ..
            })
        ));
        assert!(matches!(
            MemoryToolArguments::parse("create_memory", &json!({"category": "other"})),
            Err(MemoryToolError::MissingField("text"))
        ));
    }

    #[test]
    fn untagged_creates_are_checked_for_duplicates_before_their_category() {
        let existing = item("Mira likes tea.", 4, 1, false);
        let existing_id = existing.id;
        let state = snapshot(vec![existing]);
        let mut duplicate = call(MemoryToolArguments::CreateMemory {
            text: "mira likes tea".to_string(),
            category: CategoryArgument::Missing,
            important: false,
            source_message_id: None,
            supersedes: Vec::new(),
        });
        duplicate.create = Some(CreateMemoryPreparation {
            id: MemoryId::new(),
            token_count: 3,
            created_at: TimestampMillis::new(2),
            semantic_duplicate: None,
        });
        let mut untagged = call(MemoryToolArguments::CreateMemory {
            text: "The captain trusts Mira.".to_string(),
            category: CategoryArgument::Invalid,
            important: false,
            source_message_id: None,
            supersedes: Vec::new(),
        });
        untagged.create = Some(CreateMemoryPreparation {
            id: MemoryId::new(),
            token_count: 4,
            created_at: TimestampMillis::new(2),
            semantic_duplicate: None,
        });
        let result = MemoryToolReducer
            .reduce(&state, &policy(), &[duplicate, untagged])
            .expect("reduce");
        assert!(result.change.is_none());
        assert!(matches!(
            result.results[0].outcome,
            MemoryToolOutcome::DuplicateSkipped { existing_id: id, .. } if id == existing_id
        ));
        assert_eq!(
            result.results[1].outcome,
            MemoryToolOutcome::Skipped {
                reason: MemoryToolSkipReason::InvalidCategory
            }
        );
    }

    #[test]
    fn mutation_results_list_the_memories_right_after_each_call() {
        let first = item("Mira likes the old harbor.", 4, 1, false);
        let second = item("The lighthouse keeper is her uncle.", 6, 2, false);
        let mut superseded = item("Mira lived in the capital.", 5, 3, false);
        superseded.superseded_by = Some(first.id);
        superseded.superseded_at = Some(TimestampMillis::new(3));
        let first_short = first.short_id;
        let second_id = second.id;
        let second_short = second.short_id;
        let state = snapshot(vec![first, second, superseded]);
        let create_id = MemoryId::new();
        let mut create = call(MemoryToolArguments::CreateMemory {
            text: "Mira moved to the coast last spring.".to_string(),
            category: CategoryArgument::Tagged {
                category: MemoryCategory::PlotEvent,
            },
            important: false,
            source_message_id: None,
            supersedes: Vec::new(),
        });
        create.create = Some(CreateMemoryPreparation {
            id: create_id,
            token_count: 7,
            created_at: TimestampMillis::new(4),
            semantic_duplicate: None,
        });
        let calls = vec![
            create,
            call(MemoryToolArguments::DeleteMemory {
                target: reference(second_id),
                confidence: Some(Score::FULL),
            }),
        ];
        let result = MemoryToolReducer
            .reduce(
                &state,
                &MemoryPolicy {
                    max_entries: 10,
                    hot_token_budget: 1_000,
                    ..policy()
                },
                &calls,
            )
            .expect("reduction");
        let created_short = match &result.results[0].outcome {
            MemoryToolOutcome::Created {
                id,
                short_id,
                memories,
            } => {
                assert_eq!(*id, create_id);
                assert_eq!(
                    memories
                        .iter()
                        .map(|memory| memory.short_id)
                        .collect::<Vec<_>>(),
                    vec![first_short, second_short, *short_id]
                );
                assert_eq!(memories[2].text, "Mira moved to the coast last spring.");
                *short_id
            }
            other => panic!("unexpected outcome {other:?}"),
        };
        match &result.results[1].outcome {
            MemoryToolOutcome::Deleted {
                id,
                short_id,
                text,
                memories,
            } => {
                assert_eq!(*id, second_id);
                assert_eq!(*short_id, second_short);
                assert_eq!(text, "The lighthouse keeper is her uncle.");
                assert_eq!(
                    memories
                        .iter()
                        .map(|memory| memory.short_id)
                        .collect::<Vec<_>>(),
                    vec![first_short, created_short]
                );
            }
            other => panic!("unexpected outcome {other:?}"),
        }
    }

    #[test]
    fn create_rejects_unqualified_semantic_duplicate_evidence() {
        let existing = item("existing", 4, 1, false);
        let mut create = call(MemoryToolArguments::CreateMemory {
            text: "different memory".to_owned(),
            category: CategoryArgument::Tagged {
                category: MemoryCategory::Other,
            },
            important: false,
            source_message_id: None,
            supersedes: Vec::new(),
        });
        create.create = Some(CreateMemoryPreparation {
            id: MemoryId::new(),
            token_count: 3,
            created_at: TimestampMillis::new(2),
            semantic_duplicate: Some(super::SemanticDuplicateEvidence {
                existing_id: existing.id,
                source_revision: "v4-test".to_owned(),
                dimensions: 768,
                cosine_score: score(8_000),
                threshold: score(9_000),
            }),
        });
        let result = match MemoryToolReducer.reduce(&snapshot(vec![existing]), &policy(), &[create])
        {
            Ok(result) => result,
            Err(error) => panic!("reduction failed: {error}"),
        };
        assert!(matches!(
            result.results[0].outcome,
            MemoryToolOutcome::Rejected {
                reason: super::MemoryToolRejection::InvalidSemanticDuplicateEvidence
            }
        ));
        assert!(result.change.is_none());
    }

    #[test]
    fn delete_safeguard_soft_deletes_after_cycle_limit() {
        let first = item("first", 2, 1, false);
        let second = item("second", 2, 2, false);
        let third = item("third", 2, 3, false);
        let first_id = first.id;
        let second_id = second.id;
        let state = snapshot(vec![first, second, third]);
        let calls = vec![
            call(MemoryToolArguments::DeleteMemory {
                target: reference(first_id),
                confidence: Some(Score::FULL),
            }),
            call(MemoryToolArguments::DeleteMemory {
                target: reference(second_id),
                confidence: Some(Score::FULL),
            }),
        ];
        let result = match MemoryToolReducer.reduce(&state, &policy(), &calls) {
            Ok(result) => result,
            Err(error) => panic!("reduction failed: {error}"),
        };
        assert!(matches!(
            result.results[0].outcome,
            MemoryToolOutcome::Deleted { .. }
        ));
        assert!(matches!(
            result.results[1].outcome,
            MemoryToolOutcome::SoftDeleted {
                reason: SoftDeleteReason::HardDeleteLimitReached,
                ..
            }
        ));
    }

    #[test]
    fn hard_delete_budget_counts_every_cycle_start_item_and_spans_rounds() {
        let first = item("first", 2, 1, false);
        let second = item("second", 2, 2, false);
        let first_id = first.id;
        let second_id = second.id;
        let mut cold_one = item("cold one", 2, 3, false);
        cold_one.is_cold = true;
        let mut cold_two = item("cold two", 2, 4, false);
        cold_two.is_cold = true;
        let state = snapshot(vec![first, second, cold_one, cold_two]);
        let calls = vec![
            call(MemoryToolArguments::DeleteMemory {
                target: reference(first_id),
                confidence: Some(Score::FULL),
            }),
            call(MemoryToolArguments::DeleteMemory {
                target: reference(second_id),
                confidence: Some(Score::FULL),
            }),
        ];
        let budget = MemoryCycleBudget::fresh(&state, &policy());
        assert_eq!(budget.hard_delete_limit, 2);
        let result = MemoryToolReducer
            .reduce(&state, &policy(), &calls)
            .expect("reduce");
        assert!(matches!(
            result.results[0].outcome,
            MemoryToolOutcome::Deleted { .. }
        ));
        assert!(matches!(
            result.results[1].outcome,
            MemoryToolOutcome::Deleted { .. }
        ));
        assert_eq!(MemoryCycleBudget::count_hard_deletes(&result.results), 2);

        let later_round = MemoryToolReducer
            .reduce_round(
                &state,
                &policy(),
                MemoryCycleBudget::new(4, score(5_000), 2),
                &calls[..1],
            )
            .expect("reduce");
        assert!(matches!(
            later_round.results[0].outcome,
            MemoryToolOutcome::SoftDeleted {
                reason: SoftDeleteReason::HardDeleteLimitReached,
                ..
            }
        ));
        assert_eq!(
            MemoryCycleBudget::new(0, score(5_000), 0).hard_delete_limit,
            0
        );
        assert_eq!(
            MemoryCycleBudget::new(1, score(1_000), 0).hard_delete_limit,
            1
        );
        assert_eq!(
            MemoryCycleBudget::new(10, score(3_000), 0).hard_delete_limit,
            3
        );
    }

    #[test]
    fn done_stops_later_calls_and_pins_survive_policy() {
        let pinned = item("pinned", 50, 0, true);
        let pinned_id = pinned.id;
        let old = item("old", 15, 1, false);
        let state = snapshot(vec![pinned, old]);
        let result = match MemoryToolReducer.reduce(
            &state,
            &policy(),
            &[
                call(MemoryToolArguments::Done { summary: None }),
                call(MemoryToolArguments::UnpinMemory {
                    target: reference(pinned_id),
                }),
            ],
        ) {
            Ok(result) => result,
            Err(error) => panic!("reduction failed: {error}"),
        };
        assert!(matches!(
            result.results[1].outcome,
            MemoryToolOutcome::StoppedAfterDone
        ));
        assert!(result.change.is_none());
        let finish = MemoryToolReducer
            .finish_cycle(&state, &policy())
            .expect("finish");
        let change = finish
            .change
            .expect("budget policy should demote the old item");
        assert_eq!(finish.demoted_ids.len(), 1);
        let pinned = change.items.iter().find(|item| item.id == pinned_id);
        assert!(pinned.is_some_and(|item| item.is_pinned && !item.is_cold));
        assert!(
            change
                .items
                .iter()
                .any(|item| item.text == "old" && item.is_cold)
        );
    }

    #[test]
    fn pin_and_unpin_are_ordered_and_missing_targets_are_explicit() {
        let existing = item("existing", 2, 1, false);
        let existing_id = existing.id;
        let missing_id = MemoryId::new();
        let result = match MemoryToolReducer.reduce(
            &snapshot(vec![existing]),
            &policy(),
            &[
                call(MemoryToolArguments::PinMemory {
                    target: reference(existing_id),
                }),
                call(MemoryToolArguments::UnpinMemory {
                    target: reference(existing_id),
                }),
                call(MemoryToolArguments::PinMemory {
                    target: reference(missing_id),
                }),
            ],
        ) {
            Ok(result) => result,
            Err(error) => panic!("reduction failed: {error}"),
        };
        assert!(matches!(
            result.results[0].outcome,
            MemoryToolOutcome::Pinned { id, .. } if id == existing_id
        ));
        assert!(matches!(
            result.results[1].outcome,
            MemoryToolOutcome::Unpinned { id, .. } if id == existing_id
        ));
        assert!(matches!(
            result.results[2].outcome,
            MemoryToolOutcome::TargetNotFound { reference: ref missing } if *missing == reference(missing_id)
        ));
    }

    #[test]
    fn capacity_trimming_preserves_pinned_items() {
        let pinned = item("pinned", 2, 0, true);
        let pinned_id = pinned.id;
        let weakest = item("weakest", 2, 1, false);
        let weakest_id = weakest.id;
        let mut stronger = item("stronger", 2, 2, false);
        stronger.importance = score(8_000);
        let newest = item("newest", 2, 3, false);
        let state = snapshot(vec![pinned, weakest, stronger, newest]);
        let rounds_keep_everything = MemoryToolReducer
            .reduce(&state, &policy(), &[])
            .expect("reduce");
        assert!(rounds_keep_everything.change.is_none());
        let result = match MemoryToolReducer.finish_cycle(&state, &policy()) {
            Ok(result) => result,
            Err(error) => panic!("cycle finish failed: {error}"),
        };
        assert_eq!(result.trimmed_ids, vec![weakest_id]);
        assert!(result.demoted_ids.is_empty());
        let change = match result.change {
            Some(change) => change,
            None => panic!("capacity policy should produce a change"),
        };
        assert!(change.items.iter().any(|item| item.id == pinned_id));
    }

    #[test]
    fn fixture_pins_implemented_core_scenarios() {
        let fixture: Value = match serde_json::from_str(include_str!(
            "../../../fixtures/legacy-import/dynamic-memory-tool-scenarios-v1.json"
        )) {
            Ok(value) => value,
            Err(error) => panic!("fixture must parse: {error}"),
        };
        let ids = fixture["scenarios"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|scenario| scenario["id"].as_str())
            .collect::<HashSet<_>>();
        for required in [
            "multiple_calls_apply_in_provider_order",
            "done_stops_remaining_calls",
            "duplicate_create_is_skipped",
            "low_confidence_delete_demotes",
            "hard_delete_ratio_forces_later_soft_delete",
            "missing_pin_target_is_explicit",
            "pinned_items_survive_budget_and_capacity",
        ] {
            assert!(ids.contains(required), "missing scenario {required}");
        }
    }
}
