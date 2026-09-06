use super::*;
use lettuce_conversations::ToolExecution;
use lettuce_conversations::{
    GenerationAttemptStatus, GenerationFailureCode, InitialInferenceBinding,
    InitialInferenceRepository, InitialInferenceResult, ModelSelectionSnapshot, OperationKind,
    UsageCounters, UsageOutcome, UsageUnavailableReason,
};
use lettuce_jobs::{JobErrorCode, JobSnapshot, JobStore, events::JobEvent};
use lettuce_types::{ConversationId, GenerationAttemptId, GenerationTurnId};
use lettuce_usage::{JobInferenceUsageResult, JobUsageLedger, UsageEvent, UsageLedger};

use crate::conversation_generation::{ConversationGenerationOperation, operation_token};
use crate::{
    ConversationGenerationClaimedWork, ConversationGenerationDispatchCoordinator,
    ConversationGenerationDispatchError, ConversationGenerationInput,
    ConversationGenerationJobRunner, ConversationGenerationMemoryInput,
    ConversationGenerationRunError, ConversationGenerationSettledWork, GenerationUsageEvidence,
};

const LEASE: Duration = Duration::from_secs(60);

struct Scenario {
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
    attempt_id: GenerationAttemptId,
    model: ModelSelectionSnapshot,
    profile: lettuce_models::ResolvedChatProfile,
    space_id: Option<MemorySpaceId>,
}

fn scenario(database: &Database, dynamic_memory: bool, prefix: &str) -> Scenario {
    let model_id = seed_model(database, ProviderProtocol::Ollama, "ollama");
    set_application_default_model(database, model_id);
    let character_id = if dynamic_memory {
        seed_character(database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
            defaults.memory_policy = MemoryPolicy::Dynamic;
        })
    } else {
        plain_character(database)
    };
    let launched = ConversationLaunchPlanner::new(database)
        .launch_direct(&request(character_id, &format!("{prefix}-launch")), NOW)
        .expect("launch")
        .value;
    let conversation = launched.conversation;
    let model = conversation
        .participants
        .iter()
        .find(|participant| participant.role == ParticipantRole::Character)
        .and_then(|participant| match &participant.model_selection {
            SnapshotSelection::Inherited(model) | SnapshotSelection::Explicit(model) => {
                Some(model.clone())
            }
            SnapshotSelection::Disabled => None,
        })
        .expect("resolved model snapshot");
    let sent = database
        .begin_send(
            &direct_send_command(&conversation, &format!("{prefix}-send"), "Remember tea."),
            TimestampMillis::new(1_010),
        )
        .expect("send")
        .value;
    let mut stored_profile = ModelProfileRepository::get(database, model.source_id)
        .expect("profile")
        .expect("profile exists");
    stored_profile.config.chat_parameters.temperature = None;
    let account = ProviderAccountRepository::get(database, model.provider_account_id)
        .expect("account")
        .expect("account exists");
    let profile = lettuce_models::resolve_chat_profile(
        &model.expected_chat_identity(),
        &stored_profile,
        &account,
        &lettuce_models::ChatParameterResolutionInput::default(),
        &lettuce_models::ChatRequirements::default(),
    )
    .expect("resolve profile");
    let space_id = dynamic_memory.then(|| {
        MemoryRepository::get_for_conversation(database, conversation.id)
            .expect("memory space")
            .expect("conversation memory")
            .id
    });
    Scenario {
        conversation_id: conversation.id,
        turn_id: sent.turn.id,
        attempt_id: sent.attempt.id,
        model,
        profile,
        space_id,
    }
}

fn input(scenario: &Scenario, tools: bool) -> ConversationGenerationInput {
    ConversationGenerationInput {
        model: scenario.model.clone(),
        attributions: Default::default(),
        profile: ResolvedInferenceProfile {
            chat_profile: scenario.profile.clone(),
            tool_policy: if tools {
                ToolPolicy::Allowed
            } else {
                ToolPolicy::Disabled
            },
            output_policy: OutputPolicy::Plain,
            safety_policy: SafetyContext::Standard,
            correlation_id: None,
        },
        context: lettuce_conversations::ProviderNeutralContext {
            messages: vec![lettuce_conversations::ProviderNeutralMessage {
                role: MessageRole::User,
                parts: vec![ProviderContextPart::Text {
                    text: "Remember tea.".into(),
                }],
            }],
            attributions: Default::default(),
            budget: Default::default(),
        },
        tools: tools.then(dynamic_memory_tool_request),
        media_grants: vec![],
        stream_sink: None,
        memory: tools.then(|| memory_input(scenario.space_id.expect("dynamic memory space"))),
    }
}

fn memory_input(space_id: MemorySpaceId) -> ConversationGenerationMemoryInput {
    ConversationGenerationMemoryInput {
        space_id,
        policy: DynamicMemoryPolicy {
            max_entries: 10,
            hot_token_budget: 100,
            cold_threshold: Score::from_basis_points(2_000).expect("score"),
            delete_confidence_default: Score::from_basis_points(5_000).expect("score"),
            max_hard_delete_ratio_per_cycle: Score::from_basis_points(5_000).expect("score"),
        },
        duplicate_threshold: Score::from_basis_points(9_000).expect("score"),
    }
}

fn usage(input_tokens: u64, output_tokens: u64) -> Option<InferenceUsage> {
    Some(InferenceUsage {
        provider_reported_cost: None,
        cache_write_tokens: None,
        web_search_requests: None,
        cached_input_tokens: None,
        reasoning_tokens: None,
        input_tokens,
        output_tokens,
    })
}

fn text_outcome(id: &str, text: &str, input_tokens: u64, output_tokens: u64) -> InferenceOutcome {
    InferenceOutcome {
        provider_response_id: Some(id.into()),
        candidates: vec![InferenceCandidate {
            ordinal: 0,
            parts: vec![MessagePart::Text { text: text.into() }],
            tool_calls: vec![],
            provider_replay: None,
        }],
        usage: usage(input_tokens, output_tokens),
        finish_reason: lettuce_conversations::FinishReason::Stop,
        provider_finish_reason: Some("stop".into()),
        provider_request_id: Some(format!("{id}-request")),
        warning_codes: vec![],
    }
}

