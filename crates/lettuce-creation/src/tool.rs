use std::collections::HashSet;

use lettuce_conversations::{ProposedToolCall, ToolChoice, ToolDefinition, ToolRequest};
use lettuce_types::{
    CreationProposalId, CreationTurnId, CreationWorkflowId, LorebookEntryId, Revision, SceneId,
    TimestampMillis,
};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::{
    CreationOperation, CreationProposal, CreationProposalError, CreationRejection,
    CreationRepositoryError, CreationTargetKind, CreationWorkflow, CreationWorkflowRepository,
};

const TOOL_VERSION: u32 = 1;
/// The version every creation tool definition carries; calls to undeclared
/// tools are recorded with it too.
pub const CREATION_TOOL_VERSION: u32 = TOOL_VERSION;

/// Catalog keys of the model-facing tool texts: `(tool, parameter, key)`,
/// where a `None` parameter names the tool description itself.
pub const CREATION_TOOL_TEXT_KEYS: [(&str, Option<&str>, &str); 16] = [
    ("write_definition", None, "creation_write_definition_tool"),
    ("write_scene", None, "creation_write_scene_tool"),
    ("write_lore_entry", None, "creation_write_lore_entry_tool"),
    ("set_name", None, "creation_set_name_tool"),
    ("edit_scene", None, "creation_edit_scene_tool"),
    ("edit_scene", Some("id"), "creation_edit_scene_id"),
    ("edit_scene", Some("content"), "creation_edit_scene_content"),
    ("edit_lore_entry", None, "creation_edit_lore_entry_tool"),
    ("delete_scene", None, "creation_delete_scene_tool"),
    ("delete_lore_entry", None, "creation_delete_lore_entry_tool"),
    (
        "reorder_lore_entries",
        None,
        "creation_reorder_lore_entries_tool",
    ),
    (
        "reorder_lore_entries",
        Some("order"),
        "creation_reorder_lore_entries_order",
    ),
    ("show_preview", None, "creation_show_preview_tool"),
    (
        "show_preview",
        Some("message"),
        "creation_show_preview_message",
    ),
    (
        "request_confirmation",
        None,
        "creation_request_confirmation_tool",
    ),
    (
        "request_confirmation",
        Some("message"),
        "creation_request_confirmation_message",
    ),
];

/// The legacy creation-agent tools the proposal supports for this target,
/// without descriptions; `describe_creation_tools` adds the catalog text
/// before a request is sent. Legacy offered the same tools at every stage.
#[must_use]
pub fn creation_tool_request(target: CreationTargetKind) -> ToolRequest {
    let names: &[&str] = match target {
        CreationTargetKind::Character => &[
            "write_definition",
            "write_scene",
            "set_name",
            "edit_scene",
            "delete_scene",
            "show_preview",
            "request_confirmation",
        ],
        CreationTargetKind::Persona => &[
            "write_definition",
            "set_name",
            "show_preview",
            "request_confirmation",
        ],
        CreationTargetKind::Lorebook => &[
            "write_lore_entry",
            "set_name",
            "edit_lore_entry",
            "delete_lore_entry",
            "reorder_lore_entries",
            "show_preview",
            "request_confirmation",
        ],
    };
    ToolRequest {
        definitions: names
            .iter()
            .map(|name| ToolDefinition {
                name: (*name).to_owned(),
                description: None,
                parameters: tool_parameters(name),
                version: TOOL_VERSION,
            })
            .collect(),
        choice: ToolChoice::Auto,
    }
}

/// Adds the catalog descriptions to a creation tool request. A blank text
/// (a disabled entry) leaves that description out.
#[must_use]
pub fn describe_creation_tools(
    request: &ToolRequest,
    text: &dyn Fn(&str) -> String,
) -> ToolRequest {
    let mut request = request.clone();
    for definition in &mut request.definitions {
        for (tool, parameter, key) in CREATION_TOOL_TEXT_KEYS {
            if tool != definition.name {
                continue;
            }
            let description = text(key);
            if description.trim().is_empty() {
                continue;
            }
            match parameter {
                None => definition.description = Some(description),
                Some(parameter) => {
                    if let Some(property) = definition
                        .parameters
                        .get_mut("properties")
                        .and_then(|properties| properties.get_mut(parameter))
                        .and_then(Value::as_object_mut)
                    {
                        property.insert("description".to_owned(), Value::String(description));
                    }
                }
            }
        }
    }
    request
}

