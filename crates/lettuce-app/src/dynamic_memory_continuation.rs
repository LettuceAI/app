use lettuce_conversations::{
    ConversationManager, GenerationAttempt, GenerationAttemptStatus, InferenceCandidate,
    InferenceOutcome, InferencePort, InferenceRequest, MessagePart, PortError, ToolExecution,
    ToolExecutionOwner, ToolExecutionRepository, ToolExecutionStatus,
    context_with_settled_tool_round,
};
use lettuce_jobs::handle::JobHandle;
use lettuce_memory::{MemoryToolOutcome, dynamic_memory_tool_request};
use lettuce_types::{ConversationId, TimestampMillis};

pub const MAX_DYNAMIC_MEMORY_TOOL_ROUNDS: u8 = 4;
pub const MAX_DYNAMIC_MEMORY_TOOL_CALLS: u16 = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicMemoryContinuationResult {
    Done {
        summary: Option<String>,
    },
    NextRound {
        executions: Vec<ToolExecution>,
        outcome: InferenceOutcome,
    },
    Complete {
        candidate: InferenceCandidate,
        outcome: InferenceOutcome,
    },
}

#[derive(Debug)]
pub struct DynamicMemoryContinuationCoordinator<'a, R: ?Sized, I: ?Sized> {
    repository: &'a R,
    inference: &'a I,
}

