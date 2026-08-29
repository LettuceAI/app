use lettuce_types::{
    ConversationId, GenerationAttemptId, GenerationTurnId, Revision, TimestampMillis,
    ToolExecutionId,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::validation::{validate_collection, validate_text, validate_unique};
use crate::{ReplayArtifactRef, ValidationError};

pub const MAX_TOOL_DEFINITIONS: usize = 64;
pub const MAX_TOOL_NAME_BYTES: usize = 64;
pub const MAX_TOOL_DESCRIPTION_BYTES: usize = 4096;
pub const MAX_TOOL_SCHEMA_BYTES: usize = 64 * 1024;
pub const MAX_TOOL_REQUEST_BYTES: usize = 1024 * 1024;
pub const MAX_TOOL_ARGUMENT_BYTES: usize = 256 * 1024;
pub const MAX_TOOL_RESULT_BYTES: usize = 1024 * 1024;
pub const MAX_TOOL_CALLS_PER_RESPONSE: usize = 64;
pub const MAX_PROVIDER_TOOL_CALL_ID_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolDefinition {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Value,
    /// Stable application handler version. Provider schemas do not receive it.
    pub version: u32,
}

impl ToolDefinition {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_tool_name("tool_definition.name", &self.name)?;
        if let Some(description) = &self.description {
            validate_text(
                "tool_definition.description",
                description,
                MAX_TOOL_DESCRIPTION_BYTES,
                false,
            )?;
        }
        if self.version == 0 {
            return Err(ValidationError::UnsupportedVersion {
                field: "tool_definition.version",
                version: self.version,
            });
        }
        validate_json_object(
            "tool_definition.parameters",
            &self.parameters,
            MAX_TOOL_SCHEMA_BYTES,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ToolChoice {
    Auto,
    Required,
    Named { name: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolRequest {
    pub definitions: Vec<ToolDefinition>,
    pub choice: ToolChoice,
}

impl ToolRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.definitions.is_empty() {
            return Err(ValidationError::InvalidValue {
                field: "tool_request.definitions",
            });
        }
        validate_collection(
            "tool_request.definitions",
            &self.definitions,
            MAX_TOOL_DEFINITIONS,
        )?;
        for definition in &self.definitions {
            definition.validate()?;
        }
        let encoded_size = serde_json::to_vec(self)
            .map_err(|_| ValidationError::InvalidValue {
                field: "tool_request",
            })?
            .len();
        if encoded_size > MAX_TOOL_REQUEST_BYTES {
            return Err(ValidationError::TooLarge {
                field: "tool_request",
            });
        }
        validate_unique(
            "tool_request.definition_names",
            self.definitions
                .iter()
                .map(|definition| definition.name.as_str()),
        )?;
        if let ToolChoice::Named { name } = &self.choice {
            validate_tool_name("tool_request.choice.name", name)?;
            if !self
                .definitions
                .iter()
                .any(|definition| definition.name == *name)
            {
                return Err(ValidationError::InvalidReference {
                    field: "tool_request.choice.name",
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposedToolCall {
    pub provider_call_id: Option<String>,
    pub name: String,
    pub arguments: Value,
    /// Preserved for provider-faithful replay; execution uses `arguments`.
    pub raw_arguments: Option<String>,
    pub provider_replay: Option<ReplayArtifactRef>,
}

impl ProposedToolCall {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_tool_name("proposed_tool_call.name", &self.name)?;
        validate_provider_call_id(self.provider_call_id.as_deref())?;
        validate_json_object(
            "proposed_tool_call.arguments",
            &self.arguments,
            MAX_TOOL_ARGUMENT_BYTES,
        )?;
        if let Some(raw) = &self.raw_arguments {
            validate_text(
                "proposed_tool_call.raw_arguments",
                raw,
                MAX_TOOL_ARGUMENT_BYTES,
                false,
            )?;
            let parsed =
                serde_json::from_str::<Value>(raw).map_err(|_| ValidationError::InvalidValue {
                    field: "proposed_tool_call.raw_arguments",
                })?;
            if parsed != self.arguments {
                return Err(ValidationError::Invariant {
                    field: "proposed_tool_call.raw_arguments",
                });
            }
        }
        if let Some(replay) = &self.provider_replay {
            replay.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolOutput {
    pub value: Value,
    pub is_error: bool,
}

impl ToolOutput {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_json("tool_output.value", &self.value, MAX_TOOL_RESULT_BYTES)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptToolCall {
    pub execution_id: ToolExecutionId,
    pub provider_call_id: Option<String>,
    pub name: String,
    pub arguments: Value,
    pub raw_arguments: Option<String>,
    pub provider_replay: Option<ReplayArtifactRef>,
}

impl TranscriptToolCall {
    pub fn validate(&self) -> Result<(), ValidationError> {
        ProposedToolCall {
            provider_call_id: self.provider_call_id.clone(),
            name: self.name.clone(),
            arguments: self.arguments.clone(),
            raw_arguments: self.raw_arguments.clone(),
            provider_replay: self.provider_replay.clone(),
        }
        .validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptToolResult {
    pub execution_id: ToolExecutionId,
    pub provider_call_id: Option<String>,
    pub name: String,
    pub output: ToolOutput,
}

impl TranscriptToolResult {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_tool_name("transcript_tool_result.name", &self.name)?;
        validate_provider_call_id(self.provider_call_id.as_deref())?;
        self.output.validate()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutionStatus {
    Requested,
    Validated,
    Running,
    Succeeded,
    Rejected,
    Failed,
    Cancelled,
    Interrupted,
}

impl ToolExecutionStatus {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Rejected | Self::Failed | Self::Cancelled | Self::Interrupted
        )
    }

    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        use ToolExecutionStatus::*;
        matches!(
            (self, next),
            (Requested, Validated | Rejected | Cancelled)
                | (Validated, Running | Rejected | Cancelled)
                | (Running, Succeeded | Failed | Cancelled | Interrupted)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolFailureCode {
    InvalidArguments,
    UndeclaredTool,
    PermissionDenied,
    HandlerFailed,
    ResultInvalid,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolFailure {
    pub code: ToolFailureCode,
    pub message: Option<String>,
}

impl ToolFailure {
    fn validate(&self) -> Result<(), ValidationError> {
        if let Some(message) = &self.message {
            validate_text(
                "tool_failure.message",
                message,
                MAX_TOOL_DESCRIPTION_BYTES,
                false,
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolExecutionOwner {
    pub conversation_id: ConversationId,
    pub turn_id: GenerationTurnId,
    pub attempt_id: GenerationAttemptId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolExecution {
    pub id: ToolExecutionId,
    pub conversation_id: ConversationId,
    pub turn_id: GenerationTurnId,
    pub attempt_id: GenerationAttemptId,
    pub ordinal: u16,
    pub definition_name: String,
    pub definition_version: u32,
    pub provider_call_id: Option<String>,
    pub arguments: Value,
    pub raw_arguments: Option<String>,
    pub provider_replay: Option<ReplayArtifactRef>,
    pub status: ToolExecutionStatus,
    pub output: Option<ToolOutput>,
    pub failure: Option<ToolFailure>,
    pub revision: Revision,
    pub requested_at: TimestampMillis,
    pub started_at: Option<TimestampMillis>,
    pub finished_at: Option<TimestampMillis>,
    pub updated_at: TimestampMillis,
}

impl ToolExecution {
    pub fn requested(
        id: ToolExecutionId,
        owner: ToolExecutionOwner,
        ordinal: u16,
        definition: &ToolDefinition,
        call: ProposedToolCall,
        at: TimestampMillis,
    ) -> Result<Self, ValidationError> {
        definition.validate()?;
        call.validate()?;
        if call.name != definition.name {
            return Err(ValidationError::InvalidReference {
                field: "tool_execution.definition",
            });
        }
        let execution = Self {
            id,
            conversation_id: owner.conversation_id,
            turn_id: owner.turn_id,
            attempt_id: owner.attempt_id,
            ordinal,
            definition_name: definition.name.clone(),
            definition_version: definition.version,
            provider_call_id: call.provider_call_id,
            arguments: call.arguments,
            raw_arguments: call.raw_arguments,
            provider_replay: call.provider_replay,
            status: ToolExecutionStatus::Requested,
            output: None,
            failure: None,
            revision: Revision::INITIAL,
            requested_at: at,
            started_at: None,
            finished_at: None,
            updated_at: at,
        };
        execution.validate()?;
        Ok(execution)
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_tool_name("tool_execution.definition_name", &self.definition_name)?;
        validate_provider_call_id(self.provider_call_id.as_deref())?;
        if self.definition_version == 0 {
            return Err(ValidationError::UnsupportedVersion {
                field: "tool_execution.definition_version",
                version: self.definition_version,
            });
        }
        validate_json_object(
            "tool_execution.arguments",
            &self.arguments,
            MAX_TOOL_ARGUMENT_BYTES,
        )?;
        if let Some(raw) = &self.raw_arguments {
            validate_text(
                "tool_execution.raw_arguments",
                raw,
                MAX_TOOL_ARGUMENT_BYTES,
                false,
            )?;
            let parsed =
                serde_json::from_str::<Value>(raw).map_err(|_| ValidationError::InvalidValue {
                    field: "tool_execution.raw_arguments",
                })?;
            if parsed != self.arguments {
                return Err(ValidationError::Invariant {
                    field: "tool_execution.raw_arguments",
                });
            }
        }
        if let Some(replay) = &self.provider_replay {
            replay.validate()?;
        }
        if let Some(output) = &self.output {
            output.validate()?;
        }
        if let Some(failure) = &self.failure {
            failure.validate()?;
        }
        if self.revision.get() == 0 || self.requested_at > self.updated_at {
            return Err(ValidationError::Invariant {
                field: "tool_execution.revision_timestamps",
            });
        }
        if self
            .started_at
            .is_some_and(|started| started < self.requested_at)
            || self
                .finished_at
                .is_some_and(|finished| finished < self.requested_at)
            || self
                .started_at
                .is_some_and(|started| started > self.updated_at)
            || self
                .finished_at
                .is_some_and(|finished| finished > self.updated_at)
            || matches!((self.started_at, self.finished_at), (Some(started), Some(finished)) if started > finished)
        {
            return Err(ValidationError::InvalidTimestampOrder {
                field: "tool_execution.timestamps",
            });
        }
        let terminal = self.status.is_terminal();
        if terminal != self.finished_at.is_some()
            || matches!(self.status, ToolExecutionStatus::Running)
                != self.started_at.is_some_and(|_| self.finished_at.is_none())
            || matches!(
                self.status,
                ToolExecutionStatus::Requested | ToolExecutionStatus::Validated
            ) && (self.started_at.is_some() || self.finished_at.is_some())
            || self.output.is_some() != matches!(self.status, ToolExecutionStatus::Succeeded)
            || self.failure.is_some()
                != matches!(
                    self.status,
                    ToolExecutionStatus::Rejected | ToolExecutionStatus::Failed
                )
        {
            return Err(ValidationError::Invariant {
                field: "tool_execution.status_payload",
            });
        }
        Ok(())
    }

    pub fn transition(
        mut self,
        next: ToolExecutionStatus,
        output: Option<ToolOutput>,
        failure: Option<ToolFailure>,
        at: TimestampMillis,
    ) -> Result<Self, ValidationError> {
        if !self.status.can_transition_to(next) {
            return Err(ValidationError::IllegalTransition {
                field: "tool_execution.status",
            });
        }
        if at < self.updated_at {
            return Err(ValidationError::InvalidTimestampOrder {
                field: "tool_execution.updated_at",
            });
        }
        self.status = next;
        self.output = output;
        self.failure = failure;
        if next == ToolExecutionStatus::Running {
            self.started_at = Some(at);
        }
        if next.is_terminal() {
            self.finished_at = Some(at);
        }
        self.updated_at = at;
        self.revision = self
            .revision
            .next()
            .map_err(|_| ValidationError::OutOfBounds {
                field: "tool_execution.revision",
            })?;
        self.validate()?;
        Ok(self)
    }
}

pub trait ToolExecutionRepository: Send + Sync {
    /// Appends one provider response's ordered call set atomically and returns
    /// the stored executions in the same order. The expected ordinal prevents
    /// concurrent continuation or recovery workers from interleaving rounds.
    fn append_tool_executions(
        &self,
        expected_next_ordinal: u16,
        executions: &[ToolExecution],
    ) -> Result<Vec<ToolExecution>, crate::ConversationRepositoryError>;

    fn get_tool_execution(
        &self,
        id: ToolExecutionId,
    ) -> Result<ToolExecution, crate::ConversationRepositoryError>;

    fn list_tool_executions(
        &self,
        conversation_id: ConversationId,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
    ) -> Result<Vec<ToolExecution>, crate::ConversationRepositoryError>;

    fn transition_tool_execution(
        &self,
        id: ToolExecutionId,
        expected_revision: Revision,
        next: ToolExecutionStatus,
        output: Option<ToolOutput>,
        failure: Option<ToolFailure>,
        at: TimestampMillis,
    ) -> Result<ToolExecution, crate::ConversationRepositoryError>;
}

pub(crate) fn validate_tool_name(field: &'static str, value: &str) -> Result<(), ValidationError> {
    validate_text(field, value, MAX_TOOL_NAME_BYTES, false)?;
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(ValidationError::InvalidValue { field });
    }
    Ok(())
}

fn validate_provider_call_id(value: Option<&str>) -> Result<(), ValidationError> {
    if let Some(value) = value {
        validate_text(
            "tool_execution.provider_call_id",
            value,
            MAX_PROVIDER_TOOL_CALL_ID_BYTES,
            false,
        )?;
    }
    Ok(())
}

fn validate_json_object(
    field: &'static str,
    value: &Value,
    max_bytes: usize,
) -> Result<(), ValidationError> {
    if !value.is_object() {
        return Err(ValidationError::InvalidValue { field });
    }
    validate_json(field, value, max_bytes)
}

fn validate_json(
    field: &'static str,
    value: &Value,
    max_bytes: usize,
) -> Result<(), ValidationError> {
    let size = serde_json::to_vec(value)
        .map_err(|_| ValidationError::InvalidValue { field })?
        .len();
    if size > max_bytes {
        return Err(ValidationError::TooLarge { field });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use lettuce_types::ToolExecutionId;
    use serde_json::json;

    use super::*;

    fn definition(name: &str) -> ToolDefinition {
        ToolDefinition {
            name: name.to_owned(),
            description: Some("Stores one memory".to_owned()),
            parameters: json!({"type": "object", "properties": {}}),
            version: 1,
        }
    }

    #[test]
    fn request_requires_unique_valid_names_and_a_declared_named_choice() {
        let request = ToolRequest {
            definitions: vec![definition("create_memory")],
            choice: ToolChoice::Named {
                name: "create_memory".to_owned(),
            },
        };
        request.validate().expect("valid request");

        let mut duplicate = request.clone();
        duplicate.definitions.push(definition("create_memory"));
        assert!(matches!(
            duplicate.validate(),
            Err(ValidationError::Duplicate { .. })
        ));

        let mut missing = request;
        missing.choice = ToolChoice::Named {
            name: "delete_memory".to_owned(),
        };
        assert!(matches!(
            missing.validate(),
            Err(ValidationError::InvalidReference { .. })
        ));
    }

    #[test]
    fn proposed_calls_reject_text_arguments_and_invalid_names() {
        let call = ProposedToolCall {
            provider_call_id: Some("call-1".to_owned()),
            name: "create memory".to_owned(),
            arguments: json!("not an object"),
            raw_arguments: None,
            provider_replay: None,
        };
        assert!(call.validate().is_err());
    }

    #[test]
    fn terminal_states_do_not_regress() {
        assert!(ToolExecutionStatus::Requested.can_transition_to(ToolExecutionStatus::Validated));
        assert!(ToolExecutionStatus::Validated.can_transition_to(ToolExecutionStatus::Running));
        assert!(ToolExecutionStatus::Running.can_transition_to(ToolExecutionStatus::Succeeded));
        assert!(!ToolExecutionStatus::Succeeded.can_transition_to(ToolExecutionStatus::Running));
    }

    #[test]
    fn raw_arguments_must_be_valid_and_equal_to_the_canonical_object() {
        let call = ProposedToolCall {
            provider_call_id: Some("call-1".to_owned()),
            name: "create_memory".to_owned(),
            arguments: json!({"content": "one"}),
            raw_arguments: Some(r#"{"content":"two"}"#.to_owned()),
            provider_replay: None,
        };
        assert!(matches!(
            call.validate(),
            Err(ValidationError::Invariant { .. })
        ));
    }

    #[test]
    fn requested_execution_copies_the_declared_handler_version_and_wire_replay() {
        let definition = ToolDefinition {
            version: 7,
            ..definition("create_memory")
        };
        let call = ProposedToolCall {
            provider_call_id: Some("call-1".to_owned()),
            name: "create_memory".to_owned(),
            arguments: json!({"content": "one"}),
            raw_arguments: Some(r#"{"content":"one"}"#.to_owned()),
            provider_replay: None,
        };
        let execution = ToolExecution::requested(
            ToolExecutionId::new(),
            ToolExecutionOwner {
                conversation_id: ConversationId::new(),
                turn_id: GenerationTurnId::new(),
                attempt_id: GenerationAttemptId::new(),
            },
            2,
            &definition,
            call,
            TimestampMillis::new(10),
        )
        .expect("requested execution");

        assert_eq!(execution.definition_version, 7);
        assert_eq!(execution.ordinal, 2);
        assert_eq!(execution.status, ToolExecutionStatus::Requested);
        assert_eq!(execution.revision, Revision::INITIAL);
    }

    #[test]
    fn requested_execution_rejects_a_mismatched_definition() {
        let call = ProposedToolCall {
            provider_call_id: None,
            name: "delete_memory".to_owned(),
            arguments: json!({"id": "one"}),
            raw_arguments: None,
            provider_replay: None,
        };
        assert!(matches!(
            ToolExecution::requested(
                ToolExecutionId::new(),
                ToolExecutionOwner {
                    conversation_id: ConversationId::new(),
                    turn_id: GenerationTurnId::new(),
                    attempt_id: GenerationAttemptId::new(),
                },
                0,
                &definition("create_memory"),
                call,
                TimestampMillis::new(10),
            ),
            Err(ValidationError::InvalidReference { .. })
        ));
    }

    #[test]
    fn provider_context_requires_ordered_matching_call_and_result() {
        let execution_id = ToolExecutionId::new();
        let call = TranscriptToolCall {
            execution_id,
            provider_call_id: Some("call-1".to_owned()),
            name: "create_memory".to_owned(),
            arguments: json!({"content": "one"}),
            raw_arguments: None,
            provider_replay: None,
        };
        let result = TranscriptToolResult {
            execution_id,
            provider_call_id: Some("call-1".to_owned()),
            name: "create_memory".to_owned(),
            output: ToolOutput {
                value: json!({"memory_id": "one"}),
                is_error: false,
            },
        };
        let valid = crate::ProviderNeutralContext {
            messages: vec![
                crate::ProviderNeutralMessage {
                    role: crate::MessageRole::Assistant,
                    parts: vec![crate::ProviderContextPart::ToolCall(call.clone())],
                },
                crate::ProviderNeutralMessage {
                    role: crate::MessageRole::User,
                    parts: vec![crate::ProviderContextPart::ToolResult(result.clone())],
                },
            ],
            attributions: crate::ContextAttributions::default(),
            budget: crate::ContextBudgetReport::default(),
        };
        valid.validate().expect("ordered transcript");

        let orphan = crate::ProviderNeutralContext {
            messages: vec![crate::ProviderNeutralMessage {
                role: crate::MessageRole::User,
                parts: vec![crate::ProviderContextPart::ToolResult(result)],
            }],
            attributions: crate::ContextAttributions::default(),
            budget: crate::ContextBudgetReport::default(),
        };
        assert!(matches!(
            orphan.validate(),
            Err(ValidationError::InvalidReference { .. })
        ));
    }
}
