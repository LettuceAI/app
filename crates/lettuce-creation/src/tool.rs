use std::collections::HashSet;

use lettuce_conversations::{
    ProposedToolCall, ToolChoice, ToolDefinition, ToolOutput, ToolRequest,
};
use lettuce_types::{
    CreationProposalId, CreationTurnId, CreationWorkflowId, LorebookEntryId, Revision, SceneId,
    TimestampMillis,
};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::{
    CreationOperation, CreationOperationError, CreationProposal, CreationProposalError,
    CreationRepositoryError, CreationStage, CreationTargetKind, CreationWorkflow,
    CreationWorkflowRepository,
};

const TOOL_VERSION: u32 = 1;

#[must_use]
pub fn creation_tool_request(
    target: CreationTargetKind,
    stage: CreationStage,
) -> Option<ToolRequest> {
    let mut definitions = match stage {
        CreationStage::Drafting => drafting_definitions(target),
        CreationStage::AwaitingReview => vec![definition(
            "request_confirmation",
            "Request explicit confirmation without applying the authored target.",
            empty_parameters(),
        )],
        CreationStage::AwaitingConfirmation => return None,
    };
    if stage == CreationStage::Drafting {
        definitions.push(definition(
            "show_preview",
            "Move the current proposal to explicit review.",
            empty_parameters(),
        ));
    }
    Some(ToolRequest {
        definitions,
        choice: ToolChoice::Auto,
    })
}

fn drafting_definitions(target: CreationTargetKind) -> Vec<ToolDefinition> {
    let mut definitions = vec![definition(
        match target {
            CreationTargetKind::Character => "set_character_name",
            CreationTargetKind::Persona => "set_persona_name",
            CreationTargetKind::Lorebook => "set_lorebook_name",
        },
        "Set the proposal name.",
        one_string_parameter("name"),
    )];
    definitions.push(definition(
        match target {
            CreationTargetKind::Character => "set_character_definition",
            CreationTargetKind::Persona => "set_persona_description",
            CreationTargetKind::Lorebook => "set_lorebook_description",
        },
        "Set the proposal description or character definition.",
        one_string_parameter(match target {
            CreationTargetKind::Character => "definition",
            CreationTargetKind::Persona | CreationTargetKind::Lorebook => "description",
        }),
    ));
    match target {
        CreationTargetKind::Character => {
            definitions.push(definition(
                "add_scene",
                "Add one starting scene to the proposal.",
                json!({
                    "type": "object",
                    "properties": {
                        "content": { "type": "string" },
                        "direction": { "type": "string" }
                    },
                    "required": ["content"],
                    "additionalProperties": false
                }),
            ));
            definitions.push(definition(
                "update_scene",
                "Replace one existing proposal scene by stable ID.",
                json!({
                    "type": "object",
                    "properties": {
                        "scene_id": { "type": "string", "format": "uuid" },
                        "content": { "type": "string" },
                        "direction": { "type": "string" }
                    },
                    "required": ["scene_id", "content"],
                    "additionalProperties": false
                }),
            ));
        }
        CreationTargetKind::Persona => {}
        CreationTargetKind::Lorebook => {
            definitions.push(definition(
                "upsert_lorebook_entry",
                "Add or replace one proposal entry. Omit id when adding.",
                json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "format": "uuid" },
                        "title": { "type": "string" },
                        "content": { "type": "string" }
                    },
                    "required": ["title", "content"],
                    "additionalProperties": false
                }),
            ));
            definitions.push(definition(
                "delete_lorebook_entry",
                "Remove one entry from proposal state by stable ID.",
                json!({
                    "type": "object",
                    "properties": { "id": { "type": "string", "format": "uuid" } },
                    "required": ["id"],
                    "additionalProperties": false
                }),
            ));
        }
    }
    definitions
}

fn definition(name: &str, description: &str, parameters: Value) -> ToolDefinition {
    ToolDefinition {
        name: name.to_owned(),
        description: Some(description.to_owned()),
        parameters,
        version: TOOL_VERSION,
    }
}

fn one_string_parameter(name: &str) -> Value {
    json!({
        "type": "object",
        "properties": { name: { "type": "string" } },
        "required": [name],
        "additionalProperties": false
    })
}