fn tool_parameters(name: &str) -> Value {
    match name {
        "write_definition" => json!({
            "type": "object",
            "properties": { "definition": { "type": "string" } },
            "required": ["definition"]
        }),
        "write_scene" => json!({
            "type": "object",
            "properties": {
                "content": { "type": "string" },
                "direction": { "type": "string" }
            },
            "required": ["content"]
        }),
        "write_lore_entry" => json!({
            "type": "object",
            "properties": {
                "title": { "type": "string" },
                "content": { "type": "string" }
            },
            "required": ["title", "content"]
        }),
        "set_name" => json!({
            "type": "object",
            "properties": { "name": { "type": "string" } },
            "required": ["name"]
        }),
        "edit_scene" => json!({
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "content": { "type": "string" },
                "direction": { "type": "string" }
            },
            "required": ["id", "content"]
        }),
        "edit_lore_entry" => json!({
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "title": { "type": "string" },
                "content": { "type": "string" }
            },
            "required": ["id", "title", "content"]
        }),
        "delete_scene" | "delete_lore_entry" => json!({
            "type": "object",
            "properties": { "id": { "type": "string" } },
            "required": ["id"]
        }),
        "reorder_lore_entries" => json!({
            "type": "object",
            "properties": {
                "order": { "type": "array", "items": { "type": "string" } }
            },
            "required": ["order"]
        }),
        _ => json!({
            "type": "object",
            "properties": { "message": { "type": "string" } }
        }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmittedCreationToolCall {
    pub definition_version: u32,
    pub call: ProposedToolCall,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationToolBatch {
    pub proposal: CreationProposal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationToolCommit {
    pub workflow: CreationWorkflow,
    pub proposal: CreationProposal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationToolApply {
    pub workflow_id: CreationWorkflowId,
    pub expected_workflow_revision: Revision,
    pub base_proposal_id: CreationProposalId,
    pub proposal_id: CreationProposalId,
    pub turn_id: CreationTurnId,
    pub calls: Vec<AdmittedCreationToolCall>,
    pub now: TimestampMillis,
}

pub fn reduce_creation_tool_calls(
    base: &CreationProposal,
    proposal_id: CreationProposalId,
    turn_id: CreationTurnId,
    calls: &[AdmittedCreationToolCall],
    now: TimestampMillis,
) -> Result<CreationToolBatch, CreationToolContractError> {
    let operations = validate_and_parse_creation_tool_calls(base, proposal_id, calls)?;
    let proposal = base.apply(proposal_id, turn_id, operations, now)?;
    Ok(CreationToolBatch { proposal })
}

pub fn validate_creation_tool_calls(
    base: &CreationProposal,
    proposal_id: CreationProposalId,
    calls: &[AdmittedCreationToolCall],
) -> Result<(), CreationToolContractError> {
    validate_and_parse_creation_tool_calls(base, proposal_id, calls).map(|_| ())
}

fn validate_and_parse_creation_tool_calls(
    base: &CreationProposal,
    proposal_id: CreationProposalId,
    calls: &[AdmittedCreationToolCall],
) -> Result<Vec<CreationOperation>, CreationToolContractError> {
    let request = creation_tool_request(base.draft.kind());
    request
        .validate()
        .map_err(|_| CreationToolContractError::InvalidContract)?;
    if calls.is_empty() || calls.len() > crate::proposal::MAX_CREATION_OPERATIONS {
        return Err(CreationToolContractError::InvalidCallCount);
    }
    let mut provider_ids = HashSet::new();
    let mut operations = Vec::with_capacity(calls.len());
    for (ordinal, admitted) in calls.iter().enumerate() {
        admitted
            .call
            .validate()
            .map_err(|_| CreationToolContractError::MalformedCall)?;
        if let Some(provider_id) = &admitted.call.provider_call_id
            && !provider_ids.insert(provider_id.as_str())
        {
            return Err(CreationToolContractError::DuplicateProviderCallId);
        }
        let name = canonical_tool_name(&admitted.call.name);
        let Some(definition) = request
            .definitions
            .iter()
            .find(|definition| definition.name == name)
        else {
            if admitted.definition_version != TOOL_VERSION {
                return Err(CreationToolContractError::DefinitionVersionMismatch);
            }
            operations.push(CreationOperation::UndeclaredTool {
                name: admitted.call.name.clone(),
            });
            continue;
        };
        if admitted.definition_version != definition.version {
            return Err(CreationToolContractError::DefinitionVersionMismatch);
        }
        operations.push(parse_operation(
            &name,
            &admitted.call.arguments,
            proposal_id,
            ordinal,
        ));
    }
    Ok(operations)
}

pub fn apply_creation_tool_calls(
    repository: &dyn CreationWorkflowRepository,
    request: CreationToolApply,
) -> Result<CreationToolCommit, CreationToolContractError> {
    let base = repository.load_proposal(request.base_proposal_id)?;
    let batch = reduce_creation_tool_calls(
        &base,
        request.proposal_id,
        request.turn_id,
        &request.calls,
        request.now,
    )?;
    let workflow = repository.append_proposal(
        request.workflow_id,
        request.expected_workflow_revision,
        batch.proposal.clone(),
    )?;
    Ok(CreationToolCommit {
        workflow,
        proposal: batch.proposal,
    })
}

/// The declared tool a provider name refers to: legacy trimmed and lowercased
/// names and accepted `preview` and `confirm`.
#[must_use]
pub fn canonical_tool_name(name: &str) -> String {
    let normalized = name.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "preview" => "show_preview".to_owned(),
        "confirm" => "request_confirmation".to_owned(),
        _ => normalized,
    }
}

fn parse_operation(
    name: &str,
    arguments: &Value,
    proposal_id: CreationProposalId,
    ordinal: usize,
) -> CreationOperation {
    let empty = Map::new();
    let object = arguments.as_object().unwrap_or(&empty);
    let rejected = |reason| CreationOperation::Rejected {
        tool: name.to_owned(),
        reason,
    };
    let missing = |argument: &str| {
        rejected(CreationRejection::MissingArgument {
            argument: argument.to_owned(),
        })
    };
    let parsed = (|| -> Result<CreationOperation, CreationOperation> {
        Ok(match name {
            "set_name" => CreationOperation::SetName {
                value: text(object, &["name", "note"]).ok_or_else(|| missing("name"))?,
            },
            "write_definition" => CreationOperation::SetDescription {
                value: text(object, &["definition", "text"])
                    .ok_or_else(|| missing("definition"))?,
            },
            "write_scene" => CreationOperation::AddScene {
                id: SceneId::from_uuid(derived_id(proposal_id, ordinal, "scene")),
                content: text(object, &["content", "text"]).ok_or_else(|| missing("content"))?,
                direction: text(object, &["direction"]).filter(|value| !value.trim().is_empty()),
            },
            "edit_scene" => {
                let id = text(object, &["id", "scene_id"]).ok_or_else(|| missing("id"))?;
                let content = text(object, &["content"]).ok_or_else(|| missing("content"))?;
                CreationOperation::UpdateScene {
                    id: parse_id(&id).ok_or_else(|| rejected(unknown(&id)))?,
                    content,
                    direction: text(object, &["direction"])
                        .filter(|value| !value.trim().is_empty()),
                }
            }
            "delete_scene" => {
                let id = text(object, &["id"]).ok_or_else(|| missing("id"))?;
                CreationOperation::DeleteScene {
                    id: parse_id(&id).ok_or_else(|| rejected(unknown(&id)))?,
                }
            }
            "write_lore_entry" => {
                let content =
                    text(object, &["content", "text"]).ok_or_else(|| missing("content"))?;
                CreationOperation::UpsertLorebookEntry {
                    id: LorebookEntryId::from_uuid(derived_id(
                        proposal_id,
                        ordinal,
                        "lorebook-entry",
                    )),
                    title: text(object, &["title"]).ok_or_else(|| missing("title"))?,
                    content,
                }
            }
            "edit_lore_entry" => {
                let id = text(object, &["id"]).ok_or_else(|| missing("id"))?;
                let title = text(object, &["title"]).ok_or_else(|| missing("title"))?;
                let content = text(object, &["content"]).ok_or_else(|| missing("content"))?;
                CreationOperation::UpdateLorebookEntry {
                    id: parse_id(&id).ok_or_else(|| rejected(unknown(&id)))?,
                    title,
                    content,
                }
            }
            "delete_lore_entry" => {
                let id = text(object, &["id"]).ok_or_else(|| missing("id"))?;
                CreationOperation::DeleteLorebookEntry {
                    id: parse_id(&id).ok_or_else(|| rejected(unknown(&id)))?,
                }
            }
            "reorder_lore_entries" => {
                let ids = match object.get("order") {
                    Some(Value::Array(items)) => items
                        .iter()
                        .filter_map(|item| match item {
                            Value::String(id) => Some(id.trim().to_owned()),
                            Value::Number(id) => Some(id.to_string()),
                            _ => None,
                        })
                        .filter(|id| !id.is_empty())
                        .collect::<Vec<_>>(),
                    Some(Value::String(order)) => order
                        .split(',')
                        .map(str::trim)
                        .filter(|id| !id.is_empty())
                        .map(str::to_owned)
                        .collect(),
                    _ => Vec::new(),
                };
                if ids.is_empty() {
                    return Err(missing("order"));
                }
                CreationOperation::ReorderLorebookEntries {
                    order: ids
                        .iter()
                        .map(|id| parse_id(id).ok_or_else(|| rejected(unknown(id))))
                        .collect::<Result<_, _>>()?,
                }
            }
            "show_preview" => CreationOperation::ShowPreview,
            "request_confirmation" => CreationOperation::RequestConfirmation,
            _ => CreationOperation::UndeclaredTool {
                name: name.to_owned(),
            },
        })
    })();
    parsed.unwrap_or_else(|rejection| rejection)
}

fn unknown(id: &str) -> CreationRejection {
    CreationRejection::UnknownId { id: id.to_owned() }
}

/// The first present value among `names`; legacy skipped nulls and empty
/// values and read numbers and booleans as their text.
fn text(object: &Map<String, Value>, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| match object.get(*name)? {
        Value::String(value) if !value.is_empty() => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    })
}

fn parse_id<T: std::str::FromStr>(id: &str) -> Option<T> {
    id.trim().parse().ok()
}

fn derived_id(proposal_id: CreationProposalId, ordinal: usize, kind: &str) -> Uuid {
    Uuid::new_v5(
        &proposal_id.as_uuid(),
        format!("creation-tool-v{TOOL_VERSION}:{kind}:{ordinal}").as_bytes(),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CreationToolContractError {
    #[error("creation tool contract is invalid")]
    InvalidContract,
    #[error("creation tool call count is invalid")]
    InvalidCallCount,
    #[error("creation tool call is malformed")]
    MalformedCall,
    #[error("creation tool is not declared")]
    UnsupportedTool,
    #[error("creation tool definition version does not match")]
    DefinitionVersionMismatch,
    #[error("creation provider call id is duplicated")]
    DuplicateProviderCallId,
    #[error("creation proposal is invalid")]
    Proposal(#[from] CreationProposalError),
    #[error("creation persistence failed")]
    Repository(#[from] CreationRepositoryError),
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::ProposedToolCall;
    use lettuce_types::{
        CreationProposalId, CreationTurnId, LorebookEntryId, SceneId, TimestampMillis,
    };
    use serde_json::json;

    use crate::{
        AdmittedCreationToolCall, CreationDraft, CreationOperationError, CreationStage,
        CreationTargetKind, CreationToolContractError, creation_tool_request,
        describe_creation_tools, reduce_creation_tool_calls,
    };

    fn call(name: &str, arguments: serde_json::Value) -> AdmittedCreationToolCall {
        AdmittedCreationToolCall {
            definition_version: 1,
            call: ProposedToolCall {
                provider_call_id: Some(format!("call-{name}-{}", uuid::Uuid::new_v4())),
                name: name.to_owned(),
                arguments,
                raw_arguments: None,
                provider_replay: None,
            },
        }
    }

    #[test]
    fn tool_contract_uses_the_legacy_agent_names_per_target() {
        let names = |target| {
            let request = creation_tool_request(target);
            request.validate().expect("valid tools");
            assert!(
                request
                    .definitions
                    .iter()
                    .all(|definition| definition.version == 1 && definition.description.is_none())
            );
            request
                .definitions
                .into_iter()
                .map(|definition| definition.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(CreationTargetKind::Character),
            [
                "write_definition",
                "write_scene",
                "set_name",
                "edit_scene",
                "delete_scene",
                "show_preview",
                "request_confirmation"
            ]
        );
        assert_eq!(
            names(CreationTargetKind::Persona),
            [
                "write_definition",
                "set_name",
                "show_preview",
                "request_confirmation"
            ]
        );
        assert_eq!(
            names(CreationTargetKind::Lorebook),
            [
                "write_lore_entry",
                "set_name",
                "edit_lore_entry",
                "delete_lore_entry",
                "reorder_lore_entries",
                "show_preview",
                "request_confirmation"
            ]
        );
        let described = describe_creation_tools(
            &creation_tool_request(CreationTargetKind::Character),
            &|key| {
                if key == "creation_set_name_tool" {
                    String::new()
                } else {
                    key.to_owned()
                }
            },
        );
        described.validate().expect("described tools");
        assert_eq!(
            described.definitions[3].description.as_deref(),
            Some("creation_edit_scene_tool")
        );
        assert_eq!(
            described.definitions[3].parameters["properties"]["id"]["description"],
            json!("creation_edit_scene_id")
        );
        assert_eq!(described.definitions[2].description, None);
        assert_eq!(
            described.definitions[5].parameters["properties"]["message"]["description"],
            json!("creation_show_preview_message")
        );
    }

    #[test]
    fn reducer_returns_one_output_per_ordered_call_and_stable_generated_ids() {
        let base = crate::CreationProposal::initial(
            CreationProposalId::new(),
            CreationDraft::Character {
                name: None,
                definition: None,
                scenes: Vec::new(),
            },
            TimestampMillis::new(1),
        )
        .expect("base");
        let proposal_id = CreationProposalId::new();
        let turn_id = CreationTurnId::new();
        let missing = SceneId::new();
        let calls = vec![
            call("edit_scene", json!({"id": missing, "content": "missing"})),
            call(
                "write_scene",
                json!({"content": "Welcome.", "direction": "calmly", "extra": true}),
            ),
            call("set_name", json!({"name": "Aster"})),
            call(
                "write_definition",
                json!({"definition": "A quiet archivist."}),
            ),
            call("show_preview", json!({"message": "Take a look."})),
        ];
        let first = reduce_creation_tool_calls(
            &base,
            proposal_id,
            turn_id,
            &calls,
            TimestampMillis::new(2),
        )
        .expect("first reduction");
        let retry = reduce_creation_tool_calls(
            &base,
            proposal_id,
            turn_id,
            &calls,
            TimestampMillis::new(2),
        )
        .expect("stable retry");
        assert_eq!(first, retry);
        let errors = |proposal: &crate::CreationProposal| {
            proposal
                .outcomes
                .iter()
                .map(|outcome| outcome.error)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            errors(&first.proposal),
            [
                Some(CreationOperationError::NotFound),
                None,
                None,
                None,
                None
            ]
        );
        assert_eq!(first.proposal.stage, CreationStage::AwaitingReview);
        let CreationDraft::Character {
            name,
            definition,
            scenes,
        } = first.proposal.draft
        else {
            panic!("character draft");
        };
        assert_eq!(name.as_deref(), Some("Aster"));
        assert_eq!(definition.as_deref(), Some("A quiet archivist."));
        assert_eq!(scenes.len(), 1);
        assert!(matches!(
            first.proposal.outcomes[1].operation,
            crate::CreationOperation::AddScene { id, .. } if id == scenes[0].id
        ));

        let editable = crate::CreationProposal::initial(
            CreationProposalId::new(),
            CreationDraft::Character {
                name: None,
                definition: None,
                scenes: vec![crate::CreationScene {
                    id: scenes[0].id,
                    content: "Welcome.".to_owned(),
                    direction: Some("calmly".to_owned()),
                }],
            },
            TimestampMillis::new(2),
        )
        .expect("editable");
        let edited = reduce_creation_tool_calls(
            &editable,
            CreationProposalId::new(),
            CreationTurnId::new(),
            &[
                call(
                    "edit_scene",
                    json!({"id": scenes[0].id, "content": "Revised welcome."}),
                ),
                call("delete_scene", json!({"id": scenes[0].id})),
                call("delete_scene", json!({"id": scenes[0].id})),
            ],
            TimestampMillis::new(3),
        )
        .expect("edit and delete scene");
        assert_eq!(
            errors(&edited.proposal),
            [None, None, Some(CreationOperationError::NotFound)]
        );
        let CreationDraft::Character { scenes, .. } = edited.proposal.draft else {
            panic!("character draft");
        };
        assert!(scenes.is_empty());
    }

    #[test]
    fn lorebook_entries_are_written_edited_reordered_and_deleted() {
        let base = crate::CreationProposal::initial(
            CreationProposalId::new(),
            CreationDraft::Lorebook {
                name: None,
                description: None,
                entries: Vec::new(),
            },
            TimestampMillis::new(1),
        )
        .expect("base");
        let written = reduce_creation_tool_calls(
            &base,
            CreationProposalId::new(),
            CreationTurnId::new(),
            &[
                call(
                    "write_lore_entry",
                    json!({"title": "Pact", "content": "Sworn."}),
                ),
                call(
                    "write_lore_entry",
                    json!({"title": "Siege", "content": "Held."}),
                ),
                call(
                    "write_lore_entry",
                    json!({"title": "Oath", "content": "Kept."}),
                ),
            ],
            TimestampMillis::new(2),
        )
        .expect("write entries");
        let ids = written
            .proposal
            .outcomes
            .iter()
            .map(|outcome| match &outcome.operation {
                crate::CreationOperation::UpsertLorebookEntry { id, .. } => id.to_string(),
                other => panic!("unexpected {other:?}"),
            })
            .collect::<Vec<_>>();
        let changed = reduce_creation_tool_calls(
            &written.proposal,
            CreationProposalId::new(),
            CreationTurnId::new(),
            &[
                call(
                    "edit_lore_entry",
                    json!({"id": ids[1], "title": "Siege of Khovar", "content": "Broken."}),
                ),
                call(
                    "edit_lore_entry",
                    json!({"id": LorebookEntryId::new(), "title": "Ghost", "content": "None."}),
                ),
                call("reorder_lore_entries", json!({"order": [ids[2], ids[1]]})),
                call("reorder_lore_entries", json!({"order": [ids[2], ids[2]]})),
                call("delete_lore_entry", json!({"id": ids[0]})),
            ],
            TimestampMillis::new(3),
        )
        .expect("change entries");
        assert_eq!(
            changed
                .proposal
                .outcomes
                .iter()
                .map(|outcome| outcome.error)
                .collect::<Vec<_>>(),
            [
                None,
                Some(CreationOperationError::NotFound),
                None,
                Some(CreationOperationError::DuplicateIdentity),
                None
            ]
        );
        let CreationDraft::Lorebook { entries, .. } = changed.proposal.draft else {
            panic!("lorebook draft");
        };
        assert_eq!(
            entries
                .iter()
                .map(|entry| (entry.title.as_str(), entry.content.as_str()))
                .collect::<Vec<_>>(),
            [("Oath", "Kept."), ("Siege of Khovar", "Broken.")]
        );
    }

    #[test]
    fn undeclared_tools_and_bad_arguments_are_answered_but_versions_must_match() {
        let base = crate::CreationProposal::initial(
            CreationProposalId::new(),
            CreationDraft::Persona {
                name: None,
                description: None,
            },
            TimestampMillis::new(1),
        )
        .expect("base");
        let reduce = |calls: &[AdmittedCreationToolCall]| {
            reduce_creation_tool_calls(
                &base,
                CreationProposalId::new(),
                CreationTurnId::new(),
                calls,
                TimestampMillis::new(2),
            )
        };
        let undeclared =
            reduce(&[call("write_scene", json!({"content": "wrong target"}))]).expect("undeclared");
        assert_eq!(
            undeclared.proposal.outcomes[0].operation,
            crate::CreationOperation::UndeclaredTool {
                name: "write_scene".into()
            }
        );
        assert_eq!(
            undeclared.proposal.outcomes[0].error,
            Some(CreationOperationError::UnknownTool)
        );
        assert_eq!(undeclared.proposal.draft, base.draft);
        let mut stale = call("set_name", json!({"name": "Aster"}));
        stale.definition_version = 2;
        assert_eq!(
            reduce(&[stale]),
            Err(CreationToolContractError::DefinitionVersionMismatch)
        );
        let answered = reduce(&[
            call("set_name", json!({"title": "Aster"})),
            call("write_definition", json!({"definition": null})),
            call("Set_Name", json!({"note": "Aster", "extra": true})),
            call("write_definition", json!({"text": 42})),
            call("Preview", json!({"message": "Look"})),
        ])
        .expect("answered");
        assert_eq!(
            answered
                .proposal
                .outcomes
                .iter()
                .map(|outcome| (outcome.operation.clone(), outcome.error))
                .collect::<Vec<_>>(),
            [
                (
                    crate::CreationOperation::Rejected {
                        tool: "set_name".into(),
                        reason: crate::CreationRejection::MissingArgument {
                            argument: "name".into()
                        }
                    },
                    Some(CreationOperationError::InvalidArguments)
                ),
                (
                    crate::CreationOperation::Rejected {
                        tool: "write_definition".into(),
                        reason: crate::CreationRejection::MissingArgument {
                            argument: "definition".into()
                        }
                    },
                    Some(CreationOperationError::InvalidArguments)
                ),
                (
                    crate::CreationOperation::SetName {
                        value: "Aster".into()
                    },
                    None
                ),
                (
                    crate::CreationOperation::SetDescription { value: "42".into() },
                    None
                ),
                (crate::CreationOperation::ShowPreview, None),
            ]
        );
        assert_eq!(answered.proposal.stage, CreationStage::AwaitingReview);
    }

    #[test]
    fn unknown_ids_and_empty_directions_follow_legacy() {
        let scene = SceneId::new();
        let base = crate::CreationProposal::initial(
            CreationProposalId::new(),
            CreationDraft::Character {
                name: None,
                definition: None,
                scenes: vec![crate::CreationScene {
                    id: scene,
                    content: "Welcome.".into(),
                    direction: None,
                }],
            },
            TimestampMillis::new(1),
        )
        .expect("base");
        let batch = reduce_creation_tool_calls(
            &base,
            CreationProposalId::new(),
            CreationTurnId::new(),
            &[
                call("edit_scene", json!({"id": "sc_2", "content": "x"})),
                call("set_name", json!({"name": "", "note": "Aster"})),
                call(
                    "edit_scene",
                    json!({"scene_id": scene, "content": "Revised.", "direction": null}),
                ),
                call(
                    "write_scene",
                    json!({"content": "Second.", "direction": " "}),
                ),
            ],
            TimestampMillis::new(2),
        )
        .expect("batch");
        assert_eq!(
            batch.proposal.outcomes[0].operation,
            crate::CreationOperation::Rejected {
                tool: "edit_scene".into(),
                reason: crate::CreationRejection::UnknownId { id: "sc_2".into() }
            }
        );
        assert_eq!(
            batch.proposal.outcomes[0].error,
            Some(CreationOperationError::NotFound)
        );
        let CreationDraft::Character { name, scenes, .. } = batch.proposal.draft else {
            panic!("character draft");
        };
        assert_eq!(name.as_deref(), Some("Aster"));
        assert_eq!(scenes[0].content, "Revised.");
        assert_eq!(scenes[1].direction, None);
    }
}
