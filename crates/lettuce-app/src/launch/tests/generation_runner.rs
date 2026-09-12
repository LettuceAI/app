use super::*;
use lettuce_conversations::{
    BeginGeneration, GenerationAttemptStatus, GenerationFailureCode, InitialInferenceBinding,
    InitialInferenceRepository, InitialInferenceResult, ModelSelectionSnapshot, OperationKind,
    ResolveGroupSpeaker, SelectedSpeakerDecision, SpeakerDecisionMethod, SpeakerFallback,
    SpeakerInferenceBinding, SpeakerInferenceRepository, UsageCounters, UsageOutcome,
    UsageUnavailableReason,
};
use lettuce_embeddings::{
    EmbeddingDimensions, MemoryEmbeddingProjection, MemoryEmbeddingRepository,
};
use lettuce_inference::InferenceRuntimePort;
use lettuce_jobs::{
    CancellationReason, Clock, FakeClock, JobErrorCode, JobSnapshot, JobState, JobStore,
    ResourceAvailability, WorkerId, events::JobEvent, handle::CancellationToken,
};
use lettuce_memory::{
    MemoryRepositoryError, MemoryRetrievalAccess, MemoryRetrievalAccessReceipt,
    MemoryRetrievalRepository,
};
use lettuce_types::{ConversationId, GenerationAttemptId, GenerationTurnId, JobId};
use lettuce_usage::{JobInferenceUsageResult, JobUsageLedger, UsageEvent, UsageLedger};

use crate::conversation_generation::{ConversationGenerationOperation, operation_token};
use crate::{
    ConversationGenerationCancellationOutcome, ConversationGenerationClaimedWork,
    ConversationGenerationDispatchCoordinator, ConversationGenerationDispatchError,
    ConversationGenerationExecutionOutcome, ConversationGenerationExecutionRequest,
    ConversationGenerationInput, ConversationGenerationJobRunner, ConversationGenerationRunError,
    ConversationGenerationRuntimeInput, ConversationGenerationSettledWork,
    ConversationGenerationWorkerOutcome, ConversationGenerationWorkerRequest,
    GenerationUsageEvidence, PreparedConversationGenerationJobRunner,
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
    scenario_with_resolvable_profile(database, dynamic_memory, prefix, false)
}