fn empty_parameters() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false
    })
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
    pub outputs: Vec<ToolOutput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationToolCommit {
    pub workflow: CreationWorkflow,
    pub proposal: CreationProposal,
    pub outputs: Vec<ToolOutput>,
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
    let outputs = proposal
        .outcomes
        .iter()
        .zip(calls)
        .map(|(outcome, call)| output_for_outcome(&call.call.name, outcome))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CreationToolBatch { proposal, outputs })
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
    let request = creation_tool_request(base.draft.kind(), base.stage)
        .ok_or(CreationToolContractError::ToolsUnavailable)?;
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
        let definition = request
            .definitions
            .iter()
            .find(|definition| definition.name == admitted.call.name)
            .ok_or(CreationToolContractError::UnsupportedTool)?;
        if admitted.definition_version != definition.version {
            return Err(CreationToolContractError::DefinitionVersionMismatch);
        }
        operations.push(parse_operation(
            &admitted.call.name,
            &admitted.call.arguments,
            proposal_id,
            ordinal,
        )?);
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
        outputs: batch.outputs,
    })
}

fn parse_operation(
    name: &str,
    arguments: &Value,
    proposal_id: CreationProposalId,
    ordinal: usize,
) -> Result<CreationOperation, CreationToolContractError> {
    let object = arguments
        .as_object()
        .ok_or(CreationToolContractError::MalformedArguments)?;
    match name {
        "set_character_name" | "set_persona_name" | "set_lorebook_name" => {
            ensure_keys(object, &["name"])?;
            Ok(CreationOperation::SetName {
                value: required_string(object, "name")?,
            })
        }
        "set_character_definition" => {
            ensure_keys(object, &["definition"])?;
            Ok(CreationOperation::SetDescription {
                value: required_string(object, "definition")?,
            })
        }
        "set_persona_description" | "set_lorebook_description" => {
            ensure_keys(object, &["description"])?;
            Ok(CreationOperation::SetDescription {
                value: required_string(object, "description")?,
            })
        }
        "add_scene" => {
            ensure_keys(object, &["content", "direction"])?;
            Ok(CreationOperation::AddScene {
                id: SceneId::from_uuid(derived_id(proposal_id, ordinal, "scene")),
                content: required_string(object, "content")?,
                direction: optional_string(object, "direction")?,
            })
        }
        "update_scene" => {
            ensure_keys(object, &["scene_id", "content", "direction"])?;
            Ok(CreationOperation::UpdateScene {
                id: required_id(object, "scene_id")?,
                content: required_string(object, "content")?,
                direction: optional_string(object, "direction")?,
            })
        }
        "upsert_lorebook_entry" => {
            ensure_keys(object, &["id", "title", "content"])?;
            Ok(CreationOperation::UpsertLorebookEntry {
                id: optional_id(object, "id")?.unwrap_or_else(|| {
                    LorebookEntryId::from_uuid(derived_id(proposal_id, ordinal, "lorebook-entry"))
                }),
                title: required_string(object, "title")?,
                content: required_string(object, "content")?,
            })
        }
        "delete_lorebook_entry" => {
            ensure_keys(object, &["id"])?;
            Ok(CreationOperation::DeleteLorebookEntry {
                id: required_id(object, "id")?,
            })
        }
        "show_preview" => {
            ensure_keys(object, &[])?;
            Ok(CreationOperation::ShowPreview)
        }
        "request_confirmation" => {
            ensure_keys(object, &[])?;
            Ok(CreationOperation::RequestConfirmation)
        }
        _ => Err(CreationToolContractError::UnsupportedTool),
    }
}

fn derived_id(proposal_id: CreationProposalId, ordinal: usize, kind: &str) -> Uuid {
    Uuid::new_v5(
        &proposal_id.as_uuid(),
        format!("creation-tool-v{TOOL_VERSION}:{kind}:{ordinal}").as_bytes(),
    )
}

fn ensure_keys(
    object: &Map<String, Value>,
    allowed: &[&str],
) -> Result<(), CreationToolContractError> {
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        Err(CreationToolContractError::UnknownArgument)
    } else {
        Ok(())
    }
}

