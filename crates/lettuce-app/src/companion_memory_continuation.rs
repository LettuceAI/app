use lettuce_conversations::{
    GenerationOperation, InferencePort, InferenceRequest, MessageRole, PortError,
    ProviderContextPart, ProviderNeutralMessage, ProviderReplayArtifactPort, ToolOutput,
    TranscriptToolCall, TranscriptToolResult,
};
use lettuce_jobs::handle::JobHandle;
use lettuce_memory::{
    DynamicMemoryAttempt, DynamicMemoryAttemptStatus, DynamicMemoryInferenceRound,
    DynamicMemoryRun, DynamicMemoryRunRepository, DynamicMemoryRunRepositoryError,
    MemoryToolOutcome,
};
use lettuce_types::{
    DynamicMemoryAttemptId, DynamicMemoryRunId, GenerationAttemptId, GenerationTurnId, RequestId,
    TimestampMillis,
};

use crate::{cleanup_outcome_replays, plan_memory_round, run_memory_request_with_fallback};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompanionMemoryContinuationResult {
    Done {
        summary: Option<String>,
    },
    NextRound {
        round: Box<DynamicMemoryInferenceRound>,
        replayed: bool,
    },
}

#[derive(Debug)]
pub struct CompanionMemoryContinuationCoordinator<'a, R: ?Sized, I: ?Sized> {
    repository: &'a R,
    inference: &'a I,
}

impl<
    'a,
    R: DynamicMemoryRunRepository + ProviderReplayArtifactPort + ?Sized,
    I: InferencePort + ?Sized,