fn scenario_with_resolvable_profile(
    database: &Database,
    dynamic_memory: bool,
    prefix: &str,
    persist_resolvable_profile: bool,
) -> Scenario {
    let model_id = seed_model(database, ProviderProtocol::Ollama, "ollama");
    if persist_resolvable_profile {
        let mut model = ModelProfileRepository::get(database, model_id)
            .expect("model")
            .expect("model exists");
        let revision = model.revision;
        model.config.chat_parameters.temperature = None;
        model.config.capabilities.streaming = lettuce_models::CapabilityStatus::Supported;
        if dynamic_memory {
            model.config.capabilities.tools = lettuce_models::CapabilityStatus::Supported;
        }
        ModelProfileRepository::upsert(database, model, Some(revision))
            .expect("resolvable model profile");
    }
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

fn input(scenario: &Scenario) -> ConversationGenerationInput {
    ConversationGenerationInput {
        model: scenario.model.clone(),
        attributions: Default::default(),
        profile: ResolvedInferenceProfile {
            chat_profile: scenario.profile.clone(),
            tool_policy: ToolPolicy::Disabled,
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
        media_grants: vec![],
        stream_sink: None,
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

fn execution_request(
    scenario: &Scenario,
    cancellation: CancellationToken,
) -> ConversationGenerationExecutionRequest {
    ConversationGenerationExecutionRequest {
        conversation_id: scenario.conversation_id,
        turn_id: scenario.turn_id,
        attempt_id: scenario.attempt_id,
        worker_id: WorkerId::new(),
        lease_for: LEASE,
        resources: ResourceAvailability::all(),
        runtime: ConversationGenerationRuntimeInput::default(),
        cancellation,
        cancellation_reason: CancellationReason::User,
    }
}

fn group_scenario(
    backend: &AppBackend,
    prefix: &str,
    speaker_selection: lettuce_characters::SpeakerSelection,
    mute_second: bool,
) -> (Scenario, Vec<lettuce_types::ConversationParticipantId>) {
    group_scenario_with_message(
        backend,
        prefix,
        speaker_selection,
        mute_second,
        "Hello cast.",
    )
}

fn group_scenario_with_message(
    backend: &AppBackend,
    prefix: &str,
    speaker_selection: lettuce_characters::SpeakerSelection,
    mute_second: bool,
    message: &str,
) -> (Scenario, Vec<lettuce_types::ConversationParticipantId>) {
    let database = backend.database();
    let model_id = seed_model(database, ProviderProtocol::Ollama, "ollama");
    set_application_default_model(database, model_id);
    let mut stored_model = ModelProfileRepository::get(database, model_id)
        .expect("model")
        .expect("model exists");
    let model_revision = stored_model.revision;
    stored_model.config.chat_parameters.temperature = None;
    stored_model.config.capabilities.streaming = lettuce_models::CapabilityStatus::Supported;
    stored_model.config.capabilities.tools = lettuce_models::CapabilityStatus::Supported;
    ModelProfileRepository::upsert(database, stored_model, Some(model_revision))
        .expect("resolvable model profile");
    if speaker_selection == lettuce_characters::SpeakerSelection::Llm {
        let selection_model_id = seed_model(database, ProviderProtocol::Ollama, "speaker");
        let mut selection_model = ModelProfileRepository::get(database, selection_model_id)
            .expect("selection model")
            .expect("selection model exists");
        let selection_revision = selection_model.revision;
        selection_model.config.chat_parameters.temperature = None;
        selection_model.config.capabilities.tools = lettuce_models::CapabilityStatus::Supported;
        ModelProfileRepository::upsert(database, selection_model, Some(selection_revision))
            .expect("resolvable selection model");
        let settings = GlobalSettingsStore::load(database).expect("settings");
        GlobalSettingsStore::set_group_speaker_model_profile(
            database,
            Some(selection_model_id),
            settings.revision,
        )
        .expect("select group speaker model");
    }
    let first = seed_named_character_with(database, "Ada", |defaults| {
        defaults.model_profile_id = Some(model_id);
    });
    let second = seed_named_character_with(database, "Bea", |defaults| {
        defaults.model_profile_id = Some(model_id);
    });
    let mut muted = member(second, 1);
    muted.muted = mute_second;
    let group_id = seed_group(database, vec![member(first, 0), muted], None, |group| {
        group.speaker_selection = speaker_selection;
    });
    let launched = ConversationLaunchPlanner::new(database)
        .launch_group(&group_request(group_id, &format!("{prefix}-launch")), NOW)
        .expect("launch group")
        .value;
    let conversation = launched.conversation;
    let speakers = conversation
        .participants
        .iter()
        .filter(|participant| participant.role == ParticipantRole::Character)
        .map(|participant| participant.id)
        .collect::<Vec<_>>();
    let model = conversation
        .participants
        .iter()
        .find(|participant| participant.id == speakers[0])
        .and_then(|participant| match &participant.model_selection {
            SnapshotSelection::Inherited(model) | SnapshotSelection::Explicit(model) => {
                Some(model.clone())
            }
            SnapshotSelection::Disabled => None,
        })
        .expect("group model snapshot");
    let stored_profile = ModelProfileRepository::get(database, model.source_id)
        .expect("profile")
        .expect("profile exists");
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
    .expect("resolve group profile");
    let sent = database
        .begin_send(
            &direct_send_command(&conversation, &format!("{prefix}-send"), message),
            TimestampMillis::new(1_010),
        )
        .expect("begin group send")
        .value;
    (
        Scenario {
            conversation_id: conversation.id,
            turn_id: sent.turn.id,
            attempt_id: sent.attempt.id,
            model,
            profile,
            space_id: None,
        },
        speakers,
    )
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
    let runner = ConversationGenerationJobRunner::new(&database, &inference);
    let result = runner
        .run(&work, input(&scenario), TimestampMillis::new(1_020))
        .await
        .expect("run");
    assert!(!result.replayed);
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
        .run(&work, input(&scenario), TimestampMillis::new(1_030))
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
async fn app_backend_executes_and_settles_one_durable_generation_operation() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let scenario = scenario_with_resolvable_profile(backend.database(), false, "scheduled", true);
    let inference = scripted(vec![text_outcome(
        "scheduled-response",
        "Scheduled reply.",
        12,
        3,
    )]);
    let engine = ScenarioEmbeddingEngine;
    let clock = FakeClock::new(TimestampMillis::new(1_020));
    let outcome = backend
        .prepared_conversation_generation_runner(&engine, &inference)
        .execute(
            execution_request(&scenario, CancellationToken::new()),
            &clock,
        )
        .await
        .expect("execute generation");
    let ConversationGenerationExecutionOutcome::Settled(
        ConversationGenerationSettledWork::Succeeded { job, result },
    ) = outcome
    else {
        panic!("generation succeeds");
    };
    assert_eq!(job.state, JobState::Succeeded);
    assert_eq!(result.turn.status, GenerationTurnStatus::Succeeded);
    assert_eq!(inference.requests.lock().expect("requests").len(), 1);

    let replay = backend
        .prepared_conversation_generation_runner(&engine, &inference)
        .execute(
            execution_request(&scenario, CancellationToken::new()),
            &clock,
        )
        .await
        .expect("replay execution");
    let ConversationGenerationExecutionOutcome::Replayed {
        result: replayed,
        job: replayed_job,
    } = replay
    else {
        panic!("settled generation replays");
    };
    assert!(replayed.replayed);
    assert_eq!(replayed.candidate.id, result.candidate.id);
    assert_eq!(replayed.usage_event_id, result.usage_event_id);
    assert_eq!(replayed_job.state, JobState::Succeeded);
    assert_eq!(inference.requests.lock().expect("requests").len(), 1);

    let cancelled =
        scenario_with_resolvable_profile(backend.database(), false, "scheduled-cancel", true);
    let cancelled_inference = scripted(vec![]);
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let outcome = backend
        .prepared_conversation_generation_runner(&engine, &cancelled_inference)
        .execute(execution_request(&cancelled, cancellation), &clock)
        .await
        .expect("settle cancelled generation");
    assert!(matches!(
        outcome,
        ConversationGenerationExecutionOutcome::Settled(
            ConversationGenerationSettledWork::Cancelled { ref job, .. }
        ) if job.state == JobState::Cancelled
    ));
    assert!(
        cancelled_inference
            .requests
            .lock()
            .expect("requests")
            .is_empty()
    );
    let terminal = backend
        .prepared_conversation_generation_runner(&engine, &cancelled_inference)
        .execute(
            execution_request(&cancelled, CancellationToken::new()),
            &clock,
        )
        .await
        .expect("read terminal generation");
    assert!(matches!(
        terminal,
        ConversationGenerationExecutionOutcome::Terminal(ref admission)
            if admission.job.state == JobState::Cancelled
                && admission.attempt.status == GenerationAttemptStatus::Cancelled
    ));

    let blocked =
        scenario_with_resolvable_profile(backend.database(), false, "scheduled-blocked", true);
    let blocked_inference = scripted(vec![]);
    let mut request = execution_request(&blocked, CancellationToken::new());
    request.resources = ResourceAvailability::none();
    let outcome = backend
        .prepared_conversation_generation_runner(&engine, &blocked_inference)
        .execute(request, &clock)
        .await
        .expect("leave unavailable generation queued");
    assert!(matches!(
        outcome,
        ConversationGenerationExecutionOutcome::NotClaimed(ref admission)
            if admission.created
                && admission.job.state == JobState::Queued
                && admission.attempt.job_id == Some(admission.job.id)
    ));
    assert!(
        blocked_inference
            .requests
            .lock()
            .expect("requests")
            .is_empty()
    );
}

#[tokio::test]
async fn app_backend_cancels_queued_and_running_generation_jobs_by_id() {
    let path = std::env::temp_dir().join(format!(
        "lettuce-generation-cancel-{}.db",
        ConversationId::new()
    ));
    let backend = AppBackend::open(&path, TimestampMillis::new(1)).expect("backend");
    let engine = ScenarioEmbeddingEngine;
    let clock = FakeClock::new(TimestampMillis::new(1_020));

    let queued = scenario_with_resolvable_profile(backend.database(), false, "queued-cancel", true);
    let queued_job = backend
        .conversation_generation_dispatcher()
        .admit(
            queued.conversation_id,
            queued.turn_id,
            queued.attempt_id,
            clock.now(),
        )
        .expect("admit queued generation")
        .job;
    drop(backend);
    let backend = AppBackend::open(&path, TimestampMillis::new(1_021)).expect("reopen backend");
    let cancelled = backend
        .conversation_generation_cancellation()
        .cancel(
            queued_job.id,
            CancellationReason::User,
            TimestampMillis::new(1_021),
        )
        .expect("cancel queued generation");
    assert!(matches!(
        cancelled,
        ConversationGenerationCancellationOutcome::QueuedCancelled(ref job)
            if job.state == JobState::Cancelled
    ));
    let queued_turn = ConversationReader::get_turn(backend.database(), queued.turn_id)
        .expect("cancelled queued turn");
    assert_eq!(queued_turn.status, GenerationTurnStatus::Cancelled);
    assert_eq!(
        queued_turn.attempts[0].status,
        GenerationAttemptStatus::Cancelled
    );
    assert!(matches!(
        attempt_usage(backend.database(), queued.turn_id, 0)
            .record
            .usage,
        UsageCounters::Unavailable(UsageUnavailableReason::CancelledBeforeResponse)
    ));
    assert!(matches!(
        backend
            .conversation_generation_cancellation()
            .cancel(
                queued_job.id,
                CancellationReason::User,
                TimestampMillis::new(1_022),
            )
            .expect("repeat queued cancellation"),
        ConversationGenerationCancellationOutcome::AlreadyTerminal(ref job)
            if job.state == JobState::Cancelled
    ));
    assert_eq!(
        backend
            .conversation_generation_cancellation()
            .cancel(
                JobId::new(),
                CancellationReason::User,
                TimestampMillis::new(1_022),
            )
            .expect("unknown cancellation"),
        ConversationGenerationCancellationOutcome::NotFound
    );

    let running =
        scenario_with_resolvable_profile(backend.database(), false, "running-cancel", true);
    let provider_runtime = backend
        .provider_runtime(
            std::sync::Arc::new(lettuce_settings::InMemorySecretStore::new()),
            &lettuce_network::TlsPolicy::default(),
        )
        .expect("shared provider runtime");
    let inference_runtime = provider_runtime.inference_runtime();
    let running_job = backend
        .conversation_generation_dispatcher()
        .admit(
            running.conversation_id,
            running.turn_id,
            running.attempt_id,
            TimestampMillis::new(1_030),
        )
        .expect("admit running generation")
        .job;
    let inference = BlockingInference::new(text_outcome("late", "Late reply", 9, 4));
    let runner = backend.prepared_conversation_generation_runner(&engine, &inference);
    let run = runner.execute(
        execution_request(&running, CancellationToken::new()),
        &clock,
    );
    let cancel = async {
        inference.entered.notified().await;
        clock.set(TimestampMillis::new(1_040));
        let outcome = backend
            .conversation_generation_cancellation()
            .cancel(running_job.id, CancellationReason::User, clock.now())
            .expect("cancel running generation");
        assert!(inference_runtime.is_cancelled(running_job.id));
        inference.release.notify_one();
        outcome
    };
    let (settled, requested) = tokio::join!(run, cancel);
    assert!(matches!(
        requested,
        ConversationGenerationCancellationOutcome::Requested {
            ref job,
            live_execution_signalled: true,
        } if job.state == JobState::CancellationRequested
    ));
    assert!(matches!(
        settled.expect("settle running cancellation"),
        ConversationGenerationExecutionOutcome::Settled(
            ConversationGenerationSettledWork::Cancelled { ref job, .. }
        ) if job.state == JobState::Cancelled
    ));
    assert_eq!(inference.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(!inference_runtime.is_cancelled(running_job.id));
    assert_eq!(
        ConversationReader::get_turn(backend.database(), running.turn_id)
            .expect("cancelled running turn")
            .status,
        GenerationTurnStatus::Cancelled
    );
}

#[tokio::test]
async fn app_backend_worker_executes_one_durable_generation_job() {
    let path = std::env::temp_dir().join(format!(
        "lettuce-generation-worker-{}.db",
        ConversationId::new()
    ));
    let backend = AppBackend::open(&path, TimestampMillis::new(1)).expect("backend");
    let scenario = scenario_with_resolvable_profile(backend.database(), false, "worker", true);
    let turn =
        ConversationReader::get_turn(backend.database(), scenario.turn_id).expect("scheduled turn");
    let generation = BeginGeneration {
        conversation: ConversationReader::get(backend.database(), scenario.conversation_id)
            .expect("scheduled conversation")
            .conversation,
        attempt: turn.attempts[0].clone(),
        turn,
    };
    let job = backend
        .conversation_generation_dispatcher()
        .schedule(&generation, TimestampMillis::new(1_020))
        .expect("schedule generation")
        .job;
    drop(backend);

    let backend = AppBackend::open(&path, TimestampMillis::new(1_021)).expect("reopen backend");
    let inference = scripted(vec![text_outcome(
        "worker-response",
        "Worker reply.",
        12,
        3,
    )]);
    let engine = ScenarioEmbeddingEngine;
    let clock = FakeClock::new(TimestampMillis::new(1_022));
    let request = ConversationGenerationWorkerRequest {
        worker_id: WorkerId::new(),
        lease_for: LEASE,
        resources: ResourceAvailability::all(),
    };
    let outcome = backend
        .prepared_conversation_generation_runner(&engine, &inference)
        .execute_next(request.clone(), &clock)
        .await
        .expect("execute scheduled generation");
    let ConversationGenerationWorkerOutcome::Executed(outcome) = outcome else {
        panic!("worker executes one job");
    };
    assert!(matches!(
        *outcome,
        ConversationGenerationExecutionOutcome::Settled(
            ConversationGenerationSettledWork::Succeeded { ref job, .. }
        ) if job.state == JobState::Succeeded
    ));
    assert_eq!(
        persisted_job(backend.database(), job.id).state,
        JobState::Succeeded
    );
    assert_eq!(inference.requests.lock().expect("requests").len(), 1);
    assert!(matches!(
        backend
            .prepared_conversation_generation_runner(&engine, &inference)
            .execute_next(request, &clock)
            .await
            .expect("empty worker pass"),
        ConversationGenerationWorkerOutcome::Idle
    ));
}

#[tokio::test]
async fn app_backend_builds_manual_inputs_for_send_continue_and_regenerate() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let scenario =
        scenario_with_resolvable_profile(backend.database(), false, "prepared-input", true);
    let manual_space =
        MemoryRepository::get_for_conversation(backend.database(), scenario.conversation_id)
            .expect("manual memory space")
            .expect("manual memory exists");
    let manual_item = MemoryItem {
        id: MemoryId::new(),
        short_id: lettuce_memory::MemoryShortId::new(900001).expect("short id"),
        text: "Mira keeps a handwritten tea journal.".into(),
        category: MemoryCategory::Preference,
        source_message_id: None,
        source_role: None,
        observed_at: None,
        observed_time_precision: None,
        superseded_by: None,
        superseded_at: None,
        supersedes: vec![],
        token_count: 6,
        is_cold: false,
        is_pinned: false,
        importance: Score::FULL,
        persistence_importance: Score::FULL,
        prompt_importance: Score::FULL,
        volatility: Score::LEGACY_VOLATILITY,
        access_count: 0,
        created_at: TimestampMillis::new(1_012),
        last_accessed_at: TimestampMillis::new(1_012),
    };
    let manual_space = MemoryRepository::compare_and_apply(
        backend.database(),
        MemoryChangeSet {
            space_id: manual_space.id,
            expected_revision: manual_space.revision,
            items: vec![manual_item.clone()],
        },
    )
    .expect("store manual memory");
    let work = admit_and_claim(backend.database(), &scenario, 1_015);
    let inference = scripted(vec![text_outcome(
        "prepared-input-response",
        "Prepared reply",
        13,
        4,
    )]);
    let engine = ScenarioEmbeddingEngine;
    let runner = backend.prepared_conversation_generation_runner(&engine, &inference);
    let stream_sink = RequestId::new();
    let runtime = ConversationGenerationRuntimeInput {
        stream_sink: Some(stream_sink),
        ..Default::default()
    };
    let result = runner
        .run(&work, runtime.clone(), TimestampMillis::new(1_020))
        .await
        .expect("prepared direct send");
    assert!(!result.replayed);
    {
        let requests = inference.requests.lock().expect("requests");
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].profile.chat_profile.model_profile_id,
            scenario.model.source_id
        );
        assert_eq!(requests[0].stream_sink, Some(stream_sink));
        assert_eq!(requests[0].tools, None);
        assert!(requests[0].context.messages.iter().any(|message| {
            message.parts.iter().any(
                |part| matches!(part, ProviderContextPart::Text { text } if text == "Remember tea."),
            )
        }));
        assert!(requests[0].context.messages.iter().any(|message| {
            message.parts.iter().any(|part| {
                matches!(part, ProviderContextPart::Text { text } if text.contains("- Mira keeps a handwritten tea journal."))
            })
        }));
    }
    let prepared_turn =
        ConversationReader::get_turn(backend.database(), scenario.turn_id).expect("prepared turn");
    assert_eq!(
        prepared_turn.memory,
        Some(lettuce_conversations::MemoryAttribution {
            revision_id: lettuce_memory::memory_revision_id(manual_space.id, manual_space.revision,),
        })
    );
    assert_eq!(
        MemoryRepository::get(backend.database(), manual_space.id)
            .expect("manual memory after generation")
            .expect("manual memory remains"),
        manual_space
    );

    let operation = |name: &str| OperationToken {
        key: key(name),
        request_digest: ContentHash::parse("cd".repeat(32)).expect("operation digest"),
    };
    let current = ConversationReader::get(backend.database(), scenario.conversation_id)
        .expect("conversation after send")
        .conversation;
    let continued = backend
        .database()
        .begin_continue(
            &ContinueConversation {
                conversation_id: current.id,
                branch_id: current.active_branch_id,
                expected_revision: current.revision,
                forced_speaker: None,
                swap_roles: false,
                operation: operation("prepared-manual-continue"),
            },
            TimestampMillis::new(1_025),
        )
        .expect("begin manual continuation")
        .value;
    let continued_scenario = Scenario {
        conversation_id: scenario.conversation_id,
        turn_id: continued.turn.id,
        attempt_id: continued.attempt.id,
        model: scenario.model.clone(),
        profile: scenario.profile.clone(),
        space_id: None,
    };
    let continued_work = admit_and_claim(backend.database(), &continued_scenario, 1_026);
    let stored_settings = GlobalSettingsStore::load(backend.database()).expect("settings");
    let mut narrow_window = stored_settings.settings;
    narrow_window.manual_mode_context_window = 1;
    GlobalSettingsStore::save(
        backend.database(),
        narrow_window,
        stored_settings.default_model_profile_id,
        stored_settings.revision,
    )
    .expect("narrow the manual context window");
    let continued_inference = scripted(vec![text_outcome(
        "prepared-manual-continue-response",
        "Continued reply",
        8,
        3,
    )]);
    let continued_result = backend
        .prepared_conversation_generation_runner(&engine, &continued_inference)
        .run(
            &continued_work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_027),
        )
        .await
        .expect("run manual continuation");
    {
        let requests = continued_inference
            .requests
            .lock()
            .expect("continuation requests");
        let texts = requests[0]
            .context
            .messages
            .iter()
            .flat_map(|message| &message.parts)
            .filter_map(|part| match part {
                ProviderContextPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(
            texts
                .iter()
                .any(|text| text.contains("- Mira keeps a handwritten tea journal."))
        );
        assert!(texts.contains(&"Prepared reply"));
        assert!(!texts.contains(&"Remember tea."));
        assert!(texts.last().is_some_and(|text| {
            text.starts_with("[CONTINUE] You were in the middle of a response.")
        }));
    }

    let current = ConversationReader::get(backend.database(), scenario.conversation_id)
        .expect("conversation after continuation")
        .conversation;
    let regenerated = backend
        .database()
        .begin_regenerate(
            &lettuce_conversations::RegenerateCandidate {
                conversation_id: current.id,
                branch_id: current.active_branch_id,
                message_id: continued_result.candidate.message_id,
                turn_id: continued_result.turn.id,
                expected_revision: current.revision,
                expected_turn_revision: continued_result.turn.revision,
                operation: operation("prepared-manual-regenerate"),
                active_candidate_id: continued_result.candidate.id,
                guidance: Some("Make it shorter.".into()),
                model_override: None,
                forced_speaker: None,
                swap_roles: false,
            },
            TimestampMillis::new(1_028),
        )
        .expect("begin manual regeneration")
        .value;
    let regenerated_scenario = Scenario {
        conversation_id: scenario.conversation_id,
        turn_id: regenerated.turn.id,
        attempt_id: regenerated.attempt.id,
        model: scenario.model.clone(),
        profile: scenario.profile.clone(),
        space_id: None,
    };
    let regenerated_work = admit_and_claim(backend.database(), &regenerated_scenario, 1_029);
    let regenerated_inference = scripted(vec![text_outcome(
        "prepared-manual-regenerate-response",
        "Regenerated reply",
        9,
        3,
    )]);
    let regenerated_result = backend
        .prepared_conversation_generation_runner(&engine, &regenerated_inference)
        .run(
            &regenerated_work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_030),
        )
        .await
        .expect("run manual regeneration");
    assert_eq!(regenerated_result.candidate.ordinal, 1);
    {
        let requests = regenerated_inference
            .requests
            .lock()
            .expect("regeneration requests");
        let messages = &requests[0].context.messages;
        assert!(messages.iter().any(|message| message.parts.iter().any(|part| {
            matches!(part, ProviderContextPart::Text { text } if text.contains("- Mira keeps a handwritten tea journal."))
        })));
        let last = messages.last().expect("regeneration instruction");
        assert_eq!(last.role, MessageRole::User);
        assert!(matches!(
            last.parts.as_slice(),
            [ProviderContextPart::Text { text }]
                if text.starts_with("[REGENERATE INSTRUCTION]") && text.ends_with("\nMake it shorter.")
        ));
    }

    let replay = runner
        .run(
            &work,
            ConversationGenerationRuntimeInput {
                stream_sink: Some(RequestId::new()),
                ..runtime
            },
            TimestampMillis::new(1_031),
        )
        .await
        .expect("prepared replay");
    assert!(replay.replayed);
    assert_eq!(replay.candidate.id, result.candidate.id);
    assert_eq!(inference.requests.lock().expect("requests").len(), 1);
    assert_eq!(
        MemoryRepository::get(backend.database(), manual_space.id)
            .expect("manual memory after replay")
            .expect("manual memory remains after replay"),
        manual_space
    );
}

#[tokio::test]
async fn a_group_switched_to_llm_selection_uses_the_live_speaker_model() {
    use lettuce_conversations::ConversationRepository as _;
    let path = std::env::temp_dir().join(format!("lettuce-group-switch-{}.db", RequestId::new()));
    let backend = AppBackend::open(&path, TimestampMillis::new(1)).expect("backend");
    let (scenario, speakers) = group_scenario(
        &backend,
        "switch-llm",
        lettuce_characters::SpeakerSelection::RoundRobin,
        false,
    );
    backend
        .database()
        .update_settings(
            lettuce_conversations::PreparedConversationSettingsUpdate::new(
                lettuce_conversations::UpdateConversationSettings {
                    conversation_id: scenario.conversation_id,
                    expected_settings_revision: None,
                    operation: lettuce_conversations::OperationToken {
                        key: lettuce_jobs::IdempotencyKey::new("switch-llm-settings").expect("key"),
                        request_digest: lettuce_types::ContentHash::parse("ab".repeat(32))
                            .expect("digest"),
                    },
                    patch: lettuce_conversations::CurrentConversationSettingsPatch {
                        speaker_selection: lettuce_conversations::PatchValue::Set(
                            lettuce_conversations::GroupSpeakerSelectionSnapshot::Llm,
                        ),
                        ..lettuce_conversations::CurrentConversationSettingsPatch::default()
                    },
                },
                Vec::new(),
            )
            .expect("prepared settings"),
            TimestampMillis::new(1_012),
        )
        .expect("switch to LLM selection");
    let work = admit_and_claim(backend.database(), &scenario, 1_015);
    let inference = scripted(vec![
        call_outcome(
            "switch-llm-selection",
            "select_next_speaker",
            serde_json::json!({ "character_id": speakers[1] }),
            (15, 2),
        ),
        text_outcome("switch-llm-generation", "Group reply.", 20, 4),
    ]);
    let engine = ScenarioEmbeddingEngine;
    let result = backend
        .prepared_conversation_generation_runner(&engine, &inference)
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("run switched group generation");
    let decision = result.turn.selected_speaker.expect("speaker decision");
    assert_eq!(decision.participant_id, speakers[1]);
    assert_eq!(decision.method, SpeakerDecisionMethod::Llm);
    assert_eq!(decision.fallback, SpeakerFallback::None);
    assert_eq!(decision.decision_model, None);
    let requests = inference.requests.lock().expect("requests");
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0]
            .tools
            .as_ref()
            .expect("selection tools")
            .definitions[0]
            .name,
        "select_next_speaker"
    );
    assert_eq!(
        requests[0].profile.chat_profile.model_profile_id,
        GlobalSettingsStore::load(backend.database())
            .expect("settings")
            .default_model_profile_id
            .expect("default model")
    );
}

