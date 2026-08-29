use std::collections::HashSet;

use lettuce_conversations::{ToolChoice, ToolDefinition, ToolRequest};
use lettuce_types::{MemoryId, TimestampMillis, ToolExecutionId};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::model::validate_memory_text;
use crate::{
    MemoryCategory, MemoryChangeSet, MemoryItem, MemoryPolicy, MemorySpaceSnapshot,
    MemoryValidationError, Score,
};

const TOOL_VERSION: u32 = 1;
const MAX_DONE_SUMMARY_BYTES: usize = 4096;

pub fn dynamic_memory_tool_request() -> ToolRequest {
    ToolRequest {
        definitions: vec![
            ToolDefinition {
                name: "create_memory".to_string(),
                description: Some("Create one concise categorized long-term memory.".to_string()),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string" },
                        "category": {
                            "type": "string",
                            "enum": ["character_trait", "relationship", "plot_event", "world_detail", "preference", "other"]
                        },
                        "important": { "type": "boolean" }
                    },
                    "required": ["text", "category"],
                    "additionalProperties": false
                }),
                version: TOOL_VERSION,
            },
            ToolDefinition {
                name: "delete_memory".to_string(),
                description: Some(
                    "Delete a memory by stable ID. Low confidence performs a reversible soft delete."
                        .to_string(),
                ),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "format": "uuid" },
                        "confidence": { "type": "number", "minimum": 0, "maximum": 1 }
                    },
                    "required": ["id"],
                    "additionalProperties": false
                }),
                version: TOOL_VERSION,
            },
            ToolDefinition {
                name: "pin_memory".to_string(),
                description: Some("Pin a memory so policy cannot demote or trim it.".to_string()),
                parameters: id_parameters(),
                version: TOOL_VERSION,
            },
            ToolDefinition {
                name: "unpin_memory".to_string(),
                description: Some("Unpin a memory so normal policy applies.".to_string()),
                parameters: id_parameters(),
                version: TOOL_VERSION,
            },
            ToolDefinition {
                name: "done".to_string(),
                description: Some("Finish the current memory update round.".to_string()),
                parameters: json!({
                    "type": "object",
                    "properties": { "summary": { "type": "string" } },
                    "additionalProperties": false
                }),
                version: TOOL_VERSION,
            },
        ],
        choice: ToolChoice::Required,
    }
}