> CompanionMemoryContinuationCoordinator<'a, R, I>
{
    #[must_use]
    pub const fn new(repository: &'a R, inference: &'a I) -> Self {
        Self {
            repository,
            inference,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn continue_after_round(
        &self,
        run_id: DynamicMemoryRunId,
        attempt_id: DynamicMemoryAttemptId,
        settled_round_ordinal: u8,
        handle: &JobHandle,
        stream_sink: Option<RequestId>,
        now: TimestampMillis,
    ) -> Result<CompanionMemoryContinuationResult, CompanionMemoryContinuationError> {
        let run = self.repository.load_dynamic_memory_run(run_id)?;
        let attempt = self.repository.load_dynamic_memory_attempt(attempt_id)?;
        validate_owner(&run, &attempt, handle)?;
        let rounds = self
            .repository
            .list_dynamic_memory_inference_rounds(run_id, attempt_id)?;
        let settled_round = rounds
            .get(usize::from(settled_round_ordinal))
            .ok_or(CompanionMemoryContinuationError::InvalidOwnership)?;
        if settled_round.ordinal != settled_round_ordinal {
            return Err(CompanionMemoryContinuationError::InvalidOwnership);
        }
        let settlement = self
            .repository
            .load_dynamic_memory_round_settlement(run_id, attempt_id, settled_round_ordinal)?
            .ok_or(CompanionMemoryContinuationError::UnsettledRound)?;
        if settlement.run_id != run.id
            || settlement.attempt_id != attempt.id
            || settlement.round_ordinal != settled_round.ordinal
            || settlement.results.len() != settled_round.calls.len()
            || settled_round
                .calls
                .iter()
                .zip(&settlement.results)
                .any(|(call, result)| call.id != result.execution_id)
        {
            return Err(CompanionMemoryContinuationError::InvalidSettlement);
        }
        if let Some(summary) = done_summary(settled_round, &settlement.results)? {
            return Ok(CompanionMemoryContinuationResult::Done { summary });
        }
        let context = context_after_settlement(settled_round, &settlement.results)?;
        let next_ordinal = settled_round_ordinal
            .checked_add(1)
            .ok_or(CompanionMemoryContinuationError::RoundLimit)?;
        if let Some(next) = rounds.get(usize::from(next_ordinal)) {
            return Ok(CompanionMemoryContinuationResult::NextRound {
                round: Box::new(next.clone()),
                replayed: true,
            });
        }
        if next_ordinal >= lettuce_memory::MAX_DYNAMIC_MEMORY_INFERENCE_ROUNDS {
            return Err(CompanionMemoryContinuationError::RoundLimit);
        }
        if handle.cancellation_token().is_cancelled() {
            self.cancel(&attempt, now)?;
            return Err(CompanionMemoryContinuationError::Cancelled);
        }
        let request = InferenceRequest {
            turn_id: GenerationTurnId::from_uuid(run.id.as_uuid()),
            attempt_id: GenerationAttemptId::from_uuid(attempt.id.as_uuid()),
            operation: GenerationOperation::Send,
            profile: run.profile.clone(),
            context: context.clone(),
            cancellation: Some(handle.id()),
            stream_sink,
            media_grants: Vec::new(),
            tools: Some(run.tool_request.clone()),
        };
        request
            .validate()
            .map_err(|_| CompanionMemoryContinuationError::InvalidRequest)?;
        let outcome = match run_memory_request_with_fallback(
            self.inference,
            request,
            run.structured_fallback_format,
            |outcome| cleanup_outcome_replays(self.repository, outcome),
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(crate::CompanionMemoryInferenceError::Cancelled) => {
                self.cancel(&attempt, now)?;
                return Err(CompanionMemoryContinuationError::Cancelled);
            }
            Err(crate::CompanionMemoryInferenceError::Inference(error)) => {
                return Err(CompanionMemoryContinuationError::Inference(error));
            }
            Err(error) => return Err(CompanionMemoryContinuationError::InvalidOutcome(error)),
        };
        if handle.cancellation_token().is_cancelled() {
            cleanup_outcome_replays(self.repository, &outcome)
                .map_err(|_| CompanionMemoryContinuationError::ReplayCleanup)?;
            self.cancel(&attempt, now)?;
            return Err(CompanionMemoryContinuationError::Cancelled);
        }
        let planned = match plan_memory_round(&run, next_ordinal, context, &outcome, now) {
            Ok(round) => round,
            Err(crate::CompanionMemoryInferenceError::Cancelled) => {
                cleanup_outcome_replays(self.repository, &outcome)
                    .map_err(|_| CompanionMemoryContinuationError::ReplayCleanup)?;
                self.cancel(&attempt, now)?;
                return Err(CompanionMemoryContinuationError::Cancelled);
            }
            Err(error) => {
                cleanup_outcome_replays(self.repository, &outcome)
                    .map_err(|_| CompanionMemoryContinuationError::ReplayCleanup)?;
                return Err(CompanionMemoryContinuationError::InvalidOutcome(error));
            }
        };
        let expected_next_call = settled_round
            .first_call_ordinal
            .checked_add(
                u16::try_from(settled_round.calls.len())
                    .map_err(|_| CompanionMemoryContinuationError::CallLimit)?,
            )
            .ok_or(CompanionMemoryContinuationError::CallLimit)?;
        let admitted = match self.repository.admit_dynamic_memory_inference_round(
            run.id,
            attempt.id,
            next_ordinal,
            expected_next_call,
            planned,
        ) {
            Ok(round) => round,
            Err(error) => {
                cleanup_outcome_replays(self.repository, &outcome)
                    .map_err(|_| CompanionMemoryContinuationError::ReplayCleanup)?;
                return Err(CompanionMemoryContinuationError::Run(error));
            }
        };
        Ok(CompanionMemoryContinuationResult::NextRound {
            round: Box::new(admitted),
            replayed: false,
        })
    }

    fn cancel(
        &self,
        attempt: &DynamicMemoryAttempt,
        now: TimestampMillis,
    ) -> Result<(), CompanionMemoryContinuationError> {
        self.repository.transition_dynamic_memory_attempt(
            attempt.id,
            attempt.revision,
            DynamicMemoryAttemptStatus::Cancelled,
            None,
            now,
        )?;
        Ok(())
    }
}

fn validate_owner(
    run: &DynamicMemoryRun,
    attempt: &DynamicMemoryAttempt,
    handle: &JobHandle,
) -> Result<(), CompanionMemoryContinuationError> {
    if attempt.run_id != run.id
        || attempt.job_id != handle.id()
        || attempt.status != DynamicMemoryAttemptStatus::Processing
    {
        return Err(CompanionMemoryContinuationError::InvalidOwnership);
    }
    Ok(())
}

fn context_after_settlement(
    round: &DynamicMemoryInferenceRound,
    results: &[lettuce_memory::MemoryToolResult],
) -> Result<lettuce_conversations::ProviderNeutralContext, CompanionMemoryContinuationError> {
    round
        .request_context
        .validate()
        .map_err(|_| CompanionMemoryContinuationError::InvalidRequest)?;
    let mut calls = Vec::with_capacity(round.calls.len());
    let mut outputs = Vec::with_capacity(round.calls.len());
    for (call, result) in round.calls.iter().zip(results) {
        if call.id != result.execution_id {
            return Err(CompanionMemoryContinuationError::InvalidSettlement);
        }
        calls.push(ProviderContextPart::ToolCall(TranscriptToolCall {
            execution_id: call.id,
            provider_call_id: call.call.provider_call_id.clone(),
            name: call.call.name.clone(),
            arguments: call.call.arguments.clone(),
            raw_arguments: call.call.raw_arguments.clone(),
            provider_replay: call.call.provider_replay.clone(),
        }));
        outputs.push(ProviderContextPart::ToolResult(TranscriptToolResult {
            execution_id: call.id,
            provider_call_id: call.call.provider_call_id.clone(),
            name: call.call.name.clone(),
            output: ToolOutput {
                value: serde_json::to_value(&result.outcome)
                    .map_err(|_| CompanionMemoryContinuationError::InvalidSettlement)?,
                is_error: matches!(result.outcome, MemoryToolOutcome::Rejected { .. }),
            },
        }));
    }
    if calls.len() != results.len() {
        return Err(CompanionMemoryContinuationError::InvalidSettlement);
    }
    let mut context = round.request_context.clone();
    context.messages.push(ProviderNeutralMessage {
        role: MessageRole::Assistant,
        parts: calls,
    });
    context.messages.push(ProviderNeutralMessage {
        role: MessageRole::User,
        parts: outputs,
    });
    context
        .validate()
        .map_err(|_| CompanionMemoryContinuationError::InvalidRequest)?;
    Ok(context)
}

fn done_summary(
    round: &DynamicMemoryInferenceRound,
    results: &[lettuce_memory::MemoryToolResult],
) -> Result<Option<Option<String>>, CompanionMemoryContinuationError> {
    let mut summary = None;
    for (call, result) in round.calls.iter().zip(results) {
        if call.call.name != "done" {
            continue;
        }
        let MemoryToolOutcome::Done {
            summary: done_summary,
        } = &result.outcome
        else {
            return Err(CompanionMemoryContinuationError::InvalidSettlement);
        };
        if summary.replace(done_summary.clone()).is_some() {
            return Err(CompanionMemoryContinuationError::InvalidSettlement);
        }
    }
    Ok(summary)
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionMemoryContinuationError {
    #[error("background memory continuation ownership is invalid")]
    InvalidOwnership,
    #[error("background memory continuation request is invalid")]
    InvalidRequest,
    #[error("background memory round is not settled")]
    UnsettledRound,
    #[error("background memory settlement is invalid")]
    InvalidSettlement,
    #[error("background memory continuation reached its round limit")]
    RoundLimit,
    #[error("background memory continuation reached its call limit")]
    CallLimit,
    #[error("background memory continuation was cancelled")]
    Cancelled,
    #[error("background memory continuation provider failed: {0}")]
    Inference(PortError),
    #[error("background memory continuation outcome is invalid: {0}")]
    InvalidOutcome(crate::CompanionMemoryInferenceError),
    #[error("background memory continuation replay cleanup failed")]
    ReplayCleanup,
    #[error("background memory continuation persistence failed: {0}")]
    Run(#[from] DynamicMemoryRunRepositoryError),
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::{
        ContextAttributions, ContextBudgetReport, ProposedToolCall, ProviderNeutralContext,
    };
    use lettuce_memory::{
        DynamicMemoryRoundFinishReason, DynamicMemoryToolCallEvidence, MemoryToolResult,
    };
    use lettuce_types::{DynamicMemoryAttemptId, DynamicMemoryRunId, ToolExecutionId};
    use serde_json::json;

    use super::*;

    fn round_and_results(
        name: &str,
        arguments: serde_json::Value,
        outcome: MemoryToolOutcome,
    ) -> (DynamicMemoryInferenceRound, Vec<MemoryToolResult>) {
        let run_id = DynamicMemoryRunId::new();
        let attempt_id = DynamicMemoryAttemptId::new();
        let call_id = ToolExecutionId::new();
        let admitted_at = TimestampMillis::new(1);
        (
            DynamicMemoryInferenceRound {
                run_id,
                attempt_id,
                ordinal: 0,
                first_call_ordinal: 0,
                request_context: ProviderNeutralContext {
                    messages: vec![ProviderNeutralMessage {
                        role: MessageRole::User,
                        parts: vec![ProviderContextPart::Text {
                            text: "frozen request".into(),
                        }],
                    }],
                    attributions: ContextAttributions::default(),
                    budget: ContextBudgetReport {
                        selected_messages: 1,
                        input_bytes: 14,
                        estimated_input_tokens: 4,
                        ..Default::default()
                    },
                },
                parts: Vec::new(),
                provider_replay: None,
                usage: None,
                finish_reason: DynamicMemoryRoundFinishReason::Stop,
                provider_request_id: None,
                calls: vec![DynamicMemoryToolCallEvidence {
                    id: call_id,
                    run_id,
                    attempt_id,
                    round_ordinal: 0,
                    ordinal: 0,
                    definition_version: 1,
                    call: ProposedToolCall {
                        provider_call_id: Some("provider-call".into()),
                        name: name.into(),
                        arguments,
                        raw_arguments: None,
                        provider_replay: None,
                    },
                    admitted_at,
                }],
                admitted_at,
            },
            vec![MemoryToolResult {
                execution_id: call_id,
                outcome,
            }],
        )
    }

    #[test]
    fn settled_round_replays_native_call_and_typed_result_in_order() {
        let target = lettuce_types::MemoryId::new();
        let (round, results) = round_and_results(
            "pin_memory",
            json!({"id":target.to_string()}),
            MemoryToolOutcome::TargetNotFound { id: target },
        );
        let context = context_after_settlement(&round, &results).expect("context");
        assert_eq!(context.messages.len(), 3);
        assert!(matches!(
            &context.messages[1].parts[..],
            [ProviderContextPart::ToolCall(call)]
                if call.execution_id == round.calls[0].id
                    && call.provider_call_id.as_deref() == Some("provider-call")
        ));
        assert!(matches!(
            &context.messages[2].parts[..],
            [ProviderContextPart::ToolResult(result)]
                if result.execution_id == round.calls[0].id
                    && !result.output.is_error
                    && result.output.value == json!({"status":"target_not_found","id":target})
        ));
    }

    #[test]
    fn done_summary_stops_continuation_without_another_round() {
        let (round, results) = round_and_results(
            "done",
            json!({"summary":"finished"}),
            MemoryToolOutcome::Done {
                summary: Some("finished".into()),
            },
        );
        assert_eq!(
            done_summary(&round, &results).expect("done"),
            Some(Some("finished".into()))
        );
    }
}
