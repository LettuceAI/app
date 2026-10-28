use lettuce_conversations::{
    GenerationOperation, InferencePort, InferenceRequest, MessageRole, PortError,
    ProviderContextPart, ProviderNeutralMessage, ProviderReplayArtifactPort, TranscriptToolCall,
    TranscriptToolResult,
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
use lettuce_usage::JobUsageLedger;

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

/// Recursive-loop settings: without recursion a cycle makes one memory
/// request; with it, rounds continue until `done` or the hard cap, which ends
/// the cycle normally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompanionMemoryLoopPolicy {
    pub recursive: bool,
    pub hard_cap: u32,
}

impl CompanionMemoryLoopPolicy {
    #[must_use]
    pub fn from_settings(settings: &lettuce_settings::DynamicMemorySettings) -> Self {
        Self {
            recursive: settings.recursive_memory_loops,
            hard_cap: settings.recursive_memory_loop_hard_cap,
        }
    }

    fn round_cap(self) -> u8 {
        u8::try_from(self.hard_cap.max(1))
            .unwrap_or(u8::MAX)
            .min(lettuce_memory::MAX_DYNAMIC_MEMORY_INFERENCE_ROUNDS)
    }
}

#[derive(Debug)]
pub struct CompanionMemoryContinuationCoordinator<'a, R: ?Sized, I: ?Sized> {
    repository: &'a R,
    inference: &'a I,
}