fn id_parameters() -> Value {
    json!({
        "type": "object",
        "properties": { "id": { "type": "string", "format": "uuid" } },
        "required": ["id"],
        "additionalProperties": false
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "name", content = "arguments", rename_all = "snake_case")]
pub enum MemoryToolArguments {
    CreateMemory {
        text: String,
        category: MemoryCategory,
        important: bool,
    },
    DeleteMemory {
        id: MemoryId,
        confidence: Option<Score>,
    },
    PinMemory {
        id: MemoryId,
    },
    UnpinMemory {
        id: MemoryId,
    },
    Done {
        summary: Option<String>,
    },
}

impl MemoryToolArguments {
    pub fn parse(name: &str, arguments: &Value) -> Result<Self, MemoryToolError> {
        let object = arguments
            .as_object()
            .ok_or(MemoryToolError::ArgumentsMustBeObject)?;
        match name {
            "create_memory" => {
                ensure_keys(object, &["text", "category", "important"])?;
                let text = required_string(object, "text")?;
                validate_memory_text(&text)?;
                let category = match required_string(object, "category")?.as_str() {
                    "character_trait" => MemoryCategory::CharacterTrait,
                    "relationship" => MemoryCategory::Relationship,
                    "plot_event" => MemoryCategory::PlotEvent,
                    "world_detail" => MemoryCategory::WorldDetail,
                    "preference" => MemoryCategory::Preference,
                    "other" => MemoryCategory::Other,
                    _ => return Err(MemoryToolError::InvalidCategory),
                };
                let important = object
                    .get("important")
                    .map(|value| {
                        value
                            .as_bool()
                            .ok_or(MemoryToolError::InvalidField("important"))
                    })
                    .transpose()?
                    .unwrap_or(false);
                Ok(Self::CreateMemory {
                    text: text.trim().to_string(),
                    category,
                    important,
                })
            }
            "delete_memory" => {
                ensure_keys(object, &["id", "confidence"])?;
                let id = parse_id(object, "id")?;
                let confidence = object
                    .get("confidence")
                    .map(|value| {
                        value
                            .as_f64()
                            .ok_or(MemoryToolError::InvalidField("confidence"))
                            .and_then(|value| Score::from_ratio(value).map_err(Into::into))
                    })
                    .transpose()?;
                Ok(Self::DeleteMemory { id, confidence })
            }
            "pin_memory" => {
                ensure_keys(object, &["id"])?;
                Ok(Self::PinMemory {
                    id: parse_id(object, "id")?,
                })
            }
            "unpin_memory" => {
                ensure_keys(object, &["id"])?;
                Ok(Self::UnpinMemory {
                    id: parse_id(object, "id")?,
                })
            }
            "done" => {
                ensure_keys(object, &["summary"])?;
                let summary = object
                    .get("summary")
                    .map(|value| {
                        let summary = value
                            .as_str()
                            .ok_or(MemoryToolError::InvalidField("summary"))?
                            .trim();
                        if summary.len() > MAX_DONE_SUMMARY_BYTES {
                            return Err(MemoryToolError::SummaryTooLarge);
                        }
                        Ok(summary.to_string())
                    })
                    .transpose()?;
                Ok(Self::Done { summary })
            }
            _ => Err(MemoryToolError::UnsupportedTool),
        }
    }
}

fn ensure_keys(
    object: &serde_json::Map<String, Value>,
    allowed: &[&str],
) -> Result<(), MemoryToolError> {
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(MemoryToolError::UnknownField);
    }
    Ok(())
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

fn parse_id(
    object: &serde_json::Map<String, Value>,
    key: &'static str,
) -> Result<MemoryId, MemoryToolError> {
    required_string(object, key)?
        .parse()
        .map_err(|_| MemoryToolError::InvalidField(key))
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MemoryToolOutcome {
    Created {
        id: MemoryId,
    },
    DuplicateSkipped {
        existing_id: MemoryId,
    },
    Deleted {
        id: MemoryId,
    },
    SoftDeleted {
        id: MemoryId,
        reason: SoftDeleteReason,
    },
    Pinned {
        id: MemoryId,
    },
    Unpinned {
        id: MemoryId,
    },
    TargetNotFound {
        id: MemoryId,
    },
    Done {
        summary: Option<String>,
    },
    Rejected {
        reason: MemoryToolRejection,
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
    pub demoted_ids: Vec<MemoryId>,
    pub trimmed_ids: Vec<MemoryId>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MemoryToolReducer;

impl MemoryToolReducer {
    pub fn reduce(
        &self,
        snapshot: &MemorySpaceSnapshot,
        policy: &MemoryPolicy,
        calls: &[MemoryToolCall],
    ) -> Result<MemoryBatchResult, MemoryToolError> {
        snapshot.validate()?;
        policy.validate()?;

        let original_items = snapshot.items.clone();
        let mut items = original_items.clone();
        let initial_active_count = items.iter().filter(|item| !item.is_cold).count();
        let hard_delete_limit =
            hard_delete_limit(initial_active_count, policy.max_hard_delete_ratio_per_cycle);
        let mut hard_delete_count = 0usize;
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
                    } => apply_create(&mut items, text, *category, *important, call.create.clone()),
                    MemoryToolArguments::DeleteMemory { id, confidence } => apply_delete(
                        &mut items,
                        *id,
                        confidence.unwrap_or(policy.delete_confidence_default),
                        policy,
                        hard_delete_limit,
                        &mut hard_delete_count,
                    ),
                    MemoryToolArguments::PinMemory { id } => apply_pin(&mut items, *id, true),
                    MemoryToolArguments::UnpinMemory { id } => apply_pin(&mut items, *id, false),
                    MemoryToolArguments::Done { summary } => {
                        stopped = true;
                        MemoryToolOutcome::Done {
                            summary: summary.clone(),
                        }
                    }
                }
            };
            results.push(MemoryToolResult {
                execution_id: call.execution_id,
                outcome,
            });
        }

        ensure_pinned_hot(&mut items);
        let demoted_ids = enforce_hot_budget(&mut items, policy.hot_token_budget);
        let trimmed_ids = trim_to_capacity(&mut items, policy.max_entries);
        let change = (items != original_items).then_some(MemoryChangeSet {
            space_id: snapshot.id,
            expected_revision: snapshot.revision,
            items,
        });
        if let Some(change) = &change {
            change.validate()?;
        }

        Ok(MemoryBatchResult {
            change,
            results,
            demoted_ids,
            trimmed_ids,
        })
    }
}