fn call_outcome(
    id: &str,
    name: &str,
    arguments: serde_json::Value,
    tokens: (u64, u64),
) -> InferenceOutcome {
    InferenceOutcome {
        provider_response_id: Some(id.into()),
        candidates: vec![InferenceCandidate {
            ordinal: 0,
            parts: vec![],
            tool_calls: vec![ProposedToolCall {
                provider_call_id: Some(format!("{id}-call")),
                name: name.into(),
                arguments,
                raw_arguments: None,
                provider_replay: None,
            }],
            provider_replay: None,
        }],
        usage: usage(tokens.0, tokens.1),
        finish_reason: lettuce_conversations::FinishReason::Stop,
        provider_finish_reason: None,
        provider_request_id: Some(format!("{id}-request")),
        warning_codes: vec![],
    }
}

fn scripted(outcomes: Vec<InferenceOutcome>) -> ScriptedInference {
    ScriptedInference {
        outcomes: Mutex::new(VecDeque::from(outcomes)),
        requests: Mutex::new(vec![]),
    }
}

fn claim(
    database: &Database,
    scenario: &Scenario,
    attempt_id: GenerationAttemptId,
    now: i64,
) -> ConversationGenerationClaimedWork {
    ConversationGenerationDispatchCoordinator::new(database, database)
        .claim(
            scenario.turn_id,
            attempt_id,
            WorkerId::new(),
            TimestampMillis::new(now),
            LEASE,
            &ResourceAvailability::all(),
        )
        .expect("claim")
        .expect("claimable job")
}

fn admit_and_claim(
    database: &Database,
    scenario: &Scenario,
    now: i64,
) -> ConversationGenerationClaimedWork {
    let admission = ConversationGenerationDispatchCoordinator::new(database, database)
        .admit(
            scenario.conversation_id,
            scenario.turn_id,
            scenario.attempt_id,
            TimestampMillis::new(now),
        )
        .expect("admit");
    assert!(admission.created);
    assert_eq!(admission.attempt.job_id, Some(admission.job.id));
    claim(database, scenario, scenario.attempt_id, now + 1)
}

fn persisted_job(database: &Database, job_id: JobId) -> JobSnapshot {
    JobStore::get(database, job_id)
        .expect("job")
        .expect("job exists")
}

fn job_event_tail(database: &Database, job_id: JobId, count: usize) -> Vec<JobEvent> {
    let events = JobStore::events_since(database, job_id, None, 100).expect("events");
    events
        .iter()
        .rev()
        .take(count)
        .rev()
        .map(|envelope| envelope.event.clone())
        .collect()
}

fn attempt_usage(database: &Database, turn_id: GenerationTurnId, index: usize) -> UsageEvent {
    let turn = ConversationReader::get_turn(database, turn_id).expect("turn");
    let id = turn.attempts[index]
        .usage_event_id
        .expect("settled attempt carries a usage event");
    let event = UsageLedger::get(database, id)
        .expect("usage")
        .expect("usage event exists");
    assert_eq!(event.record.attempt_id, turn.attempts[index].id);
    event
}

#[tokio::test]
async fn plain_chat_runs_finalizes_settles_and_replays_without_redispatch() {
    let database = database();
    let scenario = scenario(&database, false, "plain");
    let work = admit_and_claim(&database, &scenario, 1_015);
    let inference = scripted(vec![text_outcome("plain-1", "Saved context", 20, 5)]);
    let engine = ScenarioEmbeddingEngine;
    let runner = ConversationGenerationJobRunner::new(&engine, &database, &inference);
    let result = runner
        .run(
            &work,
            input(&scenario, false),
            TimestampMillis::new(1_020),
            |_| vec![],
        )
        .await
        .expect("run");
    assert!(!result.replayed);
    assert_eq!(result.rounds, 0);
    assert_eq!(result.outcomes.len(), 1);
    assert_eq!(
        result.candidate.parts,
        vec![MessagePart::Text {
            text: "Saved context".into()
        }]
    );
    assert_eq!(result.turn.status, GenerationTurnStatus::Succeeded);
    assert_eq!(inference.requests.lock().expect("requests").len(), 1);
    let evidence = database.job_usage(work.handle.id()).expect("job usage");
    assert_eq!(evidence.len(), 1);
    assert_eq!(
        evidence[0].result,
        Some(JobInferenceUsageResult::Response {
            usage: usage(20, 5),
            provider_response_id: Some("plain-1".into()),
        })
    );
    let usage_event = UsageLedger::get(&database, result.usage_event_id)
        .expect("usage")
        .expect("usage exists");
    assert_eq!(
        usage_event.record.usage,
        UsageCounters::Known(usage(20, 5).expect("usage"))
    );
    let replay = runner
        .run(
            &work,
            input(&scenario, false),
            TimestampMillis::new(1_030),
            |_| vec![],
        )
        .await
        .expect("replay");
    assert!(replay.replayed);
    assert_eq!(replay.candidate.id, result.candidate.id);
    assert_eq!(replay.usage_event_id, result.usage_event_id);
    assert!(replay.outcomes.is_empty());
    assert_eq!(inference.requests.lock().expect("requests").len(), 1);
    assert_eq!(
        database.job_usage(work.handle.id()).expect("unchanged"),
        evidence
    );
    let dispatcher = ConversationGenerationDispatchCoordinator::new(&database, &database);
    let job_id = work.handle.id();
    let settled = dispatcher
        .settle(
            work,
            Ok(result),
            CancellationReason::User,
            TimestampMillis::new(1_031),
        )
        .expect("settle");
    let ConversationGenerationSettledWork::Succeeded { job, result } = settled else {
        panic!("expected success");
    };
    assert_eq!(job.state, JobState::Succeeded);
    assert_eq!(persisted_job(&database, job_id).state, JobState::Succeeded);
    assert_eq!(
        persisted_job(&database, job_id).outcome,
        Some(JobOutcome::Success {
            result_ref: OutcomeRef::GenerationTurn(scenario.turn_id)
        })
    );
    let tail = job_event_tail(&database, job_id, 2);
    assert!(matches!(tail[0], JobEvent::Progressed { .. }));
    assert!(matches!(tail[1], JobEvent::Succeeded { .. }));
    assert_eq!(result.turn.id, scenario.turn_id);
    assert_eq!(
        attempt_usage(&database, scenario.turn_id, 0).id,
        result.usage_event_id
    );
    let admission = dispatcher
        .admit(
            scenario.conversation_id,
            scenario.turn_id,
            scenario.attempt_id,
            TimestampMillis::new(1_032),
        )
        .expect("admission replay");
    assert!(!admission.created);
    assert_eq!(admission.job.id, job_id);
    assert!(
        dispatcher
            .claim(
                scenario.turn_id,
                scenario.attempt_id,
                WorkerId::new(),
                TimestampMillis::new(1_033),
                LEASE,
                &ResourceAvailability::all(),
            )
            .expect("claim settled job")
            .is_none()
    );
}