#[tokio::test]
async fn app_backend_checkpoints_llm_group_selection_and_falls_back_to_heuristic() {
    for (name, selection, expected_method, expected_fallback) in [
        (
            "llm-selected",
            Some(1usize),
            SpeakerDecisionMethod::Llm,
            SpeakerFallback::None,
        ),
        (
            "llm-fallback",
            None,
            SpeakerDecisionMethod::Llm,
            SpeakerFallback::Heuristic,
        ),
    ] {
        let path =
            std::env::temp_dir().join(format!("lettuce-group-llm-{name}-{}.db", RequestId::new()));
        let backend = AppBackend::open(&path, TimestampMillis::new(1)).expect("backend");
        let (scenario, speakers) = group_scenario(
            &backend,
            name,
            lettuce_characters::SpeakerSelection::Llm,
            selection.is_none(),
        );
        let settings = GlobalSettingsStore::load(backend.database()).expect("settings");
        GlobalSettingsStore::set_group_speaker_model_profile(
            backend.database(),
            None,
            settings.revision,
        )
        .expect("clear live selection model after launch");
        let selected_id = selection
            .map(|index| speakers[index])
            .unwrap_or_else(lettuce_types::ConversationParticipantId::new);
        let work = admit_and_claim(backend.database(), &scenario, 1_015);
        let inference = scripted(vec![
            call_outcome(
                &format!("{name}-selection"),
                "select_next_speaker",
                serde_json::json!({
                    "character_id": selected_id,
                    "reasoning": "This participant fits the conversation."
                }),
                (15, 2),
            ),
            text_outcome(&format!("{name}-generation"), "Group reply.", 20, 4),
        ]);
        let engine = ScenarioEmbeddingEngine;
        let result = backend
            .prepared_conversation_generation_runner(&engine, &inference)
            .run(
                &work,
                ConversationGenerationRuntimeInput::default(),
                TimestampMillis::new(1_020),
            )
            .await
            .expect("run LLM-selected group generation");
        let expected_speaker = selection.map_or(speakers[0], |index| speakers[index]);
        assert_eq!(result.candidate.author_participant_id, expected_speaker);
        let decision = result.turn.selected_speaker.expect("speaker decision");
        assert_eq!(decision.participant_id, expected_speaker);
        assert_eq!(decision.method, expected_method);
        assert_eq!(decision.fallback, expected_fallback);
        assert!(decision.usage_event_id.is_some());
        let decision_model = decision
            .decision_model
            .as_ref()
            .expect("frozen selection model");
        let selection_request = {
            let requests = inference.requests.lock().expect("requests");
            assert_eq!(requests.len(), 2);
            let selection_tools = requests[0].tools.as_ref().expect("selection tools");
            assert_eq!(
                selection_tools.choice,
                lettuce_conversations::ToolChoice::Required
            );
            assert_eq!(selection_tools.definitions[0].name, "select_next_speaker");
            let choices =
                selection_tools.definitions[0].parameters["properties"]["character_id"]["enum"]
                    .as_array()
                    .expect("speaker enum");
            assert_eq!(choices.len(), if selection.is_some() { 2 } else { 1 });
            if selection.is_none() {
                assert!(
                    !choices
                        .iter()
                        .any(|value| value == &serde_json::json!(speakers[1]))
                );
            }
            let ProviderContextPart::Text {
                text: selection_prompt,
            } = &requests[0].context.messages[0].parts[0]
            else {
                panic!("selection prompt is text");
            };
            assert!(selection_prompt.starts_with(
                "You are a narrator for a group chat. Your task is to select which character should respond next.\n\n## Participants\n\n### Participant\n- Name: \"Ada\"\n"
            ));
            assert!(
                selection_prompt
                    .contains("- Participation: 0 messages (0%)\n- Last spoke: never\n\n")
            );
            assert!(selection_prompt.contains("## Recent Conversation"));
            assert!(selection_prompt.contains("## New Message from User\n\n\"Hello cast.\""));
            assert!(
                selection_prompt
                    .ends_with("Use the select_next_speaker tool to choose a character.")
            );
            assert_eq!(selection_prompt.matches("Hello cast.").count(), 1);
            assert_eq!(requests[0].stream_sink, None);
            assert_eq!(requests[1].tools, None);
            assert_ne!(
                requests[0].profile.chat_profile.model_profile_id,
                requests[1].profile.chat_profile.model_profile_id
            );
            assert_eq!(
                requests[0].profile.chat_profile.model_profile_id,
                decision_model.source_id
            );
            requests[0].clone()
        };
        let binding =
            SpeakerInferenceBinding::from_request(scenario.conversation_id, &selection_request)
                .expect("speaker binding");
        let checkpoint =
            SpeakerInferenceRepository::speaker_inference(backend.database(), &binding)
                .expect("speaker checkpoint")
                .expect("speaker checkpoint exists");
        assert_eq!(checkpoint.decision, Some(decision.clone()));
        assert_eq!(
            checkpoint.usage_event_id,
            decision.usage_event_id.expect("decision usage")
        );
        let mut changed_request = selection_request;
        changed_request.context.messages[0].parts = vec![ProviderContextPart::Text {
            text: "Changed selection prompt.".into(),
        }];
        let changed_binding =
            SpeakerInferenceBinding::from_request(scenario.conversation_id, &changed_request)
                .expect("changed binding");
        assert!(matches!(
            SpeakerInferenceRepository::speaker_inference(backend.database(), &changed_binding),
            Err(lettuce_conversations::ConversationRepositoryError::Conflict)
        ));
        assert_eq!(
            backend
                .database()
                .job_usage(work.handle.id())
                .expect("usage")
                .len(),
            2
        );
        drop(backend);
        let reopened = AppBackend::open(&path, TimestampMillis::new(1_030)).expect("reopen");
        let replay = reopened
            .prepared_conversation_generation_runner(&engine, &inference)
            .run(
                &work,
                ConversationGenerationRuntimeInput::default(),
                TimestampMillis::new(1_031),
            )
            .await
            .expect("replay LLM-selected group generation");
        assert!(replay.replayed);
        assert_eq!(replay.candidate.id, result.candidate.id);
        assert_eq!(inference.requests.lock().expect("requests").len(), 2);
        drop(reopened);
        std::fs::remove_file(path).expect("remove test database");
    }
}