fn apply_create(
    items: &mut Vec<MemoryItem>,
    text: &str,
    category: MemoryCategory,
    important: bool,
    preparation: Option<CreateMemoryPreparation>,
) -> MemoryToolOutcome {
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
    if let Some(existing_id) = duplicate_id(text, preparation.semantic_duplicate.as_ref(), items) {
        return MemoryToolOutcome::DuplicateSkipped { existing_id };
    }

    items.push(MemoryItem {
        id: preparation.id,
        text: text.to_string(),
        category,
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
    MemoryToolOutcome::Created { id: preparation.id }
}

fn duplicate_id(
    candidate: &str,
    semantic_duplicate: Option<&SemanticDuplicateEvidence>,
    items: &[MemoryItem],
) -> Option<MemoryId> {
    if let Some(evidence) = semantic_duplicate {
        if items.iter().any(|item| item.id == evidence.existing_id) {
            return Some(evidence.existing_id);
        }
    }
    let normalized_candidate = normalize_text(candidate);
    let candidate_word_count = normalized_candidate.split_whitespace().count();
    items.iter().find_map(|item| {
        let normalized_existing = normalize_text(&item.text);
        if !normalized_candidate.is_empty() && normalized_candidate == normalized_existing {
            return Some(item.id);
        }
        (candidate_word_count >= 3 && lexical_overlap(candidate, &item.text) >= 0.9)
            .then_some(item.id)
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
    id: MemoryId,
    confidence: Score,
    policy: &MemoryPolicy,
    hard_delete_limit: usize,
    hard_delete_count: &mut usize,
) -> MemoryToolOutcome {
    let Some(index) = items.iter().position(|item| item.id == id) else {
        return MemoryToolOutcome::TargetNotFound { id };
    };
    let hard_requested = confidence >= Score::HARD_DELETE_THRESHOLD;
    if !hard_requested || *hard_delete_count >= hard_delete_limit {
        items[index].is_cold = true;
        items[index].importance = policy.cold_threshold;
        return MemoryToolOutcome::SoftDeleted {
            id,
            reason: if hard_requested {
                SoftDeleteReason::HardDeleteLimitReached
            } else {
                SoftDeleteReason::LowConfidence
            },
        };
    }
    items.remove(index);
    *hard_delete_count += 1;
    MemoryToolOutcome::Deleted { id }
}

fn hard_delete_limit(initial_count: usize, ratio: Score) -> usize {
    if initial_count == 0 {
        return 0;
    }
    let scaled = initial_count.saturating_mul(usize::from(ratio.basis_points())) / 10_000;
    scaled.max(1)
}

fn apply_pin(items: &mut [MemoryItem], id: MemoryId, pinned: bool) -> MemoryToolOutcome {
    let Some(item) = items.iter_mut().find(|item| item.id == id) else {
        return MemoryToolOutcome::TargetNotFound { id };
    };
    item.is_pinned = pinned;
    if pinned {
        item.is_cold = false;
        item.importance = Score::FULL;
        MemoryToolOutcome::Pinned { id }
    } else {
        MemoryToolOutcome::Unpinned { id }
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

    use lettuce_types::{MemoryId, MemorySpaceId, Revision, TimestampMillis, ToolExecutionId};
    use serde_json::{Value, json};

    use super::{
        CreateMemoryPreparation, MemoryToolArguments, MemoryToolCall, MemoryToolOutcome,
        MemoryToolReducer, SoftDeleteReason, dynamic_memory_tool_request,
    };
    use crate::{MemoryCategory, MemoryItem, MemoryPolicy, MemorySpaceSnapshot, Score};

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
        MemoryItem {
            id: MemoryId::new(),
            text: text.to_string(),
            category: MemoryCategory::Other,
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
        }
    }

    #[test]
    fn declarations_are_versioned_and_required() {
        let request = dynamic_memory_tool_request();
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
    }

    #[test]
    fn strict_arguments_use_stable_ids() {
        let id = MemoryId::new();
        let parsed =
            MemoryToolArguments::parse("delete_memory", &json!({ "id": id, "confidence": 0.75 }));
        assert_eq!(
            parsed,
            Ok(MemoryToolArguments::DeleteMemory {
                id,
                confidence: Some(score(7_500)),
            })
        );
        assert!(MemoryToolArguments::parse("delete_memory", &json!({ "text": "legacy" })).is_err());
    }

    #[test]
    fn create_skips_normalized_and_semantic_duplicates() {
        let existing = item("Mira likes the old harbor.", 4, 1, false);
        let existing_id = existing.id;
        let state = snapshot(vec![existing]);
        let create_id = MemoryId::new();
        let mut first = call(MemoryToolArguments::CreateMemory {
            text: "mira likes the old harbor".to_string(),
            category: MemoryCategory::Preference,
            important: false,
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
            MemoryToolOutcome::DuplicateSkipped { existing_id: id } if id == existing_id
        ));

        let mut semantic = call(MemoryToolArguments::CreateMemory {
            text: "She enjoys visiting the docks".to_string(),
            category: MemoryCategory::Preference,
            important: false,
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
        assert!(result.is_ok_and(|result| result.change.is_none()));
    }

    #[test]
    fn create_rejects_unqualified_semantic_duplicate_evidence() {
        let existing = item("existing", 4, 1, false);
        let mut create = call(MemoryToolArguments::CreateMemory {
            text: "different memory".to_owned(),
            category: MemoryCategory::Other,
            important: false,
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
                id: first_id,
                confidence: Some(Score::FULL),
            }),
            call(MemoryToolArguments::DeleteMemory {
                id: second_id,
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
    fn hard_delete_limit_uses_cycle_start_active_count() {
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
                id: first_id,
                confidence: Some(Score::FULL),
            }),
            call(MemoryToolArguments::DeleteMemory {
                id: second_id,
                confidence: Some(Score::FULL),
            }),
        ];
        let result = match MemoryToolReducer.reduce(&state, &policy(), &calls) {
            Ok(result) => result,
            Err(error) => panic!("reduction failed: {error}"),
        };
        assert!(matches!(
            result.results[1].outcome,
            MemoryToolOutcome::SoftDeleted {
                reason: SoftDeleteReason::HardDeleteLimitReached,
                ..
            }
        ));
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
                call(MemoryToolArguments::UnpinMemory { id: pinned_id }),
            ],
        ) {
            Ok(result) => result,
            Err(error) => panic!("reduction failed: {error}"),
        };
        assert!(matches!(
            result.results[1].outcome,
            MemoryToolOutcome::StoppedAfterDone
        ));
        let change = match result.change {
            Some(change) => change,
            None => panic!("budget policy should demote the unpinned item"),
        };
        let pinned = change.items.iter().find(|item| item.id == pinned_id);
        assert!(pinned.is_some_and(|item| item.is_pinned && !item.is_cold));
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
                call(MemoryToolArguments::PinMemory { id: existing_id }),
                call(MemoryToolArguments::UnpinMemory { id: existing_id }),
                call(MemoryToolArguments::PinMemory { id: missing_id }),
            ],
        ) {
            Ok(result) => result,
            Err(error) => panic!("reduction failed: {error}"),
        };
        assert!(matches!(
            result.results[0].outcome,
            MemoryToolOutcome::Pinned { id } if id == existing_id
        ));
        assert!(matches!(
            result.results[1].outcome,
            MemoryToolOutcome::Unpinned { id } if id == existing_id
        ));
        assert!(matches!(
            result.results[2].outcome,
            MemoryToolOutcome::TargetNotFound { id } if id == missing_id
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
        let result = match MemoryToolReducer.reduce(
            &snapshot(vec![pinned, weakest, stronger, newest]),
            &policy(),
            &[],
        ) {
            Ok(result) => result,
            Err(error) => panic!("reduction failed: {error}"),
        };
        assert_eq!(result.trimmed_ids, vec![weakest_id]);
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
