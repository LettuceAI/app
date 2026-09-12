use lettuce_context::PromptVariable as Variable;
use lettuce_conversations::{ProposedToolCall, ToolOutput};
use lettuce_memory::{
    DuplicateKind, ListedMemory, MEMORY_CATEGORIES, MemoryCategory, MemoryToolOutcome,
    MemoryToolResult, MemoryToolSkipReason,
};
use serde_json::{Value, json};

use crate::runtime_text::{RuntimeText, RuntimeTextError};

/// The result a memory tool call reports back to the model, in the shape
/// legacy sent: six-digit ids, the deleted text, the memory list right after
/// the call, and a catalog reason for every skipped call.
pub(crate) fn legacy_memory_tool_output(
    text: &RuntimeText,
    call: &ProposedToolCall,
    result: &MemoryToolResult,
) -> Result<ToolOutput, RuntimeTextError> {
    let value = match &result.outcome {
        MemoryToolOutcome::Created {
            short_id, memories, ..
        } => json!({
            "status": "created",
            "name": call.name,
            "memoryId": short_id.to_string(),
            "updatedMemories": memory_lines(text, memories)?,
        }),
        MemoryToolOutcome::Deleted {
            short_id,
            text: deleted,
            memories,
            ..
        } => json!({
            "status": "deleted",
            "name": call.name,
            "deletedMemoryId": short_id.to_string(),
            "deletedText": deleted,
            "updatedMemories": memory_lines(text, memories)?,
        }),
        MemoryToolOutcome::SoftDeleted {
            short_id,
            text: deleted,
            memories,
            ..
        } => json!({
            "status": "soft_deleted",
            "name": call.name,
            "deletedMemoryId": short_id.to_string(),
            "deletedText": deleted,
            "updatedMemories": memory_lines(text, memories)?,
        }),
        MemoryToolOutcome::Pinned { short_id, .. } => json!({
            "status": "pinned",
            "name": call.name,
            "memoryId": short_id.to_string(),
        }),
        MemoryToolOutcome::Unpinned { short_id, .. } => json!({
            "status": "unpinned",
            "name": call.name,
            "memoryId": short_id.to_string(),
        }),
        MemoryToolOutcome::DuplicateSkipped { kind, .. } => {
            skipped(call, duplicate_reason(text, *kind)?, false)
        }
        MemoryToolOutcome::TargetNotFound { .. } => skipped(
            call,
            text.render_with("memory_skip_target_not_found", [])?,
            false,
        ),
        MemoryToolOutcome::Skipped { reason } => skipped(
            call,
            skip_reason(text, call, *reason)?,
            matches!(
                reason,
                MemoryToolSkipReason::MissingCategory | MemoryToolSkipReason::InvalidCategory
            ),
        ),
        MemoryToolOutcome::Done { .. }
        | MemoryToolOutcome::StoppedAfterDone
        | MemoryToolOutcome::Rejected { .. } => {
            serde_json::to_value(&result.outcome).map_err(|_| RuntimeTextError::Render)?
        }
    };
    Ok(ToolOutput {
        value,
        is_error: matches!(result.outcome, MemoryToolOutcome::Rejected { .. }),
    })
}

/// The `[id] text` line the model quotes ids from. A disabled or blank catalog
/// entry fails closed instead of hiding every memory behind an empty line.
pub(crate) fn memory_id_line(
    text: &RuntimeText,
    short_id: lettuce_memory::MemoryShortId,
    memory_text: &str,
) -> Result<String, RuntimeTextError> {
    let mut values = lettuce_context::PromptRenderValues::default();
    values.purpose_values.extend([
        (Variable::MemoryId, short_id.to_string()),
        (Variable::MemoryText, memory_text.to_owned()),
    ]);
    text.render("memory_id_line", &values)?
        .filter(|line| !line.trim().is_empty())
        .ok_or(RuntimeTextError::Unavailable)
}

fn memory_lines(text: &RuntimeText, memories: &[ListedMemory]) -> Result<Value, RuntimeTextError> {
    memories
        .iter()
        .map(|memory| memory_id_line(text, memory.short_id, &memory.text).map(Value::String))
        .collect::<Result<Vec<_>, _>>()
        .map(Value::Array)
}

fn skipped(call: &ProposedToolCall, reason: String, repair_queued: bool) -> Value {
    let mut value = json!({
        "status": "skipped",
        "name": call.name,
        "reason": reason,
        "arguments": call.arguments,
    });
    if repair_queued {
        value["repairQueued"] = Value::Bool(true);
    }
    value
}