#[tokio::test]
async fn app_backend_selects_deterministic_group_speakers_before_generation() {
    for (name, policy, method) in [
        (
            "heuristic",
            lettuce_characters::SpeakerSelection::Heuristic,
            SpeakerDecisionMethod::Heuristic,
        ),
        (
            "round-robin",
            lettuce_characters::SpeakerSelection::RoundRobin,
            SpeakerDecisionMethod::RoundRobin,
        ),
    ] {
        let path =
            std::env::temp_dir().join(format!("lettuce-group-{name}-{}.db", RequestId::new()));
        let backend = AppBackend::open(&path, TimestampMillis::new(1)).expect("backend");
        let (scenario, speakers) = group_scenario(&backend, name, policy, true);
        let work = admit_and_claim(backend.database(), &scenario, 1_015);
        let inference = scripted(vec![text_outcome(
            &format!("{name}-response"),
            "Ada answers.",
            10,
            3,
        )]);
        let engine = ScenarioEmbeddingEngine;
        let result = {
            let runner = backend.prepared_conversation_generation_runner(&engine, &inference);
            runner
                .run(
                    &work,
                    ConversationGenerationRuntimeInput::default(),
                    TimestampMillis::new(1_020),
                )
                .await
                .expect("run automatically selected group speaker")
        };
        assert_eq!(result.candidate.author_participant_id, speakers[0]);
        let decision = result.turn.selected_speaker.expect("selected speaker");
        assert_eq!(decision.participant_id, speakers[0]);
        assert_eq!(decision.method, method);
        assert_eq!(decision.fallback, SpeakerFallback::None);
        assert_eq!(inference.requests.lock().expect("requests").len(), 1);
        drop(backend);
        let reopened =
            AppBackend::open(&path, TimestampMillis::new(1_021)).expect("reopen backend");
        let replay = reopened
            .prepared_conversation_generation_runner(&engine, &inference)
            .run(
                &work,
                ConversationGenerationRuntimeInput::default(),
                TimestampMillis::new(1_021),
            )
            .await
            .expect("replay automatically selected group speaker");
        assert!(replay.replayed);
        assert_eq!(replay.candidate.id, result.candidate.id);
        assert_eq!(inference.requests.lock().expect("requests").len(), 1);
        drop(reopened);
        std::fs::remove_file(path).expect("remove test database");
    }
}

#[tokio::test]
async fn user_mention_selects_a_muted_group_speaker_without_llm_selection() {
    for message in ["@beatrice, are you there?", "@\"Bee\" are you there?"] {
        user_mention_selects_the_current_character(message).await;
    }
}

async fn user_mention_selects_the_current_character(message: &str) {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let (scenario, speakers) = group_scenario_with_message(
        &backend,
        "mention",
        lettuce_characters::SpeakerSelection::Llm,
        true,
        message,
    );
    let conversation = ConversationReader::get(backend.database(), scenario.conversation_id)
        .expect("conversation")
        .conversation;
    let lettuce_conversations::ParticipantSource::Character(bea) = conversation
        .participants
        .iter()
        .find(|participant| participant.id == speakers[1])
        .expect("second speaker")
        .source
    else {
        panic!("second speaker is a character");
    };
    let details = CharacterRepository::get(backend.database(), bea)
        .expect("character")
        .expect("character exists");
    let mut profile = details.character.profile.clone();
    profile.name = "Beatrice".into();
    profile.nickname = Some("Bee".into());
    CharacterRepository::revise_profile(
        backend.database(),
        bea,
        details.character.revision,
        profile,
        TimestampMillis::new(1_012),
    )
    .expect("rename mentioned character");
    let work = admit_and_claim(backend.database(), &scenario, 1_015);
    let inference = scripted(vec![text_outcome(
        "mention-response",
        "Bea answers.",
        10,
        3,
    )]);
    let engine = ScenarioEmbeddingEngine;
    let result = backend
        .prepared_conversation_generation_runner(&engine, &inference)
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("run mentioned group speaker");
    assert_eq!(result.candidate.author_participant_id, speakers[1]);
    let decision = result.turn.selected_speaker.expect("selected speaker");
    assert_eq!(decision.participant_id, speakers[1]);
    assert_eq!(decision.method, SpeakerDecisionMethod::Explicit);
    assert!(decision.usage_event_id.is_none());
    assert_eq!(inference.requests.lock().expect("requests").len(), 1);
}

fn next_group_turn(
    database: &Database,
    scenario: &Scenario,
    key: &str,
    text: &str,
    at: i64,
) -> Scenario {
    let conversation = ConversationReader::get(database, scenario.conversation_id)
        .expect("conversation")
        .conversation;
    let sent = database
        .begin_send(
            &direct_send_command(&conversation, key, text),
            TimestampMillis::new(at),
        )
        .expect("begin next group send")
        .value;
    Scenario {
        conversation_id: scenario.conversation_id,
        turn_id: sent.turn.id,
        attempt_id: sent.attempt.id,
        model: scenario.model.clone(),
        profile: scenario.profile.clone(),
        space_id: None,
    }
}