fn required_string(
    object: &Map<String, Value>,
    name: &'static str,
) -> Result<String, CreationToolContractError> {
    object
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(CreationToolContractError::MissingOrInvalidArgument(name))
}

fn optional_string(
    object: &Map<String, Value>,
    name: &'static str,
) -> Result<Option<String>, CreationToolContractError> {
    object
        .get(name)
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or(CreationToolContractError::MissingOrInvalidArgument(name))
        })
        .transpose()
}

fn required_id<T: std::str::FromStr>(
    object: &Map<String, Value>,
    name: &'static str,
) -> Result<T, CreationToolContractError> {
    required_string(object, name)?
        .parse()
        .map_err(|_| CreationToolContractError::MissingOrInvalidArgument(name))
}

fn optional_id<T: std::str::FromStr>(
    object: &Map<String, Value>,
    name: &'static str,
) -> Result<Option<T>, CreationToolContractError> {
    object
        .get(name)
        .map(|_| required_id(object, name))
        .transpose()
}

fn output_for_outcome(
    tool_name: &str,
    outcome: &crate::CreationOperationOutcome,
) -> Result<ToolOutput, CreationToolContractError> {
    let mut value = if let Some(error) = outcome.error {
        json!({
            "status": "error",
            "tool": tool_name,
            "code": operation_error_name(error)
        })
    } else {
        json!({ "status": "applied", "tool": tool_name })
    };
    let object = value
        .as_object_mut()
        .ok_or(CreationToolContractError::InvalidContract)?;
    match &outcome.operation {
        CreationOperation::AddScene { id, .. } | CreationOperation::UpdateScene { id, .. } => {
            object.insert("scene_id".to_owned(), json!(id));
        }
        CreationOperation::UpsertLorebookEntry { id, .. }
        | CreationOperation::DeleteLorebookEntry { id } => {
            object.insert("entry_id".to_owned(), json!(id));
        }
        CreationOperation::ShowPreview => {
            object.insert("stage".to_owned(), json!("awaiting_review"));
        }
        CreationOperation::RequestConfirmation => {
            object.insert("stage".to_owned(), json!("awaiting_confirmation"));
        }
        CreationOperation::SetName { .. } | CreationOperation::SetDescription { .. } => {}
    }
    let output = ToolOutput {
        value,
        is_error: outcome.error.is_some(),
    };
    output
        .validate()
        .map_err(|_| CreationToolContractError::InvalidContract)?;
    Ok(output)
}