fn duplicate_reason(text: &RuntimeText, kind: DuplicateKind) -> Result<String, RuntimeTextError> {
    match kind {
        DuplicateKind::NormalizedText => text.render_with("memory_skip_duplicate_text", []),
        DuplicateKind::LexicalOverlap => text.render_with("memory_skip_duplicate_overlap", []),
        DuplicateKind::Semantic { cosine, threshold } => text.render_with(
            "memory_skip_duplicate_semantic",
            [
                (Variable::DuplicateCosine, format!("{:.2}", cosine.ratio())),
                (
                    Variable::DuplicateThreshold,
                    format!("{:.2}", threshold.ratio()),
                ),
            ],
        ),
    }
}

fn skip_reason(
    text: &RuntimeText,
    call: &ProposedToolCall,
    reason: MemoryToolSkipReason,
) -> Result<String, RuntimeTextError> {
    let key = match reason {
        MemoryToolSkipReason::MissingText => "memory_skip_missing_text",
        MemoryToolSkipReason::InvalidText => "memory_skip_invalid_text",
        MemoryToolSkipReason::EmptyText => "memory_skip_empty_text",
        MemoryToolSkipReason::TextTooLong => "memory_skip_long_text",
        MemoryToolSkipReason::RefusalText => "memory_skip_refusal_text",
        MemoryToolSkipReason::MetaText => "memory_skip_meta_text",
        MemoryToolSkipReason::MissingCategory => "memory_skip_missing_category",
        MemoryToolSkipReason::InvalidCategory => {
            let category = call
                .arguments
                .get("category")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default()
                .to_owned();
            return text.render_with(
                "memory_skip_invalid_category",
                [
                    (Variable::MemoryCategory, category),
                    (
                        Variable::MemoryCategories,
                        MEMORY_CATEGORIES.map(MemoryCategory::as_str).join(", "),
                    ),
                ],
            );
        }
        MemoryToolSkipReason::MissingTarget => "memory_skip_missing_target",
        MemoryToolSkipReason::UnsupportedTool => "memory_skip_unsupported_tool",
        MemoryToolSkipReason::MalformedArguments => "memory_skip_malformed_arguments",
    };
    text.render_with(key, [])
}

#[cfg(test)]
mod tests {
    use lettuce_memory::{MemoryShortId, Score, SoftDeleteReason};
    use lettuce_types::{MemoryId, ToolExecutionId};

    use super::*;

    fn text() -> RuntimeText {
        RuntimeText::from_seed(crate::BuiltInPromptId::MemoryRuntime)
    }

    fn call(name: &str, arguments: Value) -> ProposedToolCall {
        ProposedToolCall {
            provider_call_id: None,
            name: name.into(),
            arguments,
            raw_arguments: None,
            provider_replay: None,
        }
    }

    fn result(outcome: MemoryToolOutcome) -> MemoryToolResult {
        MemoryToolResult {
            execution_id: ToolExecutionId::new(),
            outcome,
        }
    }

    fn short(value: u32) -> MemoryShortId {
        MemoryShortId::new(value).expect("short id")
    }

    fn listed(value: u32, text: &str) -> ListedMemory {
        ListedMemory {
            short_id: short(value),
            text: text.into(),
        }
    }

    fn render(call: &ProposedToolCall, outcome: MemoryToolOutcome) -> ToolOutput {
        legacy_memory_tool_output(&text(), call, &result(outcome)).expect("output")
    }

    #[test]
    fn created_reports_the_short_id_and_the_listed_memories() {
        let arguments = json!({"text":"Mira likes tea","category":"preference"});
        let output = render(
            &call("create_memory", arguments),
            MemoryToolOutcome::Created {
                id: MemoryId::new(),
                short_id: short(42),
                memories: vec![
                    listed(7, "Mira lives by the sea."),
                    listed(42, "Mira likes tea"),
                ],
            },
        );
        assert!(!output.is_error);
        assert_eq!(
            output.value,
            json!({
                "status": "created",
                "name": "create_memory",
                "memoryId": "000042",
                "updatedMemories": ["[000007] Mira lives by the sea.", "[000042] Mira likes tea"],
            })
        );
    }

    #[test]
    fn deletes_report_the_deleted_memory_and_the_remaining_list() {
        let delete = call("delete_memory", json!({"text":"000007","confidence":0.95}));
        let hard = render(
            &delete,
            MemoryToolOutcome::Deleted {
                id: MemoryId::new(),
                short_id: short(7),
                text: "Mira lives by the sea.".into(),
                memories: vec![listed(42, "Mira likes tea")],
            },
        );
        assert_eq!(
            hard.value,
            json!({
                "status": "deleted",
                "name": "delete_memory",
                "deletedMemoryId": "000007",
                "deletedText": "Mira lives by the sea.",
                "updatedMemories": ["[000042] Mira likes tea"],
            })
        );
        let soft = render(
            &delete,
            MemoryToolOutcome::SoftDeleted {
                id: MemoryId::new(),
                short_id: short(7),
                text: "Mira lives by the sea.".into(),
                reason: SoftDeleteReason::LowConfidence,
                memories: vec![
                    listed(7, "Mira lives by the sea."),
                    listed(42, "Mira likes tea"),
                ],
            },
        );
        assert_eq!(soft.value["status"], "soft_deleted");
        assert_eq!(soft.value["deletedMemoryId"], "000007");
        assert_eq!(
            soft.value["updatedMemories"],
            json!(["[000007] Mira lives by the sea.", "[000042] Mira likes tea"])
        );
    }