#[tokio::test]
async fn group_selection_reads_the_timeline_oldest_first() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let database = backend.database();
    let (first, speakers) = group_scenario(
        &backend,
        "chronology",
        lettuce_characters::SpeakerSelection::RoundRobin,
        false,
    );
    let engine = ScenarioEmbeddingEngine;
    let mut scenario = first;
    let mut authors = Vec::new();
    for turn in 0..3_i64 {
        if turn > 0 {
            scenario = next_group_turn(
                database,
                &scenario,
                &format!("chronology-send-{turn}"),
                &format!("Question {turn}."),
                1_000 + turn * 100,
            );
        }
        let work = admit_and_claim(database, &scenario, 1_015 + turn * 100);
        let inference = scripted(vec![text_outcome(
            &format!("chronology-reply-{turn}"),
            &format!("Reply {turn}."),
            5,
            2,
        )]);
        let result = backend
            .prepared_conversation_generation_runner(&engine, &inference)
            .run(
                &work,
                ConversationGenerationRuntimeInput::default(),
                TimestampMillis::new(1_020 + turn * 100),
            )
            .await
            .expect("run round-robin turn");
        authors.push(result.candidate.author_participant_id);
    }
    assert_eq!(authors, [speakers[0], speakers[1], speakers[0]]);

    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let database = backend.database();
    let (first, speakers) = group_scenario(
        &backend,
        "chronology-llm",
        lettuce_characters::SpeakerSelection::Llm,
        false,
    );
    let select = |id: &str, speaker| {
        call_outcome(
            id,
            "select_next_speaker",
            serde_json::json!({"character_id": speaker, "reasoning": "Next in line."}),
            (10, 2),
        )
    };
    let inference = scripted(vec![
        select("chronology-select-0", speakers[0]),
        text_outcome("chronology-llm-reply-0", "First reply.", 5, 2),
    ]);
    backend
        .prepared_conversation_generation_runner(&engine, &inference)
        .run(
            &admit_and_claim(database, &first, 1_015),
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("run first LLM-selected turn");
    let second = next_group_turn(
        database,
        &first,
        "chronology-llm-second",
        "Second question.",
        1_100,
    );
    let inference = scripted(vec![
        select("chronology-select-1", speakers[1]),
        text_outcome("chronology-llm-reply-1", "Second reply.", 5, 2),
    ]);
    backend
        .prepared_conversation_generation_runner(&engine, &inference)
        .run(
            &admit_and_claim(database, &second, 1_115),
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_120),
        )
        .await
        .expect("run second LLM-selected turn");
    let requests = inference.requests.lock().expect("requests");
    let ProviderContextPart::Text { text: prompt } = &requests[0].context.messages[0].parts[0]
    else {
        panic!("selection prompt is text");
    };
    assert!(prompt.contains("## New Message from User\n\n\"Second question.\""));
    assert!(prompt.contains("- Entry\n- Speaker: \"User\"\n- Message: \"Hello cast.\"\n\n"));
    assert!(prompt.contains("- Last spoke: 0 turns ago\n\n"));
    assert!(prompt.contains("- Last spoke: never\n\n"));
    let recent = &prompt[prompt
        .find("## Recent Conversation")
        .expect("recent section")..];
    let hello = recent.find("Hello cast.").expect("first user message");
    let reply = recent.find("First reply.").expect("first reply");
    assert!(hello < reply);
}

#[tokio::test]
async fn app_backend_runs_resolved_group_speakers_and_rejects_unresolved_turns() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let (scenario, speakers) = group_scenario(
        &backend,
        "prepared-group",
        lettuce_characters::SpeakerSelection::Director,
        true,
    );
    let work = admit_and_claim(backend.database(), &scenario, 1_015);
    let operation = |name: &str| OperationToken {
        key: key(name),
        request_digest: ContentHash::parse("ce".repeat(32)).expect("operation digest"),
    };
    let mut turn =
        ConversationReader::get_turn(backend.database(), scenario.turn_id).expect("group turn");
    for (sequence, status) in [
        GenerationTurnStatus::Preparing,
        GenerationTurnStatus::SelectingSpeaker,
    ]
    .into_iter()
    .enumerate()
    {
        turn = backend
            .database()
            .append_event(
                turn.id,
                turn.revision,
                &operation(&format!("prepared-group-stage-{sequence}")),
                GenerationCheckpointEnvelope {
                    turn_id: turn.id,
                    attempt_id: scenario.attempt_id,
                    job_id: Some(work.handle.id()),
                    correlation_id: None,
                    sequence: u64::try_from(sequence + 1).expect("sequence"),
                    event: GenerationCheckpointEvent::Stage { status },
                },
                TimestampMillis::new(1_017 + i64::try_from(sequence).expect("time")),
            )
            .expect("stage group selection")
            .value;
    }
    let mentioned = SelectedSpeakerDecision {
        participant_id: speakers[1],
        method: SpeakerDecisionMethod::Explicit,
        fallback: SpeakerFallback::None,
        reference: None,
        rationale_summary: None,
        decision_model: None,
        usage_event_id: None,
    };
    backend
        .database()
        .resolve_group_speaker(
            &ResolveGroupSpeaker {
                conversation_id: scenario.conversation_id,
                turn_id: scenario.turn_id,
                expected_turn_revision: turn.revision,
                operation: operation("prepared-group-mentioned-speaker"),
                selected_speaker: mentioned.clone(),
            },
            TimestampMillis::new(1_019),
        )
        .expect("resolve mentioned muted speaker");
    let engine = ScenarioEmbeddingEngine;
    let inference = scripted(vec![text_outcome(
        "prepared-group-mentioned-response",
        "Bea answers.",
        12,
        3,
    )]);
    let manual_space =
        MemoryRepository::get_for_conversation(backend.database(), scenario.conversation_id)
            .expect("group memory space")
            .expect("manual group memory space exists");
    let lettuce_conversations::GenerationInput::UserMessage {
        message_id: user_message_id,
    } = turn.input
    else {
        panic!("group send starts from a user message");
    };
    MemorySummaryRepository::compare_and_apply_summary(
        backend.database(),
        MemorySummaryChange {
            expected_revision: manual_space.revision,
            summary: MemorySummary {
                space_id: manual_space.id,
                text: "The cast reached the harbor.".into(),
                token_count: 5,
                window_start: 0,
                window_end: 1,
                source_message_ids: vec![user_message_id],
                updated_at: TimestampMillis::new(1_019),
            },
        },
    )
    .expect("store group summary");
    let runner = backend.prepared_conversation_generation_runner(&engine, &inference);
    let result = runner
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("run mentioned group speaker");
    assert_eq!(result.candidate.author_participant_id, speakers[1]);
    assert_eq!(result.turn.selected_speaker, Some(mentioned));
    {
        let requests = inference.requests.lock().expect("requests");
        let texts = requests[0]
            .context
            .messages
            .iter()
            .flat_map(|message| &message.parts)
            .filter_map(|part| match part {
                ProviderContextPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(texts.contains(&"[Traveller]: Hello cast."), "{texts:?}");
        assert!(texts.contains(&"# Context Summary\nThe cast reached the harbor."));
    }
    let replay = runner
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_021),
        )
        .await
        .expect("replay mentioned group speaker");
    assert!(replay.replayed);
    assert_eq!(replay.candidate.id, result.candidate.id);
    assert_eq!(inference.requests.lock().expect("requests").len(), 1);

    let current = ConversationReader::get(backend.database(), scenario.conversation_id)
        .expect("conversation after mentioned response")
        .conversation;
    let continued = backend
        .database()
        .begin_continue(
            &ContinueConversation {
                conversation_id: current.id,
                branch_id: current.active_branch_id,
                expected_revision: current.revision,
                forced_speaker: Some(speakers[0]),
                swap_roles: false,
                operation: operation("prepared-group-director-continue"),
            },
            TimestampMillis::new(1_022),
        )
        .expect("begin director continuation")
        .value;
    let continued_scenario = Scenario {
        conversation_id: scenario.conversation_id,
        turn_id: continued.turn.id,
        attempt_id: continued.attempt.id,
        model: scenario.model.clone(),
        profile: scenario.profile.clone(),
        space_id: None,
    };
    let continued_work = admit_and_claim(backend.database(), &continued_scenario, 1_023);
    let continued_inference = scripted(vec![text_outcome(
        "prepared-group-director-response",
        "Ada continues.",
        8,
        3,
    )]);
    let continued_result = backend
        .prepared_conversation_generation_runner(&engine, &continued_inference)
        .run(
            &continued_work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_025),
        )
        .await
        .expect("run director continuation");
    assert_eq!(
        continued_result.candidate.author_participant_id,
        speakers[0]
    );
    assert!(continued_result.turn.selected_speaker.is_none());
    assert!(
        !continued_inference.requests.lock().expect("requests")[0]
            .context
            .messages
            .iter()
            .flat_map(|message| &message.parts)
            .any(|part| matches!(
                part,
                ProviderContextPart::Text { text }
                    if text.starts_with("[CONTINUE]") || text.starts_with("[Continue speaking as")
            ))
    );

    let current = ConversationReader::get(backend.database(), scenario.conversation_id)
        .expect("conversation after director continuation")
        .conversation;
    let regenerated = backend
        .database()
        .begin_regenerate(
            &lettuce_conversations::RegenerateCandidate {
                conversation_id: current.id,
                branch_id: current.active_branch_id,
                message_id: continued_result.candidate.message_id,
                turn_id: continued_result.turn.id,
                expected_revision: current.revision,
                expected_turn_revision: continued_result.turn.revision,
                operation: operation("prepared-group-regenerate"),
                active_candidate_id: continued_result.candidate.id,
                guidance: None,
                model_override: None,
                forced_speaker: None,
                swap_roles: false,
            },
            TimestampMillis::new(1_026),
        )
        .expect("begin retained-author regeneration")
        .value;
    let regenerated_scenario = Scenario {
        conversation_id: scenario.conversation_id,
        turn_id: regenerated.turn.id,
        attempt_id: regenerated.attempt.id,
        model: scenario.model.clone(),
        profile: scenario.profile.clone(),
        space_id: None,
    };
    let regenerated_work = admit_and_claim(backend.database(), &regenerated_scenario, 1_027);
    let regenerated_inference = scripted(vec![text_outcome(
        "prepared-group-regenerate-response",
        "Ada answers differently.",
        9,
        4,
    )]);
    let regenerated_result = backend
        .prepared_conversation_generation_runner(&engine, &regenerated_inference)
        .run(
            &regenerated_work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_029),
        )
        .await
        .expect("run retained-author regeneration");
    assert_eq!(
        regenerated_result.candidate.author_participant_id,
        speakers[0]
    );
    assert_eq!(regenerated_result.candidate.ordinal, 1);

    let unresolved_backend =
        AppBackend::open_in_memory(TimestampMillis::new(1)).expect("unresolved backend");
    let (unresolved, _) = group_scenario(
        &unresolved_backend,
        "unresolved-group",
        lettuce_characters::SpeakerSelection::Director,
        true,
    );
    let unresolved_work = admit_and_claim(unresolved_backend.database(), &unresolved, 1_015);
    let unresolved_inference = scripted(vec![text_outcome(
        "unresolved-group-response",
        "Must not dispatch.",
        1,
        1,
    )]);
    assert!(matches!(
        unresolved_backend
            .prepared_conversation_generation_runner(&engine, &unresolved_inference)
            .run(
                &unresolved_work,
                ConversationGenerationRuntimeInput::default(),
                TimestampMillis::new(1_020),
            )
            .await,
        Err(ConversationGenerationRunError::PreparationFailed {
            code: GenerationFailureCode::SpeakerUnavailable,
        })
    ));
    assert!(
        unresolved_inference
            .requests
            .lock()
            .expect("unresolved requests")
            .is_empty()
    );
}