impl<
    'a,
    R: DynamicMemoryRunRepository
        + ProviderReplayArtifactPort
        + JobUsageLedger
        + crate::generation::runtime_text::RuntimeTextSource
        + ?Sized,
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
        loop_policy: CompanionMemoryLoopPolicy,
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
        let settled_round = rounds.get(usize::from(settled_round_ordinal)).ok_or(
            CompanionMemoryContinuationError::InvalidOwnership {
                reason: "settled round ordinal is not in the stored rounds",
            },
        )?;
        if settled_round.ordinal != settled_round_ordinal {
            return Err(CompanionMemoryContinuationError::InvalidOwnership {
                reason: "stored round ordinal differs from the settled round ordinal",
            });
        }
        let settlement = self
            .repository
            .load_dynamic_memory_round_settlement(run_id, attempt_id, settled_round_ordinal)?
            .ok_or(CompanionMemoryContinuationError::UnsettledRound)?;
        if settlement.run_id != run.id || settlement.attempt_id != attempt.id {
            return Err(CompanionMemoryContinuationError::InvalidSettlement {
                reason: "settlement belongs to another run or attempt",
            });
        }
        if settlement.round_ordinal != settled_round.ordinal {
            return Err(CompanionMemoryContinuationError::InvalidSettlement {
                reason: "settlement round ordinal differs from the settled round",
            });
        }
        if settlement.results.len() != settled_round.calls.len() {
            return Err(CompanionMemoryContinuationError::InvalidSettlement {
                reason: "settlement result count differs from the round call count",
            });
        }
        if settled_round
            .calls
            .iter()
            .zip(&settlement.results)
            .any(|(call, result)| call.id != result.execution_id)
        {
            return Err(CompanionMemoryContinuationError::InvalidSettlement {
                reason: "settlement results are not in the round call order",
            });
        }
        if let Some(summary) = done_summary(settled_round, &settlement.results)? {
            return Ok(CompanionMemoryContinuationResult::Done { summary });
        }
        let next_ordinal = settled_round_ordinal
            .checked_add(1)
            .ok_or(CompanionMemoryContinuationError::RoundLimit)?;
        if let Some(next) = rounds.get(usize::from(next_ordinal)) {
            return Ok(CompanionMemoryContinuationResult::NextRound {
                round: Box::new(next.clone()),
                replayed: true,
            });
        }
        if !loop_policy.recursive {
            return Ok(CompanionMemoryContinuationResult::Done { summary: None });
        }
        if next_ordinal >= loop_policy.round_cap() {
            tracing::warn!(
                run_id = %run.id,
                rounds = next_ordinal,
                hard_cap = loop_policy.hard_cap,
                "recursive memory loop reached its hard cap (bounded by the storage limit)"
            );
            return Ok(CompanionMemoryContinuationResult::Done { summary: None });
        }
        let text = crate::generation::runtime_text::RuntimeText::load(
            self.repository,
            crate::BuiltInPromptId::MemoryRuntime,
        )
        .map_err(|error| CompanionMemoryContinuationError::RuntimeText {
            reason: error.to_string(),
        })?;
        let context = context_after_settlement(&text, settled_round, &settlement.results)?;
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
            prompt_cache_key: None,
        };
        request
            .validate()
            .map_err(|error| CompanionMemoryContinuationError::InvalidRequest {
                reason: format!("continuation request failed validation: {error}"),
            })?;
        let outcome = match run_memory_request_with_fallback(
            self.repository,
            self.inference,
            handle,
            request,
            run.structured_fallback_format,
            crate::companion::companion_memory_inference::MemoryFallbackKind::Operations,
            now,
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
            cleanup_outcome_replays(self.repository, &outcome).map_err(|error| {
                CompanionMemoryContinuationError::ReplayCleanup {
                    reason: error.to_string(),
                }
            })?;
            self.cancel(&attempt, now)?;
            return Err(CompanionMemoryContinuationError::Cancelled);
        }
        let planned = match plan_memory_round(
            &run,
            next_ordinal,
            lettuce_memory::DynamicMemoryRoundKind::Manager,
            context,
            &outcome,
            now,
        ) {
            Ok(round) => round,
            Err(crate::CompanionMemoryInferenceError::Cancelled) => {
                cleanup_outcome_replays(self.repository, &outcome).map_err(|error| {
                    CompanionMemoryContinuationError::ReplayCleanup {
                        reason: error.to_string(),
                    }
                })?;
                self.cancel(&attempt, now)?;
                return Err(CompanionMemoryContinuationError::Cancelled);
            }
            Err(crate::CompanionMemoryInferenceError::NoToolCalls { reason }) => {
                cleanup_outcome_replays(self.repository, &outcome).map_err(|error| {
                    CompanionMemoryContinuationError::ReplayCleanup {
                        reason: error.to_string(),
                    }
                })?;
                tracing::warn!(
                    run_id = %run.id,
                    round = next_ordinal,
                    %reason,
                    "recursive memory round returned no tool calls; ending the cycle"
                );
                return Ok(CompanionMemoryContinuationResult::Done { summary: None });
            }
            Err(error) => {
                cleanup_outcome_replays(self.repository, &outcome).map_err(|error| {
                    CompanionMemoryContinuationError::ReplayCleanup {
                        reason: error.to_string(),
                    }
                })?;
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
                cleanup_outcome_replays(self.repository, &outcome).map_err(|error| {
                    CompanionMemoryContinuationError::ReplayCleanup {
                        reason: error.to_string(),
                    }
                })?;
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
    if attempt.run_id != run.id {
        return Err(CompanionMemoryContinuationError::InvalidOwnership {
            reason: "attempt belongs to another run",
        });
    }
    if attempt.job_id != handle.id() {
        return Err(CompanionMemoryContinuationError::InvalidOwnership {
            reason: "attempt belongs to another job",
        });
    }
    if attempt.status != DynamicMemoryAttemptStatus::Processing {
        return Err(CompanionMemoryContinuationError::InvalidOwnership {
            reason: "attempt is not processing",
        });
    }
    Ok(())
}

fn context_after_settlement(
    text: &crate::generation::runtime_text::RuntimeText,
    round: &DynamicMemoryInferenceRound,
    results: &[lettuce_memory::MemoryToolResult],
) -> Result<lettuce_conversations::ProviderNeutralContext, CompanionMemoryContinuationError> {
    round.request_context.validate().map_err(|error| {
        CompanionMemoryContinuationError::InvalidRequest {
            reason: format!("settled round request context failed validation: {error}"),
        }
    })?;
    let mut calls = Vec::with_capacity(round.calls.len());
    // Unsigned providers replay visible assistant text with its tool calls.
    // Signed providers retain the original content in their replay artifact.
    if round.provider_replay.is_none() {
        for part in &round.parts {
            if let lettuce_conversations::MessagePart::Text { text } = part
                && !text.trim().is_empty()
            {
                calls.push(ProviderContextPart::Text { text: text.clone() });
            }
        }
    }
    let mut outputs = Vec::with_capacity(round.calls.len());
    for (call, result) in round.calls.iter().zip(results) {
        if call.id != result.execution_id {
            return Err(CompanionMemoryContinuationError::InvalidSettlement {
                reason: "tool result does not belong to the round call",
            });
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
            output: crate::memory::memory_tool_result::legacy_memory_tool_output(
                text, &call.call, result,
            )
            .map_err(|error| CompanionMemoryContinuationError::RuntimeText {
                reason: error.to_string(),
            })?,
        }));
    }
    if round.calls.len() != results.len() {
        return Err(CompanionMemoryContinuationError::InvalidSettlement {
            reason: "round has more tool results than calls",
        });
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
        .map_err(|error| CompanionMemoryContinuationError::InvalidRequest {
            reason: format!("context after settlement failed validation: {error}"),
        })?;
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
        match &result.outcome {
            MemoryToolOutcome::Done {
                summary: done_summary,
            } => {
                if summary.replace(done_summary.clone()).is_some() {
                    return Err(CompanionMemoryContinuationError::InvalidSettlement {
                        reason: "round settled more than one done call",
                    });
                }
            }
            MemoryToolOutcome::StoppedAfterDone if summary.is_some() => {}
            _ => {
                return Err(CompanionMemoryContinuationError::InvalidSettlement {
                    reason: "done call settled with an outcome other than done",
                });
            }
        }
    }
    Ok(summary)
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionMemoryContinuationError {
    #[error("background memory continuation ownership is invalid: {reason}")]
    InvalidOwnership { reason: &'static str },
    #[error("background memory continuation request is invalid: {reason}")]
    InvalidRequest { reason: String },
    #[error("background memory round is not settled")]
    UnsettledRound,
    #[error("background memory settlement is invalid: {reason}")]
    InvalidSettlement { reason: &'static str },
    #[error("background memory continuation runtime text is unavailable: {reason}")]
    RuntimeText { reason: String },
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
    #[error("background memory continuation replay cleanup failed: {reason}")]
    ReplayCleanup { reason: String },
    #[error("background memory continuation persistence failed: {0}")]
    Run(#[from] DynamicMemoryRunRepositoryError),
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::{
        ContextAttributions, ContextBudgetReport, ProposedToolCall, ProviderNeutralContext,
    };
    use lettuce_memory::{
        DynamicMemoryRoundFinishReason, DynamicMemoryRoundKind, DynamicMemoryToolCallEvidence,
        MemoryToolResult,
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
                kind: DynamicMemoryRoundKind::Manager,
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
            MemoryToolOutcome::TargetNotFound {
                reference: lettuce_memory::MemoryReference(target.to_string()),
            },
        );
        let text = crate::generation::runtime_text::RuntimeText::from_seed(
            crate::BuiltInPromptId::MemoryRuntime,
        );
        let context = context_after_settlement(&text, &round, &results).expect("context");
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
                    && result.output.value == json!({
                        "status": "skipped",
                        "name": "pin_memory",
                        "reason": "target_not_found",
                        "arguments": {"id": target.to_string()},
                    })
        ));
    }

    #[test]
    fn continuation_preserves_visible_text_with_tool_calls_and_omits_reasoning() {
        let (mut round, results) = round_and_results("done", json!({}), MemoryToolOutcome::Done { summary: None });
        round.parts = vec![
            lettuce_conversations::MessagePart::ReasoningSummary { text: "Internal reasoning.".into() },
            lettuce_conversations::MessagePart::Text { text: "Saving the memories.".into() },
            lettuce_conversations::MessagePart::Text { text: "\n".into() },
        ];
        let text = crate::generation::runtime_text::RuntimeText::from_seed(crate::BuiltInPromptId::MemoryRuntime);
        let context = context_after_settlement(&text, &round, &results).expect("mixed content continuation");
        assert!(matches!(&context.messages[1].parts[..],
            [ProviderContextPart::Text { text }, ProviderContextPart::ToolCall(_)]
                if text == "Saving the memories."));
        assert!(matches!(&context.messages[2].parts[..], [ProviderContextPart::ToolResult(_)]));
    }

    #[test]
    fn loop_policy_caps_rounds_inside_the_storage_bound() {
        let cap = |hard_cap| {
            super::CompanionMemoryLoopPolicy {
                recursive: true,
                hard_cap,
            }
            .round_cap()
        };
        assert_eq!(cap(0), 1);
        assert_eq!(cap(20), 20);
        assert_eq!(cap(100), 100);
        assert_eq!(
            cap(10_000),
            lettuce_memory::MAX_DYNAMIC_MEMORY_INFERENCE_ROUNDS
        );
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