    #[test]
    fn pin_results_carry_only_the_short_id() {
        let output = render(
            &call("pin_memory", json!({"id":"000042"})),
            MemoryToolOutcome::Pinned {
                id: MemoryId::new(),
                short_id: short(42),
            },
        );
        assert_eq!(
            output.value,
            json!({"status": "pinned", "name": "pin_memory", "memoryId": "000042"})
        );
        let output = render(
            &call("unpin_memory", json!({"id":"000042"})),
            MemoryToolOutcome::Unpinned {
                id: MemoryId::new(),
                short_id: short(42),
            },
        );
        assert_eq!(output.value["status"], "unpinned");
        assert_eq!(output.value["memoryId"], "000042");
    }

    #[test]
    fn skipped_results_echo_the_arguments_with_legacy_reasons() {
        let arguments = json!({"text":"Mira likes tea","category":"mood"});
        let output = render(
            &call("create_memory", arguments.clone()),
            MemoryToolOutcome::Skipped {
                reason: MemoryToolSkipReason::InvalidCategory,
            },
        );
        assert_eq!(
            output.value,
            json!({
                "status": "skipped",
                "name": "create_memory",
                "reason": "invalid category 'mood'; expected one of: character_trait, relationship, plot_event, world_detail, preference, other",
                "repairQueued": true,
                "arguments": arguments,
            })
        );
        let output = render(
            &call(
                "create_memory",
                json!({"text":"I cannot help with that","category":"other"}),
            ),
            MemoryToolOutcome::Skipped {
                reason: MemoryToolSkipReason::RefusalText,
            },
        );
        assert_eq!(output.value["reason"], "memory looked like a refusal");
        assert!(output.value.get("repairQueued").is_none());
        let output = render(
            &call("pin_memory", json!({"id":"999999"})),
            MemoryToolOutcome::TargetNotFound {
                reference: lettuce_memory::MemoryReference("999999".into()),
            },
        );
        assert_eq!(
            output.value,
            json!({
                "status": "skipped",
                "name": "pin_memory",
                "reason": "target_not_found",
                "arguments": {"id":"999999"},
            })
        );
        let output = render(
            &call("summarize", json!({})),
            MemoryToolOutcome::Skipped {
                reason: MemoryToolSkipReason::UnsupportedTool,
            },
        );
        assert_eq!(output.value["name"], "summarize");
        assert_eq!(output.value["reason"], "unsupported_tool");
    }

    #[test]
    fn duplicate_reasons_name_the_matching_check() {
        let create = call(
            "create_memory",
            json!({"text":"Mira likes tea","category":"other"}),
        );
        let reason = |kind| {
            render(
                &create,
                MemoryToolOutcome::DuplicateSkipped {
                    existing_id: MemoryId::new(),
                    kind,
                },
            )
            .value["reason"]
                .clone()
        };
        assert_eq!(
            reason(DuplicateKind::NormalizedText),
            "duplicate (normalized text match)"
        );
        assert_eq!(
            reason(DuplicateKind::LexicalOverlap),
            "duplicate (high lexical overlap)"
        );
        assert_eq!(
            reason(DuplicateKind::Semantic {
                cosine: Score::from_basis_points(8_034).expect("score"),
                threshold: Score::from_basis_points(7_800).expect("score"),
            }),
            "duplicate (cosine 0.80 > 0.78)"
        );
    }

    #[test]
    fn a_disabled_id_line_fails_closed() {
        let mut text = text();
        text.disable_for_test("memory_id_line");
        assert_eq!(
            memory_id_line(&text, short(42), "Mira likes tea"),
            Err(RuntimeTextError::Unavailable)
        );
    }

    #[test]
    fn rejected_results_stay_typed_errors() {
        let output = render(
            &call("create_memory", json!({"text":"x","category":"other"})),
            MemoryToolOutcome::Rejected {
                reason: lettuce_memory::MemoryToolRejection::CreateNotPrepared,
            },
        );
        assert!(output.is_error);
        assert_eq!(
            output.value,
            json!({"status":"rejected","reason":"create_not_prepared"})
        );
    }
}