#[tokio::test]
async fn app_backend_builds_dynamic_memory_input_and_replays_exactly() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let stored_settings = GlobalSettingsStore::load(backend.database()).expect("settings");
    let mut settings = stored_settings.settings;
    settings.dynamic_memory.enabled = true;
    settings.dynamic_memory.max_entries = 12;
    settings.dynamic_memory.hot_memory_token_budget = 321;
    settings.dynamic_memory.retrieval_limit = 1;
    settings.dynamic_memory.duplicate_threshold_basis_points = 8_800;
    GlobalSettingsStore::save(
        backend.database(),
        settings,
        stored_settings.default_model_profile_id,
        stored_settings.revision,
    )
    .expect("save dynamic memory settings");
    let scenario =
        scenario_with_resolvable_profile(backend.database(), true, "prepared-dynamic", true);
    let live_settings = GlobalSettingsStore::load(backend.database()).expect("live settings");
    let mut changed_settings = live_settings.settings.clone();
    changed_settings.dynamic_memory.max_entries = 99;
    changed_settings.dynamic_memory.hot_memory_token_budget = 999;
    changed_settings.dynamic_memory.min_similarity_basis_points = 10_000;
    changed_settings.dynamic_memory.retrieval_limit = 2;
    changed_settings
        .dynamic_memory
        .duplicate_threshold_basis_points = 1_000;
    GlobalSettingsStore::save(
        backend.database(),
        changed_settings,
        live_settings.default_model_profile_id,
        live_settings.revision,
    )
    .expect("change live dynamic memory settings after launch");
    let space_id = scenario.space_id.expect("dynamic memory space");
    let memory_id = MemoryId::new();
    let stored = MemoryRepository::get(backend.database(), space_id)
        .expect("memory")
        .expect("memory exists");
    let memory = MemoryItem {
        id: memory_id,
        short_id: lettuce_memory::MemoryShortId::derived(memory_id),
        text: "Mira prefers tea by the harbor.".into(),
        category: MemoryCategory::Preference,
        source_message_id: None,
        source_role: None,
        observed_at: None,
        observed_time_precision: None,
        superseded_by: None,
        superseded_at: None,
        supersedes: vec![],
        token_count: 6,
        is_cold: true,
        is_pinned: false,
        importance: Score::from_basis_points(3_000).expect("score"),
        persistence_importance: Score::from_basis_points(8_000).expect("score"),
        prompt_importance: Score::from_basis_points(8_000).expect("score"),
        volatility: Score::LEGACY_VOLATILITY,
        access_count: 2,
        created_at: TimestampMillis::new(900),
        last_accessed_at: TimestampMillis::new(900),
    };
    let untouched_memory = MemoryItem {
        id: MemoryId::new(),
        short_id: lettuce_memory::MemoryShortId::new(900002).expect("short id"),
        text: "Mira catalogued the northern lighthouse.".into(),
        category: MemoryCategory::WorldDetail,
        source_message_id: None,
        source_role: None,
        observed_at: None,
        observed_time_precision: None,
        superseded_by: None,
        superseded_at: None,
        supersedes: vec![],
        token_count: 5,
        is_cold: false,
        is_pinned: false,
        importance: Score::from_basis_points(4_000).expect("score"),
        persistence_importance: Score::from_basis_points(4_000).expect("score"),
        prompt_importance: Score::from_basis_points(4_000).expect("score"),
        volatility: Score::LEGACY_VOLATILITY,
        access_count: 7,
        created_at: TimestampMillis::new(800),
        last_accessed_at: TimestampMillis::new(850),
    };
    let stored = MemoryRepository::compare_and_apply(
        backend.database(),
        MemoryChangeSet {
            space_id,
            expected_revision: stored.revision,
            items: vec![memory.clone(), untouched_memory.clone()],
        },
    )
    .expect("seed memory");
    MemoryEmbeddingRepository::put_ready(
        backend.database(),
        MemoryEmbeddingProjection {
            space_id,
            memory_id,
            source_text: memory.text.clone(),
            vector: EmbeddingVector {
                source_revision: "scenario-v1".into(),
                values: {
                    let mut values = vec![0.0; 128];
                    values[0] = 1.0;
                    values
                },
            },
            dimensions: EmbeddingDimensions::D128,
            updated_at: TimestampMillis::new(1_012),
        },
    )
    .expect("seed projection");
    let work = admit_and_claim(backend.database(), &scenario, 1_015);
    let inference = scripted(vec![text_outcome(
        "prepared-dynamic-response",
        "I will remember that.",
        5,
        3,
    )]);
    let engine = ScenarioEmbeddingEngine;
    let runner = backend.prepared_conversation_generation_runner(&engine, &inference);
    let first_build = runner
        .build_input(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("first dynamic build");
    let rebuilt = runner
        .build_input(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_025),
        )
        .await
        .expect("dynamic rebuild after retrieval access");
    assert_eq!(rebuilt, first_build);
    let result = runner
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("prepared dynamic send");
    assert_eq!(result.outcomes.len(), 1);
    {
        let requests = inference.requests.lock().expect("requests");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].tools, None);
        assert_eq!(requests[0].profile.tool_policy, ToolPolicy::Disabled);
        let system_texts = requests[0]
            .context
            .messages
            .iter()
            .filter(|message| message.role == MessageRole::System)
            .filter_map(|message| match message.parts.as_slice() {
                [ProviderContextPart::Text { text }] => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            system_texts,
            [
                "Relevant memories:\n- Mira prefers tea by the harbor.",
                "# Key Memories\nImportant facts to remember in this conversation:\n- Mira catalogued the northern lighthouse.",
            ]
        );
        assert!(requests[0].context.messages.iter().any(|message| {
            message.parts.iter().any(|part| {
                matches!(part, ProviderContextPart::Text { text } if text.contains("- Mira prefers tea by the harbor."))
            })
        }));
    }
    let prepared_turn =
        ConversationReader::get_turn(backend.database(), scenario.turn_id).expect("prepared turn");
    let accessed_revision = stored.revision.next().expect("retrieval revision");
    assert_eq!(
        prepared_turn.memory,
        Some(lettuce_conversations::MemoryAttribution {
            revision_id: lettuce_memory::memory_revision_id(space_id, accessed_revision),
        })
    );
    let accessed = MemoryRepository::get(backend.database(), space_id)
        .expect("memory after retrieval")
        .expect("memory exists after retrieval");
    let accessed_item = accessed
        .items
        .iter()
        .find(|item| item.id == memory_id)
        .expect("retrieved memory remains");
    assert_eq!(accessed_item.access_count, 4);
    assert_eq!(accessed_item.last_accessed_at, TimestampMillis::new(1_020));
    assert!(!accessed_item.is_pinned);
    assert!(!accessed_item.is_cold);
    assert_eq!(
        accessed
            .items
            .iter()
            .find(|item| item.id == untouched_memory.id),
        Some(&untouched_memory)
    );
    let retrieval_access = MemoryRetrievalAccess {
        conversation_id: scenario.conversation_id,
        turn_id: scenario.turn_id,
        attempt_id: scenario.attempt_id,
        space_id,
        expected_revision: stored.revision,
        selected_memory_ids: vec![memory_id],
        accessed_at: TimestampMillis::new(1_020),
    };
    assert_eq!(
        MemoryRetrievalRepository::get_retrieval_access(
            backend.database(),
            scenario.conversation_id,
            scenario.turn_id,
            scenario.attempt_id,
        )
        .expect("read retrieval access")
        .expect("retrieval access exists"),
        MemoryRetrievalAccessReceipt {
            access: retrieval_access.clone(),
            resulting_revision: accessed_revision,
            promoted_memory_ids: vec![memory_id],
        }
    );
    assert_eq!(
        MemoryRetrievalRepository::apply_retrieval_access(
            backend.database(),
            retrieval_access.clone(),
        )
        .expect("replay retrieval access")
        .resulting_revision,
        accessed_revision
    );
    let mut conflicting_selection = retrieval_access.clone();
    conflicting_selection.selected_memory_ids = vec![MemoryId::new()];
    assert_eq!(
        MemoryRetrievalRepository::apply_retrieval_access(
            backend.database(),
            conflicting_selection,
        ),
        Err(MemoryRepositoryError::Conflict)
    );
    let mut stale_access = retrieval_access;
    stale_access.expected_revision = accessed_revision;
    assert_eq!(
        MemoryRetrievalRepository::apply_retrieval_access(backend.database(), stale_access),
        Err(MemoryRepositoryError::Conflict)
    );
    let aggregate_event = UsageLedger::get(backend.database(), result.usage_event_id)
        .expect("usage")
        .expect("usage exists");
    assert_eq!(
        aggregate_event.record.usage,
        UsageCounters::Known(usage(5, 3).expect("reply usage"))
    );
    let replay = runner
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_030),
        )
        .await
        .expect("prepared dynamic replay");
    assert!(replay.replayed);
    assert_eq!(replay.candidate.id, result.candidate.id);
    assert_eq!(inference.requests.lock().expect("requests").len(), 1);
    assert_eq!(
        MemoryRepository::get(backend.database(), space_id)
            .expect("memory after replay")
            .expect("memory exists after replay"),
        accessed
    );
}

#[tokio::test]
async fn disabled_global_dynamic_memory_renders_direct_memories_like_manual_mode() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let scenario =
        scenario_with_resolvable_profile(backend.database(), true, "gated-dynamic", true);
    let space_id = scenario.space_id.expect("dynamic memory space");
    let stored = MemoryRepository::get(backend.database(), space_id)
        .expect("memory")
        .expect("memory exists");
    let cold_memory = MemoryItem {
        id: MemoryId::new(),
        short_id: lettuce_memory::MemoryShortId::new(900003).expect("short id"),
        text: "Mira hides a spare key under the mat.".into(),
        category: MemoryCategory::WorldDetail,
        source_message_id: None,
        source_role: None,
        observed_at: None,
        observed_time_precision: None,
        superseded_by: None,
        superseded_at: None,
        supersedes: vec![],
        token_count: 8,
        is_cold: true,
        is_pinned: false,
        importance: Score::from_basis_points(3_000).expect("score"),
        persistence_importance: Score::from_basis_points(3_000).expect("score"),
        prompt_importance: Score::from_basis_points(3_000).expect("score"),
        volatility: Score::LEGACY_VOLATILITY,
        access_count: 0,
        created_at: TimestampMillis::new(900),
        last_accessed_at: TimestampMillis::new(900),
    };
    MemoryRepository::compare_and_apply(
        backend.database(),
        MemoryChangeSet {
            space_id,
            expected_revision: stored.revision,
            items: vec![cold_memory],
        },
    )
    .expect("seed memory");
    let work = admit_and_claim(backend.database(), &scenario, 1_015);
    let inference = scripted(vec![text_outcome("gated-dynamic-response", "Noted.", 5, 3)]);
    let engine = ScenarioEmbeddingEngine;
    backend
        .prepared_conversation_generation_runner(&engine, &inference)
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("send with global dynamic memory disabled");
    let requests = inference.requests.lock().expect("requests");
    let system_texts = requests[0]
        .context
        .messages
        .iter()
        .filter(|message| message.role == MessageRole::System)
        .filter_map(|message| match message.parts.as_slice() {
            [ProviderContextPart::Text { text }] => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        system_texts,
        [
            "# Key Memories\nImportant facts to remember in this conversation:\n- Mira hides a spare key under the mat."
        ]
    );
    assert_eq!(
        MemoryRetrievalRepository::get_retrieval_access(
            backend.database(),
            scenario.conversation_id,
            scenario.turn_id,
            scenario.attempt_id,
        )
        .expect("read retrieval access"),
        None
    );
}