#[tokio::test]
async fn two_round_memory_path_runs_through_the_runner_and_replays_once() {
    let database = database();
    let scenario = scenario(&database, true, "rounds");
    let space_id = scenario.space_id.expect("space");
    let memory_id = MemoryId::new();
    let missing_memory = MemoryId::new();
    let work = admit_and_claim(&database, &scenario, 1_015);
    let inference = scripted(vec![
        call_outcome(
            "gen-provider-initial",
            "create_memory",
            serde_json::json!({"text": "Mira prefers tea", "category": "preference"}),
            (20, 5),
        ),
        call_outcome(
            "gen-provider-second",
            "pin_memory",
            serde_json::json!({"id": missing_memory}),
            (7, 2),
        ),
        text_outcome("gen-provider-final", "I will remember that.", 5, 3),
    ]);
    let engine = ScenarioEmbeddingEngine;
    let runner = ConversationGenerationJobRunner::new(&engine, &database, &inference);
    let seeds = |executions: &[ToolExecution]| {
        executions
            .iter()
            .filter(|execution| execution.definition_name == "create_memory")
            .map(|execution| crate::MemoryCreateSeed {
                execution_id: execution.id,
                id: memory_id,
                token_count: 4,
                created_at: TimestampMillis::new(1_016),
            })
            .collect()
    };
    let result = runner
        .run(
            &work,
            input(&scenario, true),
            TimestampMillis::new(1_020),
            seeds,
        )
        .await
        .expect("run rounds");
    assert!(!result.replayed);
    assert_eq!(result.rounds, 2);
    assert_eq!(result.outcomes.len(), 3);
    assert_eq!(
        result.candidate.parts,
        vec![MessagePart::Text {
            text: "I will remember that.".into()
        }]
    );
    let replay_counts = {
        let requests = inference.requests.lock().expect("requests");
        assert_eq!(requests.len(), 3);
        requests
            .iter()
            .map(|request| {
                request
                    .context
                    .messages
                    .iter()
                    .flat_map(|message| &message.parts)
                    .filter(|part| {
                        matches!(
                            part,
                            ProviderContextPart::ToolCall(_) | ProviderContextPart::ToolResult(_)
                        )
                    })
                    .count()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(replay_counts, vec![0, 2, 4]);
    let stored_memory = MemoryRepository::get(&database, space_id)
        .expect("memory")
        .expect("memory exists");
    assert_eq!(stored_memory.revision, Revision::new(2));
    assert_eq!(stored_memory.items.len(), 1);
    assert_eq!(stored_memory.items[0].id, memory_id);
    let evidence = database.job_usage(work.handle.id()).expect("job usage");
    assert_eq!(evidence.len(), 3);
    for (id, input_tokens, output_tokens) in [
        ("gen-provider-initial", 20, 5),
        ("gen-provider-second", 7, 2),
        ("gen-provider-final", 5, 3),
    ] {
        assert!(evidence.iter().any(|event| {
            event.result
                == Some(JobInferenceUsageResult::Response {
                    usage: usage(input_tokens, output_tokens),
                    provider_response_id: Some(id.into()),
                })
        }));
    }
    let usage_event = UsageLedger::get(&database, result.usage_event_id)
        .expect("usage")
        .expect("usage exists");
    assert_eq!(
        usage_event.record.usage,
        UsageCounters::Known(usage(32, 10).expect("usage"))
    );
    let executions = ToolExecutionRepository::list_tool_executions(
        &database,
        scenario.conversation_id,
        scenario.turn_id,
        scenario.attempt_id,
    )
    .expect("executions");
    assert_eq!(executions.len(), 2);
    assert!(
        executions
            .iter()
            .all(|execution| execution.status == ToolExecutionStatus::Succeeded)
    );
    let replay = runner
        .run(
            &work,
            input(&scenario, true),
            TimestampMillis::new(1_030),
            seeds,
        )
        .await
        .expect("replay finalized rounds");
    assert!(replay.replayed);
    assert_eq!(replay.candidate.id, result.candidate.id);
    assert_eq!(replay.usage_event_id, result.usage_event_id);
    assert_eq!(inference.requests.lock().expect("requests").len(), 3);
    assert_eq!(
        database.job_usage(work.handle.id()).expect("unchanged"),
        evidence
    );
    assert_eq!(
        MemoryRepository::get(&database, space_id)
            .expect("memory")
            .expect("memory exists")
            .revision,
        Revision::new(2)
    );
    let job_id = work.handle.id();
    let settled = ConversationGenerationDispatchCoordinator::new(&database, &database)
        .settle(
            work,
            Ok(result),
            CancellationReason::User,
            TimestampMillis::new(1_031),
        )
        .expect("settle");
    assert!(matches!(
        settled,
        ConversationGenerationSettledWork::Succeeded { ref job, .. } if job.state == JobState::Succeeded
    ));
    assert_eq!(persisted_job(&database, job_id).state, JobState::Succeeded);
    assert_eq!(
        ConversationReader::get_turn(&database, scenario.turn_id)
            .expect("turn")
            .status,
        GenerationTurnStatus::Succeeded
    );
}

#[tokio::test]
async fn settled_tool_rounds_are_not_resumed_or_recovered_into_a_child() {
    let database = database();
    let scenario = scenario(&database, true, "unrecoverable");
    let space_id = scenario.space_id.expect("space");
    let memory_id = MemoryId::new();
    let work = admit_and_claim(&database, &scenario, 1_015);
    let inference = FallibleScriptedInference {
        outcomes: Mutex::new(VecDeque::from([
            Ok(call_outcome(
                "round-initial",
                "create_memory",
                serde_json::json!({"text": "Mira prefers tea", "category": "preference"}),
                (20, 5),
            )),
            Err(PortError::Unavailable),
        ])),
        requests: Mutex::new(vec![]),
    };
    let engine = ScenarioEmbeddingEngine;
    let runner = ConversationGenerationJobRunner::new(&engine, &database, &inference);
    let seeds = |executions: &[ToolExecution]| {
        executions
            .iter()
            .map(|execution| crate::MemoryCreateSeed {
                execution_id: execution.id,
                id: memory_id,
                token_count: 4,
                created_at: TimestampMillis::new(1_016),
            })
            .collect()
    };
    let first = runner
        .run(
            &work,
            input(&scenario, true),
            TimestampMillis::new(1_020),
            seeds,
        )
        .await
        .expect_err("continuation provider failed");
    assert!(matches!(
        first,
        ConversationGenerationRunError::Continuation { .. }
    ));
    assert_eq!(inference.requests.lock().expect("requests").len(), 2);
    let stored_memory = MemoryRepository::get(&database, space_id)
        .expect("memory")
        .expect("memory exists");
    assert_eq!(stored_memory.revision, Revision::new(2));
    let dispatch_id = database
        .job_usage(work.handle.id())
        .expect("job usage")
        .into_iter()
        .find(|record| {
            matches!(
                &record.result,
                Some(JobInferenceUsageResult::Response {
                    provider_response_id: Some(id),
                    ..
                }) if id == "round-initial"
            )
        })
        .expect("initial dispatch evidence")
        .id;
    let rerun = runner
        .run(
            &work,
            input(&scenario, true),
            TimestampMillis::new(1_021),
            seeds,
        )
        .await
        .expect_err("settled rounds cannot be resumed");
    let ConversationGenerationRunError::ToolRoundsUnrecoverable { evidence } = rerun else {
        panic!("expected unrecoverable rounds: {rerun:?}");
    };
    assert_eq!(evidence, GenerationUsageEvidence::Dispatch(dispatch_id));
    assert_eq!(inference.requests.lock().expect("requests").len(), 2);
    let job_id = work.handle.id();
    let settled = ConversationGenerationDispatchCoordinator::new(&database, &database)
        .settle(
            work,
            Err(ConversationGenerationRunError::Pending { evidence }),
            CancellationReason::Recovery,
            TimestampMillis::new(1_022),
        )
        .expect("settle");
    let ConversationGenerationSettledWork::Failed { job, .. } = settled else {
        panic!("expected failure instead of a recovery child: {settled:?}");
    };
    assert_eq!(job.state, JobState::Failed);
    assert_eq!(persisted_job(&database, job_id).state, JobState::Failed);
    let turn = ConversationReader::get_turn(&database, scenario.turn_id).expect("turn");
    assert_eq!(turn.status, GenerationTurnStatus::Failed);
    assert_eq!(
        turn.failure,
        Some(GenerationFailureCode::RecoveryUnavailable)
    );
    assert_eq!(turn.attempts.len(), 1);
    let usage_event = attempt_usage(&database, scenario.turn_id, 0);
    assert_eq!(usage_event.record.outcome, UsageOutcome::Failed);
    assert_eq!(
        usage_event.record.usage,
        UsageCounters::Known(usage(20, 5).expect("usage"))
    );
    assert_eq!(
        MemoryRepository::get(&database, space_id)
            .expect("memory")
            .expect("memory exists")
            .revision,
        Revision::new(2)
    );
}

#[tokio::test]
async fn cancellation_before_dispatch_settles_turn_and_job_without_evidence() {
    let database = database();
    let scenario = scenario(&database, false, "cancel-early");
    let work = admit_and_claim(&database, &scenario, 1_015);
    let inference = scripted(vec![text_outcome("unused", "Unused", 1, 1)]);
    let engine = ScenarioEmbeddingEngine;
    let runner = ConversationGenerationJobRunner::new(&engine, &database, &inference);
    work.handle.request_cancel();
    let error = runner
        .run(
            &work,
            input(&scenario, false),
            TimestampMillis::new(1_020),
            |_| vec![],
        )
        .await
        .expect_err("cancelled");
    assert!(matches!(
        error,
        ConversationGenerationRunError::Cancelled {
            evidence: GenerationUsageEvidence::None
        }
    ));
    let job_id = work.handle.id();
    let settled = ConversationGenerationDispatchCoordinator::new(&database, &database)
        .settle(
            work.clone(),
            Err(error),
            CancellationReason::User,
            TimestampMillis::new(1_021),
        )
        .expect("settle");
    let ConversationGenerationSettledWork::Cancelled { job, .. } = settled else {
        panic!("expected cancellation");
    };
    assert_eq!(job.state, JobState::Cancelled);
    assert_eq!(persisted_job(&database, job_id).state, JobState::Cancelled);
    let tail = job_event_tail(&database, job_id, 3);
    assert!(matches!(tail[0], JobEvent::CancellationRequested { .. }));
    assert!(matches!(tail[1], JobEvent::CleanupStarted));
    assert!(matches!(tail[2], JobEvent::Cancelled));
    let turn = ConversationReader::get_turn(&database, scenario.turn_id).expect("turn");
    assert_eq!(turn.status, GenerationTurnStatus::Cancelled);
    assert_eq!(turn.attempts[0].status, GenerationAttemptStatus::Cancelled);
    assert!(inference.requests.lock().expect("requests").is_empty());
    assert!(database.job_usage(job_id).expect("job usage").is_empty());
    let usage_event = attempt_usage(&database, scenario.turn_id, 0);
    assert_eq!(usage_event.record.outcome, UsageOutcome::Cancelled);
    assert_eq!(
        usage_event.record.usage,
        UsageCounters::Unavailable(UsageUnavailableReason::CancelledBeforeResponse)
    );
    let rerun = runner
        .run(
            &work,
            input(&scenario, false),
            TimestampMillis::new(1_030),
            |_| vec![],
        )
        .await
        .expect_err("settled attempt");
    assert!(matches!(
        rerun,
        ConversationGenerationRunError::Cancelled {
            evidence: GenerationUsageEvidence::Event(id)
        } if id == usage_event.id
    ));
    assert!(inference.requests.lock().expect("requests").is_empty());
}

#[tokio::test]
async fn cancellation_during_provider_retains_usage_and_settles_cancelled() {
    let database = database();
    let scenario = scenario(&database, false, "cancel-late");
    let work = admit_and_claim(&database, &scenario, 1_015);
    let inference = BlockingInference::new(text_outcome("blocking", "Late answer", 9, 4));
    let engine = ScenarioEmbeddingEngine;
    let runner = ConversationGenerationJobRunner::new(&engine, &database, &inference);
    let error = {
        let run = runner.run(
            &work,
            input(&scenario, false),
            TimestampMillis::new(1_020),
            |_| vec![],
        );
        tokio::pin!(run);
        tokio::select! {
            _ = inference.entered.notified() => {},
            result = &mut run => panic!("provider must still be pending: {result:?}"),
        }
        work.handle.request_cancel();
        inference.release.notify_one();
        run.await.expect_err("cancelled after response")
    };
    let ConversationGenerationRunError::Cancelled {
        evidence: GenerationUsageEvidence::Dispatch(dispatch_id),
    } = error
    else {
        panic!("expected cancellation with retained usage: {error:?}");
    };
    let evidence = database.job_usage(work.handle.id()).expect("job usage");
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].id, dispatch_id);
    assert!(matches!(
        evidence[0].result,
        Some(JobInferenceUsageResult::Response { .. })
    ));
    let request = InferenceRequest {
        turn_id: scenario.turn_id,
        attempt_id: scenario.attempt_id,
        operation: lettuce_conversations::GenerationOperation::Send,
        profile: input(&scenario, false).profile,
        context: input(&scenario, false).context,
        cancellation: Some(work.handle.id()),
        stream_sink: None,
        media_grants: vec![],
        tools: None,
    };
    let binding =
        InitialInferenceBinding::from_request(scenario.conversation_id, &request).expect("binding");
    assert_eq!(
        database
            .initial_inference(&binding)
            .expect("record")
            .and_then(|record| record.result),
        Some(InitialInferenceResult::Failed(PortError::Cancelled))
    );
    let job_id = work.handle.id();
    let settled = ConversationGenerationDispatchCoordinator::new(&database, &database)
        .settle(
            work.clone(),
            Err(error),
            CancellationReason::User,
            TimestampMillis::new(1_021),
        )
        .expect("settle");
    let ConversationGenerationSettledWork::Cancelled { job, .. } = settled else {
        panic!("expected cancellation");
    };
    assert_eq!(job.state, JobState::Cancelled);
    assert_eq!(persisted_job(&database, job_id).state, JobState::Cancelled);
    let turn = ConversationReader::get_turn(&database, scenario.turn_id).expect("turn");
    assert_eq!(turn.status, GenerationTurnStatus::Cancelled);
    assert_eq!(turn.attempts[0].status, GenerationAttemptStatus::Cancelled);
    let usage_event = attempt_usage(&database, scenario.turn_id, 0);
    assert_ne!(usage_event.id, dispatch_id);
    assert_eq!(usage_event.record.outcome, UsageOutcome::Cancelled);
    assert_eq!(
        usage_event.record.usage,
        UsageCounters::Known(usage(9, 4).expect("usage"))
    );
    assert_eq!(usage_event.record.recorded_at, evidence[0].admitted_at);
    assert!(
        ConversationReader::operation_record(
            &database,
            scenario.conversation_id,
            OperationKind::Cancel,
            &operation_token(
                scenario.conversation_id,
                scenario.turn_id,
                scenario.attempt_id,
                job_id,
                ConversationGenerationOperation::RequestCancellation,
            ),
        )
        .expect("operation record")
        .is_some()
    );
    assert_eq!(inference.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    let rerun = runner
        .run(
            &work,
            input(&scenario, false),
            TimestampMillis::new(1_030),
            |_| vec![],
        )
        .await
        .expect_err("settled attempt");
    assert!(matches!(
        rerun,
        ConversationGenerationRunError::Cancelled {
            evidence: GenerationUsageEvidence::Event(id)
        } if id == usage_event.id
    ));
    assert_eq!(inference.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn provider_failure_fails_turn_and_job_and_replays_without_redispatch() {
    let database = database();
    let scenario = scenario(&database, false, "unavailable");
    let work = admit_and_claim(&database, &scenario, 1_015);
    let inference = FallibleScriptedInference {
        outcomes: Mutex::new(VecDeque::from([Err(PortError::Unavailable)])),
        requests: Mutex::new(vec![]),
    };
    let engine = ScenarioEmbeddingEngine;
    let runner = ConversationGenerationJobRunner::new(&engine, &database, &inference);
    let error = runner
        .run(
            &work,
            input(&scenario, false),
            TimestampMillis::new(1_020),
            |_| vec![],
        )
        .await
        .expect_err("provider failed");
    let ConversationGenerationRunError::Provider {
        error: PortError::Unavailable,
        evidence: GenerationUsageEvidence::Dispatch(dispatch_id),
    } = error
    else {
        panic!("expected provider failure: {error:?}");
    };
    let replay = runner
        .run(
            &work,
            input(&scenario, false),
            TimestampMillis::new(1_030),
            |_| vec![],
        )
        .await
        .expect_err("replayed failure");
    assert!(matches!(
        replay,
        ConversationGenerationRunError::Provider {
            error: PortError::Unavailable,
            evidence: GenerationUsageEvidence::Dispatch(id)
        } if id == dispatch_id
    ));
    assert_eq!(inference.requests.lock().expect("requests").len(), 1);
    let job_id = work.handle.id();
    let settled = ConversationGenerationDispatchCoordinator::new(&database, &database)
        .settle(
            work.clone(),
            Err(error),
            CancellationReason::User,
            TimestampMillis::new(1_031),
        )
        .expect("settle");
    let ConversationGenerationSettledWork::Failed { job, .. } = settled else {
        panic!("expected failure");
    };
    assert_eq!(job.state, JobState::Failed);
    let persisted = persisted_job(&database, job_id);
    assert_eq!(persisted.state, JobState::Failed);
    let job_error = persisted.error.expect("job error");
    assert_eq!(job_error.code, JobErrorCode::ResourceUnavailable);
    assert!(job_error.retryable);
    let turn = ConversationReader::get_turn(&database, scenario.turn_id).expect("turn");
    assert_eq!(turn.status, GenerationTurnStatus::Failed);
    assert_eq!(
        turn.failure,
        Some(GenerationFailureCode::ProviderUnavailable)
    );
    assert_eq!(turn.attempts[0].status, GenerationAttemptStatus::Failed);
    let usage_event = attempt_usage(&database, scenario.turn_id, 0);
    assert_ne!(usage_event.id, dispatch_id);
    assert_eq!(usage_event.record.outcome, UsageOutcome::Failed);
    assert_eq!(
        usage_event.record.usage,
        UsageCounters::Unavailable(UsageUnavailableReason::TransportFailed)
    );
    assert!(
        ConversationReader::operation_record(
            &database,
            scenario.conversation_id,
            OperationKind::Fail,
            &operation_token(
                scenario.conversation_id,
                scenario.turn_id,
                scenario.attempt_id,
                job_id,
                ConversationGenerationOperation::Fail,
            ),
        )
        .expect("operation record")
        .is_some()
    );
    let rerun = runner
        .run(
            &work,
            input(&scenario, false),
            TimestampMillis::new(1_040),
            |_| vec![],
        )
        .await
        .expect_err("already failed");
    assert!(matches!(
        rerun,
        ConversationGenerationRunError::AlreadyFailed {
            code: GenerationFailureCode::ProviderUnavailable,
            evidence: GenerationUsageEvidence::Event(id)
        } if id == usage_event.id
    ));
    assert_eq!(inference.requests.lock().expect("requests").len(), 1);
}

#[tokio::test]
async fn pending_dispatch_interrupts_recovers_and_finishes_in_the_child() {
    let path = std::env::temp_dir().join(format!(
        "lettuce-generation-runner-{}.db",
        ConversationId::new()
    ));
    let database = Database::open(&path).expect("database");
    let scenario = scenario(&database, false, "pending");
    let mut missing_replay = text_outcome("pending-1", "Lost answer", 20, 5);
    missing_replay.candidates[0].provider_replay = Some(lettuce_conversations::ReplayArtifactRef {
        artifact_id: lettuce_types::ReplayArtifactId::new(),
        digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
        schema_version: 1,
        byte_size: 4,
        codec: lettuce_conversations::ReplayCodec::Json,
        retention: lettuce_conversations::ReplayRetention::Conversation,
    });
    let inference = scripted(vec![
        missing_replay,
        text_outcome("pending-2", "Recovered answer", 8, 3),
    ]);
    let engine = ScenarioEmbeddingEngine;
    let work = admit_and_claim(&database, &scenario, 1_015);
    let parent_job_id = work.handle.id();
    let error = ConversationGenerationJobRunner::new(&engine, &database, &inference)
        .run(
            &work,
            input(&scenario, false),
            TimestampMillis::new(1_020),
            |_| vec![],
        )
        .await
        .expect_err("settlement failure");
    assert!(matches!(
        error,
        ConversationGenerationRunError::Repository(_)
    ));
    let settled = ConversationGenerationDispatchCoordinator::new(&database, &database)
        .settle(
            work,
            Err(error),
            CancellationReason::User,
            TimestampMillis::new(1_021),
        )
        .expect("settle");
    let ConversationGenerationSettledWork::RetryScheduled { job, .. } = settled else {
        panic!("expected retry");
    };
    assert_eq!(job.state, JobState::Queued);
    assert_eq!(
        persisted_job(&database, parent_job_id).state,
        JobState::Queued
    );
    let mut request = InferenceRequest {
        turn_id: scenario.turn_id,
        attempt_id: scenario.attempt_id,
        operation: lettuce_conversations::GenerationOperation::Send,
        profile: input(&scenario, false).profile,
        context: input(&scenario, false).context,
        cancellation: Some(parent_job_id),
        stream_sink: None,
        media_grants: vec![],
        tools: None,
    };
    let binding =
        InitialInferenceBinding::from_request(scenario.conversation_id, &request).expect("binding");
    let pending = database
        .initial_inference(&binding)
        .expect("pending")
        .expect("record");
    assert!(pending.result.is_none());
    drop(database);

    let database = Database::open(&path).expect("reopen");
    let dispatcher = ConversationGenerationDispatchCoordinator::new(&database, &database);
    let work = claim(&database, &scenario, scenario.attempt_id, 1_040);
    assert_eq!(work.handle.id(), parent_job_id);
    let error = ConversationGenerationJobRunner::new(&engine, &database, &inference)
        .run(
            &work,
            input(&scenario, false),
            TimestampMillis::new(1_041),
            |_| vec![],
        )
        .await
        .expect_err("pending");
    let ConversationGenerationRunError::Pending {
        evidence: GenerationUsageEvidence::Dispatch(dispatch_id),
    } = error
    else {
        panic!("expected pending dispatch: {error:?}");
    };
    assert_eq!(dispatch_id, pending.usage_event_id);
    assert_eq!(inference.requests.lock().expect("requests").len(), 1);
    let settled = dispatcher
        .settle(
            work.clone(),
            Err(error),
            CancellationReason::Recovery,
            TimestampMillis::new(1_042),
        )
        .expect("settle pending");
    let ConversationGenerationSettledWork::Interrupted {
        job,
        child_attempt_id,
        child_job,
        ..
    } = settled
    else {
        panic!("expected interruption");
    };
    assert_eq!(job.state, JobState::Interrupted);
    assert_ne!(child_job.id, parent_job_id);
    assert_eq!(child_job.state, JobState::Queued);
    let persisted_parent = persisted_job(&database, parent_job_id);
    assert_eq!(persisted_parent.state, JobState::Interrupted);
    assert_eq!(persisted_parent.children.len(), 1);
    assert_eq!(persisted_parent.children[0].child_id, child_job.id);
    let turn = ConversationReader::get_turn(&database, scenario.turn_id).expect("turn");
    assert_eq!(turn.status, GenerationTurnStatus::Recovering);
    assert_eq!(turn.attempts.len(), 2);
    assert_eq!(
        turn.attempts[0].status,
        GenerationAttemptStatus::Interrupted
    );
    assert_eq!(turn.attempts[1].id, child_attempt_id);
    assert_eq!(turn.attempts[1].job_id, Some(child_job.id));
    let parent_usage = attempt_usage(&database, scenario.turn_id, 0);
    assert_ne!(parent_usage.id, dispatch_id);
    assert_eq!(parent_usage.record.outcome, UsageOutcome::Interrupted);
    assert_eq!(
        parent_usage.record.usage,
        UsageCounters::Known(usage(20, 5).expect("usage"))
    );
    assert_eq!(
        database.initial_inference(&binding).expect("parent record"),
        Some(pending)
    );
    let parent_evidence = database.job_usage(parent_job_id).expect("parent usage");
    assert_eq!(parent_evidence.len(), 1);
    assert!(matches!(
        parent_evidence[0].result,
        Some(JobInferenceUsageResult::Response { .. })
    ));
    let rerun = ConversationGenerationJobRunner::new(&engine, &database, &inference)
        .run(
            &work,
            input(&scenario, false),
            TimestampMillis::new(1_043),
            |_| vec![],
        )
        .await
        .expect_err("interrupted parent");
    assert!(matches!(
        rerun,
        ConversationGenerationRunError::Pending {
            evidence: GenerationUsageEvidence::Event(id)
        } if id == parent_usage.id
    ));
    assert_eq!(inference.requests.lock().expect("requests").len(), 1);

    let child_work = claim(&database, &scenario, child_attempt_id, 1_050);
    assert_eq!(child_work.handle.id(), child_job.id);
    let result = ConversationGenerationJobRunner::new(&engine, &database, &inference)
        .run(
            &child_work,
            input(&scenario, false),
            TimestampMillis::new(1_051),
            |_| vec![],
        )
        .await
        .expect("child run");
    assert!(!result.replayed);
    assert_eq!(
        result.candidate.parts,
        vec![MessagePart::Text {
            text: "Recovered answer".into()
        }]
    );
    assert_eq!(result.candidate.attempt_id, child_attempt_id);
    assert_eq!(inference.requests.lock().expect("requests").len(), 2);
    request.attempt_id = child_attempt_id;
    request.cancellation = Some(child_job.id);
    let child_binding =
        InitialInferenceBinding::from_request(scenario.conversation_id, &request).expect("binding");
    assert!(matches!(
        database
            .initial_inference(&child_binding)
            .expect("child record")
            .and_then(|record| record.result),
        Some(InitialInferenceResult::Response(_))
    ));
    let settled = dispatcher
        .settle(
            child_work,
            Ok(result),
            CancellationReason::User,
            TimestampMillis::new(1_052),
        )
        .expect("settle child");
    assert!(matches!(
        settled,
        ConversationGenerationSettledWork::Succeeded { ref job, .. } if job.state == JobState::Succeeded
    ));
    assert_eq!(
        persisted_job(&database, child_job.id).state,
        JobState::Succeeded
    );
    let turn = ConversationReader::get_turn(&database, scenario.turn_id).expect("turn");
    assert_eq!(turn.status, GenerationTurnStatus::Succeeded);
    assert_eq!(turn.attempts[1].status, GenerationAttemptStatus::Succeeded);
    assert_eq!(
        database.job_usage(parent_job_id).expect("parent usage"),
        parent_evidence
    );
    drop(database);
    std::fs::remove_file(path).expect("remove database");
}

#[tokio::test]
async fn invalid_input_settlement_fails_the_job_and_closes_the_turn() {
    let database = database();
    let scenario = scenario(&database, false, "invalid");
    let work = admit_and_claim(&database, &scenario, 1_015);
    let job_id = work.handle.id();
    let settled = ConversationGenerationDispatchCoordinator::new(&database, &database)
        .settle(
            work,
            Err(ConversationGenerationRunError::InvalidInput),
            CancellationReason::User,
            TimestampMillis::new(1_021),
        )
        .expect("settle");
    let ConversationGenerationSettledWork::Failed { job, .. } = settled else {
        panic!("expected failure: {settled:?}");
    };
    assert_eq!(job.state, JobState::Failed);
    let persisted = persisted_job(&database, job_id);
    assert_eq!(persisted.state, JobState::Failed);
    assert_eq!(
        persisted.error.map(|error| error.code),
        Some(JobErrorCode::InvalidInput)
    );
    let turn = ConversationReader::get_turn(&database, scenario.turn_id).expect("turn");
    assert_eq!(turn.status, GenerationTurnStatus::Cancelled);
    assert_eq!(turn.attempts[0].status, GenerationAttemptStatus::Cancelled);
    let usage_event = attempt_usage(&database, scenario.turn_id, 0);
    assert_eq!(usage_event.record.outcome, UsageOutcome::Cancelled);
    assert_eq!(
        usage_event.record.usage,
        UsageCounters::Unavailable(UsageUnavailableReason::CancelledBeforeResponse)
    );
    assert!(database.job_usage(job_id).expect("job usage").is_empty());
}

#[tokio::test]
async fn identity_and_input_guards_reject_before_any_mutation() {
    let database = database_with_builtins();
    let scenario = scenario(&database, false, "guards");
    let dispatcher = ConversationGenerationDispatchCoordinator::new(&database, &database);
    let inference = scripted(vec![text_outcome("unused", "Unused", 1, 1)]);
    let engine = ScenarioEmbeddingEngine;
    let runner = ConversationGenerationJobRunner::new(&engine, &database, &inference);
    let work = admit_and_claim(&database, &scenario, 1_015);
    let mut with_tools = input(&scenario, false);
    with_tools.tools = Some(dynamic_memory_tool_request());
    assert!(matches!(
        runner
            .run(&work, with_tools, TimestampMillis::new(1_020), |_| vec![])
            .await,
        Err(ConversationGenerationRunError::InvalidInput)
    ));
    let mut memory_only = input(&scenario, false);
    memory_only.memory = Some(memory_input(MemorySpaceId::new()));
    assert!(matches!(
        runner
            .run(&work, memory_only, TimestampMillis::new(1_020), |_| vec![])
            .await,
        Err(ConversationGenerationRunError::InvalidInput)
    ));
    let mut foreign_tools = input(&scenario, false);
    foreign_tools.memory = Some(memory_input(MemorySpaceId::new()));
    let mut tools = dynamic_memory_tool_request();
    tools.choice = lettuce_conversations::ToolChoice::Auto;
    foreign_tools.tools = Some(tools);
    assert!(matches!(
        runner
            .run(
                &work,
                foreign_tools,
                TimestampMillis::new(1_020),
                |_| vec![]
            )
            .await,
        Err(ConversationGenerationRunError::InvalidInput)
    ));
    let mut foreign = work.clone();
    foreign.handle = JobHandle::new(JobId::new());
    assert!(matches!(
        runner
            .run(
                &foreign,
                input(&scenario, false),
                TimestampMillis::new(1_020),
                |_| vec![]
            )
            .await,
        Err(ConversationGenerationRunError::InvalidWork)
    ));
    assert!(matches!(
        dispatcher.settle(
            foreign,
            Err(ConversationGenerationRunError::InvalidWork),
            CancellationReason::User,
            TimestampMillis::new(1_021),
        ),
        Err(ConversationGenerationDispatchError::InvalidWork)
    ));
    let turn = ConversationReader::get_turn(&database, scenario.turn_id).expect("turn");
    assert_eq!(turn.status, GenerationTurnStatus::Created);
    assert!(inference.requests.lock().expect("requests").is_empty());
    assert!(
        database
            .job_usage(work.handle.id())
            .expect("usage")
            .is_empty()
    );
    assert!(matches!(
        dispatcher.claim(
            scenario.turn_id,
            GenerationAttemptId::new(),
            WorkerId::new(),
            TimestampMillis::new(1_022),
            LEASE,
            &ResourceAvailability::all(),
        ),
        Err(ConversationGenerationDispatchError::InvalidWork)
    ));

    let group_id = two_member_group(&database);
    let group = ConversationLaunchPlanner::new(&database)
        .launch_group(&group_request(group_id, "guards-group-launch"), NOW)
        .expect("launch group")
        .value
        .conversation;
    let sent = database
        .begin_send(
            &direct_send_command(&group, "guards-group-send", "Hello all."),
            TimestampMillis::new(1_030),
        )
        .expect("group send")
        .value;
    assert!(matches!(
        dispatcher.admit(
            group.id,
            sent.turn.id,
            sent.attempt.id,
            TimestampMillis::new(1_031)
        ),
        Err(ConversationGenerationDispatchError::GroupUnsupported)
    ));
    assert!(
        ConversationReader::get_turn(&database, sent.turn.id)
            .expect("group turn")
            .attempts[0]
            .job_id
            .is_none()
    );
}

#[tokio::test]
async fn derived_operation_tokens_are_stable_and_replay_recorded_operations() {
    let database = database();
    let scenario = scenario(&database, false, "tokens");
    let work = admit_and_claim(&database, &scenario, 1_015);
    let job_id = work.handle.id();
    let token = |operation| {
        operation_token(
            scenario.conversation_id,
            scenario.turn_id,
            scenario.attempt_id,
            job_id,
            operation,
        )
    };
    assert_eq!(
        token(ConversationGenerationOperation::Prepare),
        token(ConversationGenerationOperation::Prepare)
    );
    assert_ne!(
        token(ConversationGenerationOperation::Prepare),
        token(ConversationGenerationOperation::Finalize)
    );
    assert_ne!(
        token(ConversationGenerationOperation::Prepare).request_digest,
        operation_token(
            scenario.conversation_id,
            scenario.turn_id,
            scenario.attempt_id,
            JobId::new(),
            ConversationGenerationOperation::Prepare,
        )
        .request_digest
    );
    let inference = scripted(vec![text_outcome("tokens-1", "Stable", 2, 1)]);
    let engine = ScenarioEmbeddingEngine;
    let runner = ConversationGenerationJobRunner::new(&engine, &database, &inference);
    runner
        .run(
            &work,
            input(&scenario, false),
            TimestampMillis::new(1_020),
            |_| vec![],
        )
        .await
        .expect("run");
    let operations = [
        (
            OperationKind::AttachJob,
            ConversationGenerationOperation::AttachJob,
        ),
        (
            OperationKind::Checkpoint,
            ConversationGenerationOperation::StagePreparing,
        ),
        (
            OperationKind::PrepareGeneration,
            ConversationGenerationOperation::Prepare,
        ),
        (
            OperationKind::Checkpoint,
            ConversationGenerationOperation::StageRunning,
        ),
        (
            OperationKind::Finalize,
            ConversationGenerationOperation::Finalize,
        ),
    ];
    let records = |database: &Database| {
        operations
            .iter()
            .map(|(kind, operation)| {
                let record = ConversationReader::operation_record(
                    database,
                    scenario.conversation_id,
                    *kind,
                    &token(*operation),
                )
                .expect("operation record")
                .expect("recorded operation");
                assert_eq!(record.kind, *kind);
                record
            })
            .collect::<Vec<_>>()
    };
    let first = records(&database);
    runner
        .run(
            &work,
            input(&scenario, false),
            TimestampMillis::new(1_030),
            |_| vec![],
        )
        .await
        .expect("replay");
    assert_eq!(records(&database), first);
    assert_eq!(inference.requests.lock().expect("requests").len(), 1);
}