fn operation_error_name(error: CreationOperationError) -> &'static str {
    match error {
        CreationOperationError::WrongTarget => "wrong_target",
        CreationOperationError::InvalidText => "invalid_text",
        CreationOperationError::DuplicateIdentity => "duplicate_identity",
        CreationOperationError::NotFound => "not_found",
        CreationOperationError::InvalidStage => "invalid_stage",
        CreationOperationError::LimitExceeded => "limit_exceeded",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CreationToolContractError {
    #[error("creation tools are unavailable for this stage")]
    ToolsUnavailable,
    #[error("creation tool contract is invalid")]
    InvalidContract,
    #[error("creation tool call count is invalid")]
    InvalidCallCount,
    #[error("creation tool call is malformed")]
    MalformedCall,
    #[error("creation tool call arguments are malformed")]
    MalformedArguments,
    #[error("creation tool is not declared")]
    UnsupportedTool,
    #[error("creation tool definition version does not match")]
    DefinitionVersionMismatch,
    #[error("creation tool arguments contain an unknown field")]
    UnknownArgument,
    #[error("creation tool argument is missing or invalid")]
    MissingOrInvalidArgument(&'static str),
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
    use lettuce_types::{CreationProposalId, CreationTurnId, SceneId, TimestampMillis};
    use serde_json::json;

    use crate::{
        AdmittedCreationToolCall, CreationDraft, CreationStage, CreationTargetKind,
        CreationToolContractError, creation_tool_request, reduce_creation_tool_calls,
    };

    fn call(name: &str, arguments: serde_json::Value) -> AdmittedCreationToolCall {
        AdmittedCreationToolCall {
            definition_version: 1,
            call: ProposedToolCall {
                provider_call_id: Some(format!("call-{name}")),
                name: name.to_owned(),
                arguments,
                raw_arguments: None,
                provider_replay: None,
            },
        }
    }

    #[test]
    fn tool_contract_is_target_and_stage_specific() {
        let character =
            creation_tool_request(CreationTargetKind::Character, CreationStage::Drafting)
                .expect("character tools");
        character.validate().expect("valid character tools");
        assert_eq!(
            character
                .definitions
                .iter()
                .map(|definition| definition.name.as_str())
                .collect::<Vec<_>>(),
            [
                "set_character_name",
                "set_character_definition",
                "add_scene",
                "update_scene",
                "show_preview",
            ]
        );
        assert!(character.definitions.iter().all(|definition| {
            definition.version == 1 && definition.parameters["additionalProperties"] == json!(false)
        }));
        assert_eq!(
            character.definitions[1].parameters["properties"]
                .as_object()
                .expect("properties")
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["definition"]
        );

        let persona = creation_tool_request(CreationTargetKind::Persona, CreationStage::Drafting)
            .expect("persona tools");
        assert_eq!(persona.definitions.len(), 3);
        assert!(
            persona
                .definitions
                .iter()
                .any(|definition| definition.name == "set_persona_description")
        );
        let lorebook = creation_tool_request(CreationTargetKind::Lorebook, CreationStage::Drafting)
            .expect("lorebook tools");
        assert!(
            lorebook
                .definitions
                .iter()
                .any(|definition| definition.name == "upsert_lorebook_entry")
        );
        let review =
            creation_tool_request(CreationTargetKind::Character, CreationStage::AwaitingReview)
                .expect("review tools");
        assert_eq!(review.definitions.len(), 1);
        assert_eq!(review.definitions[0].name, "request_confirmation");
        assert!(
            creation_tool_request(
                CreationTargetKind::Character,
                CreationStage::AwaitingConfirmation
            )
            .is_none()
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
            call(
                "update_scene",
                json!({"scene_id": missing, "content": "missing"}),
            ),
            call(
                "add_scene",
                json!({"content": "Welcome.", "direction": "calmly"}),
            ),
            call("set_character_name", json!({"name": "Aster"})),
            call("show_preview", json!({})),
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
        assert_eq!(first.outputs.len(), calls.len());
        assert!(first.outputs[0].is_error);
        assert_eq!(first.outputs[0].value["code"], "not_found");
        assert!(first.outputs[1..].iter().all(|output| !output.is_error));
        assert_eq!(first.proposal.stage, CreationStage::AwaitingReview);
        let CreationDraft::Character { name, scenes, .. } = first.proposal.draft else {
            panic!("character draft");
        };
        assert_eq!(name.as_deref(), Some("Aster"));
        assert_eq!(scenes.len(), 1);
        assert_eq!(first.outputs[1].value["scene_id"], json!(scenes[0].id));
        assert_eq!(first.outputs[3].value["stage"], json!("awaiting_review"));

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
        let updated = editable
            .apply(
                CreationProposalId::new(),
                CreationTurnId::new(),
                vec![crate::CreationOperation::UpdateScene {
                    id: scenes[0].id,
                    content: "Revised welcome.".to_owned(),
                    direction: None,
                }],
                TimestampMillis::new(3),
            )
            .expect("update scene");
        let CreationDraft::Character { scenes, .. } = updated.draft else {
            panic!("character draft");
        };
        assert_eq!(scenes[0].direction.as_deref(), Some("calmly"));
    }

    #[test]
    fn undeclared_version_mismatch_and_unknown_arguments_fail_before_reduction() {
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
        assert_eq!(
            reduce(&[call("add_scene", json!({"content": "wrong target"}))]),
            Err(CreationToolContractError::UnsupportedTool)
        );
        let mut stale = call("set_persona_name", json!({"name": "Aster"}));
        stale.definition_version = 2;
        assert_eq!(
            reduce(&[stale]),
            Err(CreationToolContractError::DefinitionVersionMismatch)
        );
        assert_eq!(
            reduce(&[call(
                "set_persona_name",
                json!({"name": "Aster", "extra": true})
            )]),
            Err(CreationToolContractError::UnknownArgument)
        );
    }
}