#[tokio::test]
async fn chat_runtime_sections_follow_catalog_edits() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let scenario =
        scenario_with_resolvable_profile(backend.database(), false, "runtime-edit", true);
    let space =
        MemoryRepository::get_for_conversation(backend.database(), scenario.conversation_id)
            .expect("manual memory space")
            .expect("manual memory exists");
    MemoryRepository::compare_and_apply(
        backend.database(),
        MemoryChangeSet {
            space_id: space.id,
            expected_revision: space.revision,
            items: vec![MemoryItem {
                id: MemoryId::new(),
                short_id: lettuce_memory::MemoryShortId::new(900004).expect("short id"),
                text: "Mira keeps a brass compass.".into(),
                category: MemoryCategory::WorldDetail,
                source_message_id: None,
                source_role: None,
                observed_at: None,
                observed_time_precision: None,
                superseded_by: None,
                superseded_at: None,
                supersedes: vec![],
                token_count: 6,
                is_cold: false,
                is_pinned: false,
                importance: Score::FULL,
                persistence_importance: Score::FULL,
                prompt_importance: Score::FULL,
                volatility: Score::LEGACY_VOLATILITY,
                access_count: 0,
                created_at: TimestampMillis::new(1_012),
                last_accessed_at: TimestampMillis::new(1_012),
            }],
        },
    )
    .expect("store manual memory");
    let runtime_id = backend
        .built_in_prompt_ids()
        .get(crate::BuiltInPromptId::ChatRuntime);
    let runtime = lettuce_context::PromptRepository::get(backend.database(), runtime_id)
        .expect("runtime prompt")
        .expect("runtime prompt exists");
    let key_entry = runtime
        .entries
        .iter()
        .find(|entry| entry.built_in_entry_key.as_deref() == Some("runtime_key_memories"))
        .expect("key memories entry");
    lettuce_context::PromptRepository::mutate_entries(
        backend.database(),
        runtime.id,
        runtime.revision,
        lettuce_context::PromptEntryMutation::Update {
            entry_id: key_entry.id,
            draft: lettuce_context::PromptEntryDraft {
                built_in_entry_key: key_entry.built_in_entry_key.clone(),
                name: key_entry.name.clone(),
                role: key_entry.role,
                content: "Facts {{char.name}} knows:\n{{key_memories}}".into(),
                enabled: key_entry.enabled,
                injection_position: key_entry.injection_position,
                depth: key_entry.depth,
                conditional_min_messages: key_entry.conditional_min_messages,
                interval_turns: key_entry.interval_turns,
                system_prompt: key_entry.system_prompt,
                conditions: key_entry.conditions.clone(),
                payload: key_entry.payload.clone(),
            },
        },
        TimestampMillis::new(1_013),
    )
    .expect("edit runtime key memories entry");
    let work = admit_and_claim(backend.database(), &scenario, 1_015);
    let inference = scripted(vec![text_outcome("runtime-edit-response", "Noted.", 5, 3)]);
    let engine = ScenarioEmbeddingEngine;
    backend
        .prepared_conversation_generation_runner(&engine, &inference)
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("send with an edited runtime prompt");
    let requests = inference.requests.lock().expect("requests");
    assert!(requests[0].context.messages.iter().any(|message| {
        message.parts.iter().any(|part| {
            matches!(
                part,
                ProviderContextPart::Text { text }
                    if text == "Facts Ada knows:\n- Mira keeps a brass compass."
            )
        })
    }));
}

#[tokio::test]
async fn plain_dynamic_turns_admit_and_run_a_post_turn_memory_cycle() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let database = backend.database();
    let scenario = scenario_with_resolvable_profile(database, true, "post-turn", true);
    let generation = admit_and_claim(database, &scenario, 1_015);
    let engine = ScenarioEmbeddingEngine;
    let reply = scripted(vec![text_outcome("post-turn-reply", "Tea it is.", 5, 3)]);
    backend
        .prepared_conversation_generation_runner(&engine, &reply)
        .run(
            &generation,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("finalize plain dynamic turn");
    assert!(reply.requests.lock().expect("requests")[0].tools.is_none());

    let dispatch = crate::CompanionMemoryDispatchCoordinator::new(database, database);
    let claim = |interval, mode| {
        dispatch.admit_plain_after_turn_and_claim(
            scenario.conversation_id,
            interval,
            mode,
            WorkerId::new(),
            TimestampMillis::new(1_030),
            LEASE,
            &ResourceAvailability::all(),
        )
    };
    assert!(
        claim(3, lettuce_memory::DynamicMemoryRunMode::Auto)
            .expect("interval not reached")
            .is_empty()
    );
    assert!(
        claim(2, lettuce_memory::DynamicMemoryRunMode::Manual)
            .expect("manual mode")
            .is_empty()
    );
    assert!(
        claim(2, lettuce_memory::DynamicMemoryRunMode::AskFirst)
            .expect("ask first")
            .is_empty()
    );
    assert_eq!(
        dispatch
            .pending_approval_count(scenario.conversation_id)
            .expect("pending approval"),
        Some(2)
    );
    let work = claim(2, lettuce_memory::DynamicMemoryRunMode::Auto)
        .expect("auto admission")
        .into_iter()
        .next()
        .expect("claimed plain memory work");
    let expected_sources = work
        .admission
        .batch
        .source_messages()
        .expect("plain window");
    assert_eq!(expected_sources.len(), 2);
    assert_eq!(expected_sources[0].1, MessageRole::User);
    assert_eq!(expected_sources[1].1, MessageRole::Assistant);
    assert!(work.admission.batch.effects().is_empty());
    assert_eq!(
        dispatch
            .pending_approval_count(scenario.conversation_id)
            .expect("cleared approval"),
        None
    );

    let prompt_ids = BuiltInPromptService::new(database)
        .expect("prompt service")
        .bootstrap(TimestampMillis::new(1_031))
        .expect("prompt ids");
    let memory_prompt =
        PromptRepository::get(database, prompt_ids.get(BuiltInPromptId::DynamicMemory))
            .expect("prompt")
            .expect("dynamic memory prompt");
    let summary_prompt =
        PromptRepository::get(database, prompt_ids.get(BuiltInPromptId::DynamicSummary))
            .expect("prompt")
            .expect("dynamic summary prompt");
    let memory = scripted(vec![
        call_outcome(
            "post-turn-summary",
            "write_summary",
            serde_json::json!({"summary": "The user chose tea."}),
            (6, 2),
        ),
        call_outcome(
            "post-turn-create",
            "create_memory",
            serde_json::json!({"text": "The user prefers tea", "category": "preference"}),
            (7, 2),
        ),
    ]);
    let memory_id = MemoryId::new();
    let result = crate::CompanionMemoryJobRunner::new(&engine, database, database, &memory)
        .run(
            &work.admission,
            ResolvedInferenceProfile {
                chat_profile: scenario.profile.clone(),
                tool_policy: ToolPolicy::Required,
                output_policy: OutputPolicy::Plain,
                safety_policy: SafetyContext::Standard,
                correlation_id: None,
            },
            false,
            false,
            lettuce_memory::DynamicMemoryStructuredFallbackFormat::Xml,
            &summary_prompt,
            &memory_prompt,
            &DynamicMemoryPolicy {
                max_entries: 10,
                hot_token_budget: 2,
                cold_threshold: Score::from_basis_points(2_000).expect("score"),
                delete_confidence_default: Score::from_basis_points(5_000).expect("score"),
                max_hard_delete_ratio_per_cycle: Score::from_basis_points(5_000).expect("score"),
            },
            Score::from_basis_points(9_000).expect("score"),
            &work.claim,
            &work.handle,
            None,
            TimestampMillis::new(1_032),
            |round| {
                if round.ordinal == 0 {
                    vec![crate::MemoryCreateSeed {
                        execution_id: round.calls[0].id,
                        id: memory_id,
                        token_count: 4,
                        created_at: TimestampMillis::new(1_032),
                    }]
                } else {
                    Vec::new()
                }
            },
        )
        .await
        .expect("run plain post-turn memory cycle");
    assert_eq!(
        result.dispatch.attempt.status,
        lettuce_memory::DynamicMemoryAttemptStatus::Succeeded
    );
    assert_eq!(
        memory.requests.lock().expect("memory requests").len(),
        2,
        "without recursive loops a cycle makes one summary and one memory request"
    );
    assert!(result.effects.is_empty());
    let space_id = scenario.space_id.expect("dynamic memory space");
    let summary = MemorySummaryRepository::get_summary(database, space_id)
        .expect("summary")
        .expect("stored summary");
    assert_eq!(summary.text, "The user chose tea.");
    assert_eq!(summary.window_end, 2);
    assert_eq!(
        summary.source_message_ids,
        expected_sources
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>()
    );
    let stored = MemoryRepository::get(database, space_id)
        .expect("memory")
        .expect("memory space");
    assert_eq!(stored.items.len(), 1);
    assert_eq!(stored.items[0].id, memory_id);
    assert!(
        stored.items[0].is_cold,
        "the cycle end demotes the created memory past the two-token hot budget"
    );
    assert!(
        claim(2, lettuce_memory::DynamicMemoryRunMode::Auto)
            .expect("cursor advanced")
            .is_empty()
    );
}