impl<'a, R: ToolExecutionRepository + ?Sized, I: InferencePort + ?Sized>
    DynamicMemoryContinuationCoordinator<'a, R, I>
{
    #[must_use]
    pub const fn new(repository: &'a R, inference: &'a I) -> Self {
        Self {
            repository,
            inference,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn continue_after_settled_round(
        &self,
        conversation_id: ConversationId,
        attempt: &GenerationAttempt,
        handle: &JobHandle,
        mut request: InferenceRequest,
        settled_round: &[ToolExecution],
        completed_rounds: u8,
        total_tool_calls: u16,
        at: TimestampMillis,
    ) -> Result<DynamicMemoryContinuationResult, DynamicMemoryContinuationError> {
        validate_ownership(
            conversation_id,
            attempt,
            handle,
            &request,
            completed_rounds,
            total_tool_calls,
            settled_round,
        )?;
        let durable =
            self.repository
                .list_tool_executions(conversation_id, attempt.turn_id, attempt.id)?;
        if durable.len() != usize::from(total_tool_calls)
            || durable.len() < settled_round.len()
            || durable[..durable.len() - settled_round.len()]
                .iter()
                .any(|execution| execution.status != ToolExecutionStatus::Succeeded)
            || durable[durable.len() - settled_round.len()..] != *settled_round
        {
            return Err(DynamicMemoryContinuationError::InvalidOwnership);
        }
        request.context = context_with_settled_tool_round(&request.context, settled_round)?;

        if let Some(summary) = done_summary(settled_round)? {
            return Ok(DynamicMemoryContinuationResult::Done { summary });
        }
        if handle.cancellation_token().is_cancelled() {
            return Err(DynamicMemoryContinuationError::Cancelled);
        }

        request.validate()?;
        let outcome = self.inference.run(request.clone()).await?;
        if handle.cancellation_token().is_cancelled() {
            return Err(DynamicMemoryContinuationError::Cancelled);
        }
        outcome.validate()?;
        let candidate = single_candidate(&outcome)?;

        if !candidate.tool_calls.is_empty() {
            if !candidate.parts.is_empty() {
                return Err(DynamicMemoryContinuationError::MixedToolAndContent);
            }
            if candidate.provider_replay.is_some() {
                return Err(DynamicMemoryContinuationError::UnsupportedSignedReplay);
            }
            let next_total = total_tool_calls
                .checked_add(
                    u16::try_from(candidate.tool_calls.len())
                        .map_err(|_| DynamicMemoryContinuationError::ToolBudgetExceeded)?,
                )
                .ok_or(DynamicMemoryContinuationError::ToolBudgetExceeded)?;
            if completed_rounds >= MAX_DYNAMIC_MEMORY_TOOL_ROUNDS
                || next_total > MAX_DYNAMIC_MEMORY_TOOL_CALLS
            {
                return Err(DynamicMemoryContinuationError::ToolBudgetExceeded);
            }
            let executions = ConversationManager::new(self.repository).request_tool_executions(
                ToolExecutionOwner {
                    conversation_id,
                    turn_id: attempt.turn_id,
                    attempt_id: attempt.id,
                },
                request
                    .tools
                    .as_ref()
                    .ok_or(DynamicMemoryContinuationError::InvalidRequest)?,
                candidate.tool_calls.clone(),
                at,
            )?;
            return Ok(DynamicMemoryContinuationResult::NextRound {
                executions,
                outcome,
            });
        }

        if !candidate
            .parts
            .iter()
            .any(|part| matches!(part, MessagePart::Text { text } if !text.trim().is_empty()))
        {
            return Err(DynamicMemoryContinuationError::EmptyCompletion);
        }
        Ok(DynamicMemoryContinuationResult::Complete {
            candidate: candidate.clone(),
            outcome,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_ownership(
    conversation_id: ConversationId,
    attempt: &GenerationAttempt,
    handle: &JobHandle,
    request: &InferenceRequest,
    completed_rounds: u8,
    total_tool_calls: u16,
    settled_round: &[ToolExecution],
) -> Result<(), DynamicMemoryContinuationError> {
    attempt.validate()?;
    if attempt.status != GenerationAttemptStatus::Running
        || attempt.job_id != Some(handle.id())
        || request.turn_id != attempt.turn_id
        || request.attempt_id != attempt.id
        || request.cancellation != Some(handle.id())
        || completed_rounds == 0
        || completed_rounds > MAX_DYNAMIC_MEMORY_TOOL_ROUNDS
        || total_tool_calls == 0
        || total_tool_calls > MAX_DYNAMIC_MEMORY_TOOL_CALLS
        || usize::from(total_tool_calls) < settled_round.len()
    {
        return Err(DynamicMemoryContinuationError::InvalidOwnership);
    }
    if request.tools.as_ref() != Some(&dynamic_memory_tool_request()) {
        return Err(DynamicMemoryContinuationError::InvalidRequest);
    }
    if settled_round.iter().any(|execution| {
        execution.conversation_id != conversation_id
            || execution.turn_id != attempt.turn_id
            || execution.attempt_id != attempt.id
            || execution.status != ToolExecutionStatus::Succeeded
    }) {
        return Err(DynamicMemoryContinuationError::InvalidOwnership);
    }
    Ok(())
}

fn done_summary(
    settled_round: &[ToolExecution],
) -> Result<Option<Option<String>>, DynamicMemoryContinuationError> {
    let mut summary = None;
    for execution in settled_round {
        if execution.definition_name != "done" {
            continue;
        }
        let output = execution
            .output
            .as_ref()
            .ok_or(DynamicMemoryContinuationError::InvalidDoneResult)?;
        let outcome = serde_json::from_value::<MemoryToolOutcome>(output.value.clone())
            .map_err(|_| DynamicMemoryContinuationError::InvalidDoneResult)?;
        let MemoryToolOutcome::Done {
            summary: done_summary,
        } = outcome
        else {
            return Err(DynamicMemoryContinuationError::InvalidDoneResult);
        };
        if summary.replace(done_summary).is_some() {
            return Err(DynamicMemoryContinuationError::InvalidDoneResult);
        }
    }
    Ok(summary)
}

fn single_candidate(
    outcome: &InferenceOutcome,
) -> Result<&InferenceCandidate, DynamicMemoryContinuationError> {
    if outcome.candidates.len() != 1 {
        return Err(DynamicMemoryContinuationError::MultipleCandidates);
    }
    match outcome.finish_reason {
        lettuce_conversations::FinishReason::Stop | lettuce_conversations::FinishReason::Length => {
            Ok(&outcome.candidates[0])
        }
        lettuce_conversations::FinishReason::Cancelled => {
            Err(DynamicMemoryContinuationError::Cancelled)
        }
        lettuce_conversations::FinishReason::Error => {
            Err(DynamicMemoryContinuationError::ProviderFailed)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DynamicMemoryContinuationError {
    #[error("dynamic-memory continuation ownership is invalid")]
    InvalidOwnership,
    #[error("dynamic-memory continuation request is invalid")]
    InvalidRequest,
    #[error("dynamic-memory continuation was cancelled")]
    Cancelled,
    #[error("dynamic-memory provider continuation failed")]
    ProviderFailed,
    #[error("dynamic-memory continuation returned multiple candidates")]
    MultipleCandidates,
    #[error("dynamic-memory continuation mixed tool calls with content")]
    MixedToolAndContent,
    #[error("dynamic-memory continuation requires unsupported signed replay")]
    UnsupportedSignedReplay,
    #[error("dynamic-memory continuation exceeded its tool budget")]
    ToolBudgetExceeded,
    #[error("dynamic-memory continuation returned no usable text")]
    EmptyCompletion,
    #[error("dynamic-memory done result is invalid")]
    InvalidDoneResult,
    #[error("dynamic-memory continuation contract is invalid: {0}")]
    Validation(#[from] lettuce_conversations::ValidationError),
    #[error("dynamic-memory continuation inference failed: {0}")]
    Inference(#[from] PortError),
    #[error("dynamic-memory continuation persistence failed: {0}")]
    Conversation(#[from] lettuce_conversations::ConversationServiceError),
    #[error("dynamic-memory continuation repository failed: {0}")]
    Repository(#[from] lettuce_conversations::ConversationRepositoryError),
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::{
        FinishReason, InferenceUsage, InferenceWarningCode, ToolExecutionOwner, ToolOutput,
    };
    use lettuce_memory::MemoryToolArguments;
    use lettuce_types::{GenerationAttemptId, GenerationTurnId, ToolExecutionId};
    use serde_json::json;

    use super::*;

    fn settled_done(summary: Option<&str>) -> ToolExecution {
        let owner = ToolExecutionOwner {
            conversation_id: ConversationId::new(),
            turn_id: GenerationTurnId::new(),
            attempt_id: GenerationAttemptId::new(),
        };
        let definition = dynamic_memory_tool_request()
            .definitions
            .into_iter()
            .find(|definition| definition.name == "done")
            .expect("done definition");
        let requested = ToolExecution::requested(
            ToolExecutionId::new(),
            owner,
            0,
            &definition,
            lettuce_conversations::ProposedToolCall {
                provider_call_id: Some("done-1".to_owned()),
                name: "done".to_owned(),
                arguments: match summary {
                    Some(summary) => json!({"summary": summary}),
                    None => json!({}),
                },
                raw_arguments: None,
                provider_replay: None,
            },
            TimestampMillis::new(1),
        )
        .expect("requested");
        let validated = requested
            .transition(
                ToolExecutionStatus::Validated,
                None,
                None,
                TimestampMillis::new(2),
            )
            .expect("validated");
        let running = validated
            .transition(
                ToolExecutionStatus::Running,
                None,
                None,
                TimestampMillis::new(3),
            )
            .expect("running");
        running
            .transition(
                ToolExecutionStatus::Succeeded,
                Some(ToolOutput {
                    value: serde_json::to_value(MemoryToolOutcome::Done {
                        summary: summary.map(str::to_owned),
                    })
                    .expect("outcome"),
                    is_error: false,
                }),
                None,
                TimestampMillis::new(4),
            )
            .expect("succeeded")
    }

    #[test]
    fn done_result_stops_provider_continuation_with_exact_summary() {
        assert_eq!(
            done_summary(&[settled_done(Some("finished"))]).expect("done"),
            Some(Some("finished".to_owned()))
        );
        assert_eq!(
            done_summary(&[settled_done(None)]).expect("done"),
            Some(None)
        );
    }

    #[test]
    fn malformed_done_result_fails_closed() {
        let mut execution = settled_done(None);
        execution.output = Some(ToolOutput {
            value: serde_json::to_value(MemoryToolArguments::Done { summary: None })
                .expect("arguments"),
            is_error: false,
        });
        assert!(matches!(
            done_summary(&[execution]),
            Err(DynamicMemoryContinuationError::InvalidDoneResult)
        ));
    }

    #[test]
    fn continuation_requires_one_successful_provider_candidate() {
        let outcome = InferenceOutcome {
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts: vec![MessagePart::Text {
                    text: "complete".to_owned(),
                }],
                tool_calls: vec![],
                provider_replay: None,
            }],
            usage: Some(InferenceUsage {
                input_tokens: 4,
                output_tokens: 1,
            }),
            finish_reason: FinishReason::Stop,
            provider_finish_reason: None,
            provider_request_id: None,
            warning_codes: Vec::<InferenceWarningCode>::new(),
        };
        assert_eq!(single_candidate(&outcome).expect("candidate").ordinal, 0);

        let mut failed = outcome;
        failed.finish_reason = FinishReason::Error;
        assert!(matches!(
            single_candidate(&failed),
            Err(DynamicMemoryContinuationError::ProviderFailed)
        ));
    }

    #[test]
    fn continuation_limits_are_small_and_explicit() {
        assert_eq!(MAX_DYNAMIC_MEMORY_TOOL_ROUNDS, 4);
        assert_eq!(MAX_DYNAMIC_MEMORY_TOOL_CALLS, 64);
    }
}