#[tokio::test]
async fn post_turn_memory_host_runs_the_plain_cycle_from_live_settings() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let database = backend.database();
    let stored = GlobalSettingsStore::load(database).expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.enabled = true;
    settings.dynamic_memory.summary_message_interval = 2;
    settings.dynamic_memory.hot_memory_token_budget = 2;
    settings.dynamic_memory.structured_fallback_format =
        lettuce_settings::MemoryStructuredFallbackFormat::Json;
    GlobalSettingsStore::save(
        database,
        settings,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("save dynamic memory settings");
    let scenario = scenario_with_resolvable_profile(database, true, "host-post-turn", true);
    let generation = admit_and_claim(database, &scenario, 1_015);
    let engine = ScenarioEmbeddingEngine;
    let reply = scripted(vec![text_outcome("host-reply", "Tea it is.", 5, 3)]);
    backend
        .prepared_conversation_generation_runner(&engine, &reply)
        .run(
            &generation,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("finalize plain dynamic turn");
    let memory = scripted(vec![
        call_outcome(
            "host-summary",
            "write_summary",
            serde_json::json!({"summary": "The user chose tea."}),
            (6, 2),
        ),
        call_outcome(
            "host-create",
            "create_memory",
            serde_json::json!({"text": "The user prefers tea", "category": "preference"}),
            (7, 2),
        ),
    ]);
    let host = backend.companion_memory_host(&engine, &memory);
    let claim = |operation| {
        host.after_turn(
            scenario.conversation_id,
            operation,
            WorkerId::new(),
            TimestampMillis::new(1_030),
            LEASE,
            &ResourceAvailability::all(),
        )
    };
    assert!(
        claim(lettuce_conversations::GenerationOperation::Regenerate)
            .expect("regenerate never admits a cycle")
            .is_empty()
    );
    let work = claim(lettuce_conversations::GenerationOperation::Send)
        .expect("send admits the interval window")
        .into_iter()
        .next()
        .expect("claimed plain memory work");
    let inputs = host
        .resolve_runtime_inputs(&work.admission)
        .expect("runtime inputs from live settings");
    assert_eq!(inputs.policy.hot_token_budget, 2);
    assert_eq!(
        inputs.structured_fallback_format,
        lettuce_memory::DynamicMemoryStructuredFallbackFormat::Json
    );
    assert_eq!(
        inputs.memory_prompt.purpose,
        PromptPurpose::DynamicMemoryManager
    );
    assert_eq!(
        inputs.summary_prompt.purpose,
        PromptPurpose::DynamicMemorySummarizer
    );
    assert!(!inputs.supersession_enabled);
    assert_eq!(
        inputs.profile.chat_profile.model_profile_id,
        scenario.model.source_id
    );
    let settled = host
        .run_claimed(work, CancellationReason::User, TimestampMillis::new(1_032))
        .await
        .expect("run and settle the claimed cycle");
    let crate::CompanionMemorySettledWork::Succeeded { result, job } = settled else {
        panic!("the plain cycle should succeed");
    };
    assert_eq!(job.state, JobState::Succeeded);
    assert_eq!(
        result.dispatch.attempt.status,
        lettuce_memory::DynamicMemoryAttemptStatus::Succeeded
    );
    assert_eq!(memory.requests.lock().expect("memory requests").len(), 2);
    let space_id = scenario.space_id.expect("dynamic memory space");
    let summary = MemorySummaryRepository::get_summary(database, space_id)
        .expect("summary")
        .expect("stored summary");
    assert_eq!(summary.text, "The user chose tea.");
    let stored_memory = MemoryRepository::get(database, space_id)
        .expect("memory")
        .expect("memory space");
    assert_eq!(stored_memory.items.len(), 1);
    assert_eq!(stored_memory.items[0].text, "The user prefers tea");
    assert_eq!(stored_memory.items[0].token_count, 4);
    assert!(stored_memory.items[0].is_cold);

    let stored = GlobalSettingsStore::load(database).expect("settings");
    let mut disabled = stored.settings;
    disabled.dynamic_memory.enabled = false;
    GlobalSettingsStore::save(
        database,
        disabled,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("disable dynamic memory");
    assert!(
        claim(lettuce_conversations::GenerationOperation::Continue)
            .expect("disabled global setting admits nothing")
            .is_empty()
    );
}

#[tokio::test]
async fn preexisting_progress_checkpoint_advances_runner_stage_sequences() {
    let database = database();
    let scenario = scenario(&database, false, "progress-sequence");
    let work = admit_and_claim(&database, &scenario, 1_015);
    let turn = ConversationReader::get_turn(&database, scenario.turn_id).expect("turn");
    database
        .append_event(
            scenario.turn_id,
            turn.revision,
            &OperationToken {
                key: key("progress-sequence-existing"),
                request_digest: ContentHash::parse("ba".repeat(32)).expect("digest"),
            },
            GenerationCheckpointEnvelope {
                turn_id: scenario.turn_id,
                attempt_id: scenario.attempt_id,
                job_id: Some(work.handle.id()),
                correlation_id: None,
                sequence: 1,
                event: GenerationCheckpointEvent::Progress { emitted_parts: 2 },
            },
            TimestampMillis::new(1_017),
        )
        .expect("preexisting progress checkpoint");
    let inference = scripted(vec![text_outcome(
        "progress-sequence-response",
        "Continued after progress",
        9,
        3,
    )]);
    let result = ConversationGenerationJobRunner::new(&database, &inference)
        .run(&work, input(&scenario), TimestampMillis::new(1_020))
        .await
        .expect("run after progress checkpoint");

    assert_eq!(result.turn.status, GenerationTurnStatus::Succeeded);
    assert_eq!(
        database
            .latest_checkpoint_sequence(scenario.turn_id, scenario.attempt_id)
            .expect("latest sequence"),
        Some(3)
    );
}

#[tokio::test]
async fn cancellation_before_dispatch_settles_turn_and_job_without_evidence() {
    let database = database();
    let scenario = scenario(&database, false, "cancel-early");
    let work = admit_and_claim(&database, &scenario, 1_015);
    let inference = scripted(vec![text_outcome("unused", "Unused", 1, 1)]);
    let runner = ConversationGenerationJobRunner::new(&database, &inference);
    work.handle.request_cancel();
    let error = runner
        .run(&work, input(&scenario), TimestampMillis::new(1_020))
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
        .run(&work, input(&scenario), TimestampMillis::new(1_030))
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
    let runner = ConversationGenerationJobRunner::new(&database, &inference);
    let error = {
        let run = runner.run(&work, input(&scenario), TimestampMillis::new(1_020));
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
        profile: input(&scenario).profile,
        context: input(&scenario).context,
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
        .run(&work, input(&scenario), TimestampMillis::new(1_030))
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
async fn chat_reply_tool_calls_are_rejected_and_the_turn_fails() {
    let database = database();
    let scenario = scenario(&database, false, "tool-reply");
    let work = admit_and_claim(&database, &scenario, 1_015);
    let inference = scripted(vec![call_outcome(
        "tool-reply",
        "create_memory",
        serde_json::json!({"text": "Mira prefers tea"}),
        (10, 2),
    )]);
    let runner = ConversationGenerationJobRunner::new(&database, &inference);
    let error = runner
        .run(&work, input(&scenario), TimestampMillis::new(1_020))
        .await
        .expect_err("tool calls on a chat reply are rejected");
    assert!(matches!(
        error,
        ConversationGenerationRunError::Provider {
            error: PortError::Rejected,
            evidence: GenerationUsageEvidence::Dispatch(_),
        }
    ));
    assert_eq!(inference.requests.lock().expect("requests")[0].tools, None);
    ConversationGenerationDispatchCoordinator::new(&database, &database)
        .settle(
            work,
            Err(error),
            CancellationReason::User,
            TimestampMillis::new(1_021),
        )
        .expect("settle rejected reply");
    let turn = ConversationReader::get_turn(&database, scenario.turn_id).expect("turn");
    assert_eq!(turn.status, GenerationTurnStatus::Failed);
    assert_eq!(turn.failure, Some(GenerationFailureCode::ProviderRejected));
    assert!(turn.selected_candidate_id.is_none());
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
    let runner = ConversationGenerationJobRunner::new(&database, &inference);
    let error = runner
        .run(&work, input(&scenario), TimestampMillis::new(1_020))
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
        .run(&work, input(&scenario), TimestampMillis::new(1_030))
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
        .run(&work, input(&scenario), TimestampMillis::new(1_040))
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
    crate::BuiltInPromptService::new(&database)
        .expect("built-in prompt catalog")
        .bootstrap(TimestampMillis::new(1))
        .expect("bootstrap built-in prompts");
    let scenario = scenario_with_resolvable_profile(&database, false, "pending", true);
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
    let error = PreparedConversationGenerationJobRunner::new(&engine, &database, &inference)
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
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
    let pending = database
        .initial_inference_for_attempt(
            scenario.conversation_id,
            scenario.turn_id,
            scenario.attempt_id,
            parent_job_id,
        )
        .expect("pending")
        .expect("record");
    let binding = pending.binding.clone();
    let mut request = pending.request.clone();
    assert!(pending.result.is_none());
    drop(database);

    let database = Database::open(&path).expect("reopen");
    let mut changed_model = ModelProfileRepository::get(&database, scenario.model.source_id)
        .expect("live model")
        .expect("live model exists");
    let model_revision = changed_model.revision;
    changed_model.display_name = "Changed after admission".into();
    ModelProfileRepository::upsert(&database, changed_model, Some(model_revision))
        .expect("change live model after admission");
    let dispatcher = ConversationGenerationDispatchCoordinator::new(&database, &database);
    let work = claim(&database, &scenario, scenario.attempt_id, 1_040);
    assert_eq!(work.handle.id(), parent_job_id);
    let error = PreparedConversationGenerationJobRunner::new(&engine, &database, &inference)
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_041),
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
    let rerun = ConversationGenerationJobRunner::new(&database, &inference)
        .run(&work, input(&scenario), TimestampMillis::new(1_043))
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
    let result = PreparedConversationGenerationJobRunner::new(&engine, &database, &inference)
        .run(
            &child_work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_051),
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
    let runner = ConversationGenerationJobRunner::new(&database, &inference);
    let work = admit_and_claim(&database, &scenario, 1_015);
    let mut foreign = work.clone();
    foreign.handle = JobHandle::new(JobId::new());
    assert!(matches!(
        runner
            .run(&foreign, input(&scenario), TimestampMillis::new(1_020),)
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
    let runner = ConversationGenerationJobRunner::new(&database, &inference);
    runner
        .run(&work, input(&scenario), TimestampMillis::new(1_020))
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
        .run(&work, input(&scenario), TimestampMillis::new(1_030))
        .await
        .expect("replay");
    assert_eq!(records(&database), first);
    assert_eq!(inference.requests.lock().expect("requests").len(), 1);
}
