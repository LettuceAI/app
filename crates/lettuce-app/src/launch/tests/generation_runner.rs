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

use crate::generation::conversation_generation::{
    ConversationGenerationOperation, operation_token,
};
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
    direct_scenario(
        database,
        dynamic_memory,
        prefix,
        persist_resolvable_profile,
        true,
    )
}

fn direct_scenario(
    database: &Database,
    dynamic_memory: bool,
    prefix: &str,
    persist_resolvable_profile: bool,
    character_prompt: bool,
) -> Scenario {
    direct_scenario_with(
        database,
        dynamic_memory,
        prefix,
        persist_resolvable_profile,
        character_prompt,
        |_| {},
    )
}

fn direct_scenario_with(
    database: &Database,
    dynamic_memory: bool,
    prefix: &str,
    persist_resolvable_profile: bool,
    character_prompt: bool,
    configure: impl FnOnce(&mut CharacterDefaults),
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
    let empty_prompt = character_prompt
        .then(|| seed_prompt(database, "Scenario prompt", PromptPurpose::DirectChat));
    let character_id = seed_character(database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.direct_prompt_id = empty_prompt;
        if dynamic_memory {
            defaults.memory_policy = MemoryPolicy::Dynamic;
        }
        configure(defaults);
    });
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
        strip_time_stamps: false,
        reply_images: None,
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
        image_tokens: None,
        audio_tokens: None,
        total_tokens: None,
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
    group_scenario_with(
        backend,
        prefix,
        speaker_selection,
        mute_second,
        message,
        |_| {},
    )
}

fn group_scenario_with(
    backend: &AppBackend,
    prefix: &str,
    speaker_selection: lettuce_characters::SpeakerSelection,
    mute_second: bool,
    message: &str,
    configure: impl FnOnce(&mut GroupProfile),
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
        configure(group);
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
async fn a_direct_chat_follows_the_live_default_model_on_each_turn() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let database = backend.database();
    let scenario = scenario_with_resolvable_profile(database, false, "live-model", true);
    let switched = seed_model(database, ProviderProtocol::Ollama, "ollama");
    let mut model = ModelProfileRepository::get(database, switched)
        .expect("model")
        .expect("model exists");
    let revision = model.revision;
    model.config.chat_parameters.temperature = None;
    model.config.capabilities.streaming = lettuce_models::CapabilityStatus::Supported;
    ModelProfileRepository::upsert(database, model, Some(revision)).expect("resolvable model");
    set_application_default_model(database, switched);
    let inference = scripted(vec![text_outcome("live-model", "Live reply.", 12, 3)]);
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
        ConversationGenerationSettledWork::Succeeded { result, .. },
    ) = outcome
    else {
        panic!("generation succeeds");
    };
    assert_ne!(switched, scenario.model.source_id);
    assert_eq!(
        inference.requests.lock().expect("requests")[0]
            .profile
            .chat_profile
            .model_profile_id,
        switched,
        "legacy select_model_with_credential read the default model on every turn"
    );
    assert_eq!(
        result.turn.resolved_model.map(|model| model.source_id),
        Some(switched)
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
    let mut streamed = execution_request(&running, CancellationToken::new());
    streamed.runtime.stream_sink = Some(RequestId::new());
    let run = runner.execute(streamed, &clock);
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
    let settled = settled.expect("settle running cancellation");
    let ConversationGenerationExecutionOutcome::Settled(
        ConversationGenerationSettledWork::Succeeded {
            ref result,
            ref job,
        },
    ) = settled
    else {
        panic!("legacy useChatAbortController keeps the streamed reply on stop: {settled:?}");
    };
    assert_eq!(job.state, JobState::Cancelled);
    assert_eq!(
        result.candidate.parts,
        vec![MessagePart::Text {
            text: "Late reply".into()
        }]
    );
    assert_eq!(inference.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(!inference_runtime.is_cancelled(running_job.id));
    assert_eq!(
        ConversationReader::get_turn(backend.database(), running.turn_id)
            .expect("stopped running turn")
            .status,
        GenerationTurnStatus::Succeeded
    );
}

#[tokio::test]
async fn restart_recovery_settles_every_turn_the_previous_process_left_live() {
    let path = std::env::temp_dir().join(format!(
        "lettuce-generation-restart-{}.db",
        ConversationId::new()
    ));
    let backend = AppBackend::open(&path, TimestampMillis::new(1)).expect("backend");
    let engine = ScenarioEmbeddingEngine;
    let clock = FakeClock::new(TimestampMillis::new(1_020));

    let unstarted =
        scenario_with_resolvable_profile(backend.database(), false, "restart-unstarted", true);

    let stopping =
        scenario_with_resolvable_profile(backend.database(), false, "restart-stopping", true);
    let stopping_job = backend
        .conversation_generation_dispatcher()
        .admit(
            stopping.conversation_id,
            stopping.turn_id,
            stopping.attempt_id,
            TimestampMillis::new(1_010),
        )
        .expect("admit stopping generation")
        .job;
    claim(backend.database(), &stopping, stopping.attempt_id, 1_011);
    assert!(matches!(
        backend
            .conversation_generation_cancellation()
            .cancel(
                stopping_job.id,
                CancellationReason::User,
                TimestampMillis::new(1_012),
            )
            .expect("request cancellation"),
        ConversationGenerationCancellationOutcome::Requested { .. }
    ));

    let running =
        scenario_with_resolvable_profile(backend.database(), false, "restart-running", true);
    let running_job = backend
        .conversation_generation_dispatcher()
        .admit(
            running.conversation_id,
            running.turn_id,
            running.attempt_id,
            TimestampMillis::new(1_015),
        )
        .expect("admit running generation")
        .job;
    let inference = BlockingInference::new(text_outcome("lost", "Lost reply", 9, 4));
    let runner = backend.prepared_conversation_generation_runner(&engine, &inference);
    tokio::select! {
        _ = runner.execute(execution_request(&running, CancellationToken::new()), &clock) => {
            panic!("the blocked generation must not finish");
        }
        () = inference.entered.notified() => {}
    }
    drop(backend);

    let backend = AppBackend::open(&path, TimestampMillis::new(1_030)).expect("reopen backend");
    let report = backend
        .recover_after_restart(TimestampMillis::new(1_030))
        .expect("restart recovery");
    let settled = |turn_id| {
        report
            .turns
            .iter()
            .find(|(id, _)| *id == turn_id)
            .map(|(_, settlement)| *settlement)
    };
    assert_eq!(
        settled(unstarted.turn_id),
        Some(crate::ConversationGenerationRestartSettlement::Cancelled)
    );
    assert_eq!(
        settled(stopping.turn_id),
        Some(crate::ConversationGenerationRestartSettlement::Cancelled)
    );
    assert_eq!(
        settled(running.turn_id),
        Some(crate::ConversationGenerationRestartSettlement::Interrupted)
    );
    let turn =
        |turn_id| ConversationReader::get_turn(backend.database(), turn_id).expect("settled turn");
    assert_eq!(
        turn(unstarted.turn_id).status,
        GenerationTurnStatus::Cancelled
    );
    assert_eq!(
        turn(stopping.turn_id).status,
        GenerationTurnStatus::Cancelled
    );
    let interrupted = turn(running.turn_id);
    assert_eq!(interrupted.status, GenerationTurnStatus::Interrupted);
    assert_eq!(
        interrupted.attempts[0].status,
        GenerationAttemptStatus::Interrupted
    );
    assert_eq!(
        attempt_usage(backend.database(), running.turn_id, 0)
            .record
            .outcome,
        UsageOutcome::Interrupted
    );
    assert_eq!(
        persisted_job(backend.database(), running_job.id).state,
        JobState::Cancelled
    );
    assert!(report.cancelled_generation_jobs.contains(&running_job.id));
    assert!(
        persisted_job(backend.database(), stopping_job.id)
            .state
            .is_terminal()
    );
    assert!(
        lettuce_conversations::LiveTurnReader::live_turns(backend.database(), 10)
            .expect("live turns")
            .is_empty()
    );
    let again = backend
        .recover_after_restart(TimestampMillis::new(1_031))
        .expect("second recovery");
    assert!(
        again.jobs.is_empty()
            && again.cancelled_generation_jobs.is_empty()
            && again.turns.is_empty()
    );
    drop(backend);
    let _ = std::fs::remove_file(&path);
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
async fn a_chat_with_a_ten_thousand_entry_lorebook_and_129_lorebooks_runs_a_turn() {
    let path = std::env::temp_dir().join(format!(
        "lettuce-large-lorebook-{}.db",
        ConversationId::new()
    ));
    let backend = AppBackend::open(&path, TimestampMillis::new(1)).expect("backend");
    let database = backend.database();
    let lorebook = LorebookRepository::create(
        database,
        LorebookMetadataDraft {
            name: "Atlas".into(),
            detection_policy: DetectionPolicy::RecentMessageWindow,
            icon_asset_id: None,
            behavior_version: LorebookBehaviorVersion::LegacyV1,
        },
        (0..10_001)
            .map(|index| lettuce_context::LorebookEntryDraft {
                title: format!("Entry {index}"),
                enabled: true,
                always_active: false,
                keywords: vec!["tea".into()],
                case_sensitive: false,
                match_mode: lettuce_context::KeywordMatchMode::Literal,
                content: format!("Atlas fact {index}."),
                priority: 0,
            })
            .collect(),
        TimestampMillis::new(1),
    )
    .expect("large lorebook");
    let persona_id = seed_persona(database, "Traveller");
    let mut revision = PersonaLorebookBindingRepository::bind_persona_lorebook(
        database,
        persona_id,
        Revision::INITIAL,
        LorebookBindingCreate {
            lorebook_id: lorebook.book.id,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind lorebook")
    .owner_revision;
    for index in 0..129 {
        let small = LorebookRepository::create(
            database,
            LorebookMetadataDraft {
                name: format!("Shelf {index}"),
                detection_policy: DetectionPolicy::RecentMessageWindow,
                icon_asset_id: None,
                behavior_version: LorebookBehaviorVersion::LegacyV1,
            },
            vec![lettuce_context::LorebookEntryDraft {
                title: format!("Shelf {index}"),
                enabled: true,
                always_active: true,
                keywords: Vec::new(),
                case_sensitive: false,
                match_mode: lettuce_context::KeywordMatchMode::Literal,
                content: format!("Shelf note {index}."),
                priority: 0,
            }],
            TimestampMillis::new(1),
        )
        .expect("small lorebook");
        revision = PersonaLorebookBindingRepository::bind_persona_lorebook(
            database,
            persona_id,
            revision,
            LorebookBindingCreate {
                lorebook_id: small.book.id,
                target: BindingInsertionTarget::Append,
            },
            NOW,
        )
        .expect("bind small lorebook")
        .owner_revision;
    }
    let default_revision = PersonaRepository::get_default_snapshot(database)
        .expect("default persona")
        .state
        .revision;
    PersonaRepository::set_default(database, persona_id, default_revision, NOW)
        .expect("default persona");
    let scenario = direct_scenario(database, false, "large-lorebook", true, false);
    let turn = ConversationReader::get_turn(database, scenario.turn_id).expect("scheduled turn");
    let generation = BeginGeneration {
        conversation: ConversationReader::get(database, scenario.conversation_id)
            .expect("scheduled conversation")
            .conversation,
        attempt: turn.attempts[0].clone(),
        turn,
    };
    backend
        .conversation_generation_dispatcher()
        .schedule(&generation, TimestampMillis::new(1_020))
        .expect("schedule generation");
    let inference = scripted(vec![text_outcome("atlas", "Noted.", 12, 3)]);
    let engine = ScenarioEmbeddingEngine;
    let clock = FakeClock::new(TimestampMillis::new(1_022));
    let outcome = backend
        .prepared_conversation_generation_runner(&engine, &inference)
        .execute_next(
            ConversationGenerationWorkerRequest {
                worker_id: WorkerId::new(),
                lease_for: LEASE,
                resources: ResourceAvailability::all(),
            },
            &clock,
        )
        .await
        .expect("execute generation");
    let ConversationGenerationWorkerOutcome::Executed(outcome) = outcome else {
        panic!("worker executes the turn");
    };
    assert!(
        matches!(
            *outcome,
            ConversationGenerationExecutionOutcome::Settled(
                ConversationGenerationSettledWork::Succeeded { .. }
            )
        ),
        "{outcome:?}"
    );
    let requests = inference.requests.lock().expect("requests");
    assert_eq!(requests.len(), 1);
    let sent = format!("{:?}", requests[0]);
    assert!(sent.contains("Atlas fact 0."));
    assert!(sent.contains("Atlas fact 10000."));
    assert!(sent.contains("Shelf note 128."));
    drop(requests);
    drop(backend);
    std::fs::remove_file(&path).ok();
}

#[tokio::test]
async fn conversation_and_app_model_settings_reach_the_resolved_request() {
    use lettuce_conversations::ConversationRepository as _;
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let scenario =
        scenario_with_resolvable_profile(backend.database(), false, "layered-settings", true);
    let (_, settings_revision) =
        lettuce_models::GlobalModelSettingsRepository::global_model_settings(backend.database())
            .expect("global model settings");
    lettuce_models::GlobalModelSettingsRepository::save_global_model_settings(
        backend.database(),
        lettuce_models::ModelSettingsLayer {
            chat_parameters: lettuce_models::ChatParameterProfile {
                context_length: Some(12_288),
                max_output_tokens: Some(111),
                ..Default::default()
            },
            ..Default::default()
        },
        settings_revision,
        TimestampMillis::now().expect("clock"),
    )
    .expect("save global model settings");
    backend
        .database()
        .update_settings(
            lettuce_conversations::PreparedConversationSettingsUpdate::new(
                lettuce_conversations::UpdateConversationSettings {
                    conversation_id: scenario.conversation_id,
                    expected_settings_revision: None,
                    operation: lettuce_conversations::OperationToken {
                        key: lettuce_jobs::IdempotencyKey::new("layered-settings").expect("key"),
                        request_digest: lettuce_types::ContentHash::parse("cd".repeat(32))
                            .expect("digest"),
                    },
                    patch: lettuce_conversations::CurrentConversationSettingsPatch {
                        model_settings: lettuce_conversations::PatchValue::Set(
                            lettuce_models::ModelSettingsLayer {
                                chat_parameters: lettuce_models::ChatParameterProfile {
                                    max_output_tokens: Some(333),
                                    ..Default::default()
                                },
                                ..Default::default()
                            },
                        ),
                        ..lettuce_conversations::CurrentConversationSettingsPatch::default()
                    },
                },
                Vec::new(),
            )
            .expect("prepared settings"),
            TimestampMillis::new(1_012),
        )
        .expect("conversation model settings");
    let work = admit_and_claim(backend.database(), &scenario, 1_015);
    let inference = scripted(vec![text_outcome("layered-response", "Reply", 13, 4)]);
    let engine = ScenarioEmbeddingEngine;
    backend
        .prepared_conversation_generation_runner(&engine, &inference)
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("layered generation");
    let requests = inference.requests.lock().expect("requests");
    let parameters = &requests[0].profile.chat_profile.parameters;
    assert_eq!(
        parameters.visible_max_output_tokens,
        Some(333),
        "the conversation layer overrides the model and the app"
    );
    assert_eq!(
        parameters.context_length,
        Some(8_192),
        "the model's own value wins over the app layer"
    );
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
    changed_settings.dynamic_memory.min_similarity_basis_points = Some(10_000);
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
    MemoryEmbeddingRepository::put_ready(
        backend.database(),
        MemoryEmbeddingProjection {
            space_id,
            memory_id: untouched_memory.id,
            source_text: untouched_memory.text.clone(),
            vector: EmbeddingVector {
                source_revision: "scenario-v1".into(),
                values: {
                    let mut values = vec![0.0; 128];
                    values[1] = 1.0;
                    values
                },
            },
            dimensions: EmbeddingDimensions::D128,
            updated_at: TimestampMillis::new(1_012),
        },
    )
    .expect("seed unrelated projection");
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
                "# Key Memories\nImportant facts to remember in this conversation:\n- Mira prefers tea by the harbor."
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
    let accessed_revision = stored.revision;
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
    stale_access.expected_revision = accessed_revision.next().expect("next revision");
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

fn enable_retrieval_only_dynamic_memory(database: &Database) {
    let stored = GlobalSettingsStore::load(database).expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.enabled = true;
    settings.dynamic_memory.retrieval_limit = 1;
    settings.dynamic_memory.min_similarity_basis_points = Some(5_000);
    GlobalSettingsStore::save(
        database,
        settings,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("enable dynamic memory");
}

fn seed_retrieved_and_hot_memories(database: &Database, space_id: MemorySpaceId) {
    let stored = MemoryRepository::get(database, space_id)
        .expect("memory")
        .expect("memory exists");
    let memory = |text: &str, is_cold: bool, observed_at: Option<TimestampMillis>| {
        let id = MemoryId::new();
        MemoryItem {
            id,
            short_id: lettuce_memory::MemoryShortId::derived(id),
            text: text.into(),
            category: MemoryCategory::WorldDetail,
            source_message_id: observed_at.map(|_| lettuce_types::MessageId::new()),
            source_role: observed_at.map(|_| MessageRole::User),
            observed_at,
            observed_time_precision: observed_at.map(|_| "turn".into()),
            superseded_by: None,
            superseded_at: None,
            supersedes: vec![],
            token_count: 6,
            is_cold,
            is_pinned: false,
            importance: Score::from_basis_points(4_000).expect("score"),
            persistence_importance: Score::from_basis_points(4_000).expect("score"),
            prompt_importance: Score::from_basis_points(4_000).expect("score"),
            volatility: Score::LEGACY_VOLATILITY,
            access_count: 0,
            created_at: TimestampMillis::new(900),
            last_accessed_at: TimestampMillis::new(900),
        }
    };
    let retrieved = memory(
        "Mira prefers tea by the harbor.",
        true,
        Some(TimestampMillis::new(1_000)),
    );
    let hot = memory("Mira catalogued the northern lighthouse.", false, None);
    MemoryRepository::compare_and_apply(
        database,
        MemoryChangeSet {
            space_id,
            expected_revision: stored.revision,
            items: vec![retrieved.clone(), hot.clone()],
        },
    )
    .expect("seed memories");
    for (item, axis) in [(&retrieved, 0), (&hot, 1)] {
        let mut values = vec![0.0; 128];
        values[axis] = 1.0;
        MemoryEmbeddingRepository::put_ready(
            database,
            MemoryEmbeddingProjection {
                space_id,
                memory_id: item.id,
                source_text: item.text.clone(),
                vector: EmbeddingVector {
                    source_revision: "scenario-v1".into(),
                    values,
                },
                dimensions: EmbeddingDimensions::D128,
                updated_at: TimestampMillis::new(1_012),
            },
        )
        .expect("seed projection");
    }
}

fn request_system_texts(inference: &ScriptedInference) -> Vec<String> {
    inference.requests.lock().expect("requests")[0]
        .context
        .messages
        .iter()
        .filter(|message| message.role == MessageRole::System)
        .flat_map(|message| &message.parts)
        .filter_map(|part| match part {
            ProviderContextPart::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn bundled_default_prompt_renders_only_retrieved_memories_in_a_dynamic_direct_chat() {
    use chrono::TimeZone;

    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    enable_retrieval_only_dynamic_memory(backend.database());
    let scenario = direct_scenario(backend.database(), true, "bundled-dynamic", true, false);
    let conversation = ConversationReader::get(backend.database(), scenario.conversation_id)
        .expect("conversation")
        .conversation;
    let ConversationKind::Direct(details) = &conversation.kind else {
        panic!("expected a direct conversation");
    };
    match &details.prompt {
        SnapshotSelection::Inherited(prompt) => assert_eq!(
            prompt.source_id,
            backend
                .built_in_prompt_ids()
                .get(crate::BuiltInPromptId::AppDefault)
        ),
        other => panic!("expected the bundled app default prompt, got {other:?}"),
    }
    seed_retrieved_and_hot_memories(
        backend.database(),
        scenario.space_id.expect("dynamic memory space"),
    );
    let observed = chrono::Local
        .timestamp_millis_opt(1_000)
        .single()
        .expect("local observation time")
        .format("%Y-%m-%d %H:%M");
    let key_memories = format!(
        "# Key Memories\nImportant facts to remember in this conversation:\n- Mira prefers tea by the harbor. (observed {observed}, just now)"
    );
    let assert_retrieved_only = |system_texts: &[String]| {
        assert!(
            system_texts
                .iter()
                .any(|text| text.starts_with("# Your Character: ")),
            "{system_texts:?}"
        );
        assert_eq!(
            system_texts
                .iter()
                .filter(|text| text.contains("Mira"))
                .collect::<Vec<_>>(),
            [&key_memories],
            "{system_texts:?}"
        );
        assert!(
            system_texts
                .iter()
                .all(|text| !text.contains("Relevant memories")
                    && !text.contains("northern lighthouse")),
            "{system_texts:?}"
        );
    };
    let engine = ScenarioEmbeddingEngine;
    let work = admit_and_claim(backend.database(), &scenario, 1_015);
    let inference = scripted(vec![text_outcome(
        "bundled-dynamic-response",
        "Tea it is.",
        9,
        3,
    )]);
    let sent = backend
        .prepared_conversation_generation_runner(&engine, &inference)
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("send in a bundled-prompt dynamic chat");
    assert_retrieved_only(&request_system_texts(&inference));

    let current = ConversationReader::get(backend.database(), scenario.conversation_id)
        .expect("conversation after send")
        .conversation;
    let regenerated = backend
        .database()
        .begin_regenerate(
            &lettuce_conversations::RegenerateCandidate {
                conversation_id: current.id,
                branch_id: current.active_branch_id,
                message_id: sent.candidate.message_id,
                turn_id: sent.turn.id,
                expected_revision: current.revision,
                expected_turn_revision: sent.turn.revision,
                operation: OperationToken {
                    key: key("bundled-dynamic-regenerate"),
                    request_digest: ContentHash::parse("cf".repeat(32)).expect("digest"),
                },
                active_candidate_id: sent.candidate.id,
                guidance: None,
                model_override: None,
                forced_speaker: None,
                swap_roles: false,
            },
            TimestampMillis::new(1_021),
        )
        .expect("begin regeneration")
        .value;
    let regenerated_scenario = Scenario {
        turn_id: regenerated.turn.id,
        attempt_id: regenerated.attempt.id,
        ..scenario
    };
    let regenerated_work = admit_and_claim(backend.database(), &regenerated_scenario, 1_022);
    let regenerated_inference = scripted(vec![text_outcome(
        "bundled-dynamic-regenerate-response",
        "Tea again.",
        9,
        3,
    )]);
    backend
        .prepared_conversation_generation_runner(&engine, &regenerated_inference)
        .run(
            &regenerated_work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_024),
        )
        .await
        .expect("regenerate in a bundled-prompt dynamic chat");
    assert_retrieved_only(&request_system_texts(&regenerated_inference));
}

#[tokio::test]
async fn dynamic_group_chats_keep_retrieved_key_memories_without_observation_notes() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    enable_retrieval_only_dynamic_memory(backend.database());
    let (scenario, _) = group_scenario_with(
        &backend,
        "group-dynamic",
        lettuce_characters::SpeakerSelection::Heuristic,
        false,
        "Remember tea.",
        |group| group.memory_policy = MemoryPolicy::Dynamic,
    );
    let space =
        MemoryRepository::get_for_conversation(backend.database(), scenario.conversation_id)
            .expect("group memory space")
            .expect("group memory exists");
    seed_retrieved_and_hot_memories(backend.database(), space.id);
    let work = admit_and_claim(backend.database(), &scenario, 1_015);
    let inference = scripted(vec![text_outcome(
        "group-dynamic-response",
        "Ada nods.",
        9,
        3,
    )]);
    let engine = ScenarioEmbeddingEngine;
    backend
        .prepared_conversation_generation_runner(&engine, &inference)
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("run a dynamic group turn");
    let system_texts = request_system_texts(&inference);
    assert!(
        system_texts.iter().any(|text| text.contains(
            "# Key Memories\nImportant facts to remember in this conversation:\n- Mira prefers tea by the harbor."
        )),
        "{system_texts:?}"
    );
    assert!(
        system_texts.iter().all(|text| !text.contains("(observed ")
            && !text.contains("Relevant memories")
            && !text.contains("northern lighthouse")),
        "{system_texts:?}"
    );
}

fn observed_note(observed_at: i64) -> String {
    use chrono::TimeZone;

    chrono::Local
        .timestamp_millis_opt(observed_at)
        .single()
        .expect("local observation time")
        .format("%Y-%m-%d %H:%M")
        .to_string()
}

fn assert_only_the_retrieved_key_memories(system_texts: &[String], key_memories: &str) {
    assert_eq!(
        system_texts
            .iter()
            .filter(|text| text.contains("Mira"))
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [key_memories],
        "{system_texts:?}"
    );
    assert!(
        system_texts.iter().all(
            |text| !text.contains("Relevant memories") && !text.contains("northern lighthouse")
        ),
        "{system_texts:?}"
    );
}

#[tokio::test]
async fn dynamic_direct_continuation_sends_only_the_retrieved_memories_as_key_memories() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    enable_retrieval_only_dynamic_memory(backend.database());
    let scenario = direct_scenario(backend.database(), true, "continue-dynamic", true, false);
    seed_retrieved_and_hot_memories(
        backend.database(),
        scenario.space_id.expect("dynamic memory space"),
    );
    let key_memories = format!(
        "# Key Memories\nImportant facts to remember in this conversation:\n- Mira prefers tea by the harbor. (observed {}, just now)",
        observed_note(1_000)
    );
    let engine = ScenarioEmbeddingEngine;
    let work = admit_and_claim(backend.database(), &scenario, 1_015);
    let inference = scripted(vec![text_outcome(
        "continue-dynamic-response",
        "Tea it is.",
        9,
        3,
    )]);
    backend
        .prepared_conversation_generation_runner(&engine, &inference)
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("send in a dynamic chat");
    assert_only_the_retrieved_key_memories(&request_system_texts(&inference), &key_memories);

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
                operation: OperationToken {
                    key: key("continue-dynamic-continue"),
                    request_digest: ContentHash::parse("ce".repeat(32)).expect("digest"),
                },
            },
            TimestampMillis::new(1_021),
        )
        .expect("begin continuation")
        .value;
    let continued_scenario = Scenario {
        turn_id: continued.turn.id,
        attempt_id: continued.attempt.id,
        ..scenario
    };
    let continued_work = admit_and_claim(backend.database(), &continued_scenario, 1_022);
    let continued_inference = scripted(vec![text_outcome(
        "continue-dynamic-continue-response",
        "And more tea.",
        9,
        3,
    )]);
    backend
        .prepared_conversation_generation_runner(&engine, &continued_inference)
        .run(
            &continued_work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_024),
        )
        .await
        .expect("continue in a dynamic chat");
    assert_only_the_retrieved_key_memories(
        &request_system_texts(&continued_inference),
        &key_memories,
    );
    let receipt = MemoryRetrievalRepository::get_retrieval_access(
        backend.database(),
        continued_scenario.conversation_id,
        continued_scenario.turn_id,
        continued_scenario.attempt_id,
    )
    .expect("read continuation retrieval access")
    .expect("continuation retrieval access exists");
    assert_eq!(receipt.access.selected_memory_ids.len(), 1);
    assert_eq!(receipt.access.accessed_at, TimestampMillis::new(1_024));
}

#[tokio::test]
async fn dynamic_companion_chat_fills_its_continuity_section_with_only_the_retrieved_memories() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    enable_retrieval_only_dynamic_memory(backend.database());
    let scenario = direct_scenario_with(
        backend.database(),
        true,
        "companion-dynamic",
        true,
        false,
        |defaults| {
            defaults.interaction_mode = InteractionMode::Companion;
            defaults.companion_soul = Some(lettuce_companions::CompanionSoulConfig::default());
        },
    );
    seed_retrieved_and_hot_memories(
        backend.database(),
        scenario.space_id.expect("companion memory space"),
    );
    let engine = ScenarioEmbeddingEngine;
    let work = admit_and_claim(backend.database(), &scenario, 1_015);
    let inference = scripted(vec![text_outcome(
        "companion-dynamic-response",
        "Tea it is.",
        9,
        3,
    )]);
    backend
        .prepared_conversation_generation_runner(&engine, &inference)
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("send in a dynamic companion chat");
    let system_texts = request_system_texts(&inference);
    let continuity = system_texts
        .iter()
        .filter(|text| text.contains("Mira"))
        .collect::<Vec<_>>();
    assert_eq!(continuity.len(), 1, "{system_texts:?}");
    assert!(
        continuity[0].starts_with("# Relationship Continuity\n"),
        "{system_texts:?}"
    );
    assert!(
        continuity[0].ends_with(&format!(
            "\n\n## Key Memories\nUnless a memory explicitly describes a third-party relationship, interpret relationship, boundary, preference, profile, routine, plan, and milestone memories as continuity between Ada and Traveller.\n- Mira prefers tea by the harbor. (observed {}, just now)\n\n## Relevant Lore",
            observed_note(1_000)
        )),
        "{system_texts:?}"
    );
    assert!(
        system_texts
            .iter()
            .all(|text| !text.starts_with("# Key Memories")
                && !text.contains("Relevant memories")
                && !text.contains("northern lighthouse")),
        "{system_texts:?}"
    );
}

fn seed_unrelated_cold_memory(database: &Database, space_id: MemorySpaceId) {
    let stored = MemoryRepository::get(database, space_id)
        .expect("memory")
        .expect("memory exists");
    let id = MemoryId::new();
    let item = MemoryItem {
        id,
        short_id: lettuce_memory::MemoryShortId::derived(id),
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
        is_cold: true,
        is_pinned: false,
        importance: Score::from_basis_points(4_000).expect("score"),
        persistence_importance: Score::from_basis_points(4_000).expect("score"),
        prompt_importance: Score::from_basis_points(4_000).expect("score"),
        volatility: Score::LEGACY_VOLATILITY,
        access_count: 0,
        created_at: TimestampMillis::new(900),
        last_accessed_at: TimestampMillis::new(900),
    };
    MemoryRepository::compare_and_apply(
        database,
        MemoryChangeSet {
            space_id,
            expected_revision: stored.revision,
            items: vec![item.clone()],
        },
    )
    .expect("seed memory");
    let mut values = vec![0.0; 128];
    values[1] = 1.0;
    MemoryEmbeddingRepository::put_ready(
        database,
        MemoryEmbeddingProjection {
            space_id,
            memory_id: item.id,
            source_text: item.text.clone(),
            vector: EmbeddingVector {
                source_revision: "scenario-v1".into(),
                values,
            },
            dimensions: EmbeddingDimensions::D128,
            updated_at: TimestampMillis::new(1_012),
        },
    )
    .expect("seed projection");
}

async fn empty_retrieval_system_texts(
    prefix: &str,
    character_prompt: bool,
    seed_memory: bool,
) -> Vec<String> {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    enable_retrieval_only_dynamic_memory(backend.database());
    let scenario = direct_scenario(backend.database(), true, prefix, true, character_prompt);
    if seed_memory {
        seed_unrelated_cold_memory(
            backend.database(),
            scenario.space_id.expect("dynamic memory space"),
        );
    }
    let engine = ScenarioEmbeddingEngine;
    let work = admit_and_claim(backend.database(), &scenario, 1_015);
    let inference = scripted(vec![text_outcome(
        &format!("{prefix}-response"),
        "Noted.",
        5,
        3,
    )]);
    backend
        .prepared_conversation_generation_runner(&engine, &inference)
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("send with nothing retrieved");
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
    request_system_texts(&inference)
}

#[tokio::test]
async fn empty_dynamic_retrieval_keeps_the_bundled_heading_without_a_fallback_block() {
    for (prefix, seed_memory) in [("empty-unmatched", true), ("empty-space", false)] {
        let bundled = empty_retrieval_system_texts(prefix, false, seed_memory).await;
        assert!(
            bundled.iter().any(|text| text.trim_end()
                == "# Key Memories\nImportant facts to remember in this conversation:"),
            "{bundled:?}"
        );
        assert!(
            bundled.iter().all(|text| !text.contains("Mira")),
            "{bundled:?}"
        );
        let custom =
            empty_retrieval_system_texts(&format!("{prefix}-custom"), true, seed_memory).await;
        assert!(
            custom
                .iter()
                .all(|text| !text.contains("Key Memories") && !text.contains("Mira")),
            "{custom:?}"
        );
    }
}

struct CalibratedEmbeddingEngine {
    calibration: lettuce_embeddings::SimilarityCalibration,
}

impl crate::MemoryEmbeddingEngine for CalibratedEmbeddingEngine {
    fn source_revision(&self) -> &str {
        "calibrated-v1"
    }

    fn dimensions(&self) -> EmbeddingDimensions {
        EmbeddingDimensions::D128
    }

    fn calibration(&self) -> lettuce_embeddings::SimilarityCalibration {
        self.calibration
    }

    fn count_tokens(&self, text: &str) -> Result<u32, crate::EmbeddingGenerationError> {
        u32::try_from(text.split_whitespace().count())
            .map_err(|_| crate::EmbeddingGenerationError::Unavailable)
    }

    fn embed_memory(
        &self,
        request: &EmbeddingRequest,
        cancellation: &CancellationToken,
    ) -> Result<EmbeddingVector, crate::EmbeddingGenerationError> {
        if cancellation.is_cancelled() {
            return Err(crate::EmbeddingGenerationError::Cancelled);
        }
        let cosine = if request.text.contains("harbor") {
            0.75_f32
        } else if request.text.contains("lighthouse") {
            0.7
        } else {
            1.0
        };
        let mut values = vec![0.0; request.dimensions.get()];
        values[0] = cosine;
        values[1] = (1.0 - cosine * cosine).sqrt();
        Ok(EmbeddingVector {
            source_revision: self.source_revision().into(),
            values,
        })
    }
}

fn eidos_calibration() -> lettuce_embeddings::SimilarityCalibration {
    lettuce_embeddings::SimilarityCalibration::from_json(
        br#"{"default_threshold": 0.5, "fallback_threshold": 0.35, "dims": {
            "768": {"a": 2.381, "b": -1.381}, "512": {"a": 2.2901, "b": -1.2863},
            "256": {"a": 2.2388, "b": -1.244}, "128": {"a": 2.2388, "b": -1.2664},
            "64": {"a": 1.9481, "b": -1.0058}}}"#,
    )
    .expect("calibration")
}

async fn calibrated_retrieval(
    prefix: &str,
    min_similarity_basis_points: Option<u16>,
) -> (Vec<String>, Vec<MemoryId>, Vec<MemoryId>) {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let stored = GlobalSettingsStore::load(backend.database()).expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.enabled = true;
    settings.dynamic_memory.retrieval_limit = 2;
    settings.dynamic_memory.retrieval_strategy = lettuce_settings::MemoryRetrievalStrategy::Cosine;
    settings.dynamic_memory.min_similarity_basis_points = min_similarity_basis_points;
    GlobalSettingsStore::save(
        backend.database(),
        settings,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("configure dynamic memory");
    let scenario = scenario_with_resolvable_profile(backend.database(), true, prefix, true);
    let space_id = scenario.space_id.expect("dynamic memory space");
    let space = MemoryRepository::get(backend.database(), space_id)
        .expect("memory")
        .expect("memory exists");
    let memory = |text: &str| {
        let id = MemoryId::new();
        MemoryItem {
            id,
            short_id: lettuce_memory::MemoryShortId::derived(id),
            text: text.into(),
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
            importance: Score::from_basis_points(4_000).expect("score"),
            persistence_importance: Score::from_basis_points(4_000).expect("score"),
            prompt_importance: Score::from_basis_points(4_000).expect("score"),
            volatility: Score::LEGACY_VOLATILITY,
            access_count: 0,
            created_at: TimestampMillis::new(900),
            last_accessed_at: TimestampMillis::new(900),
        }
    };
    let harbor = memory("Mira prefers tea by the harbor.");
    let lighthouse = memory("Mira catalogued the northern lighthouse.");
    MemoryRepository::compare_and_apply(
        backend.database(),
        MemoryChangeSet {
            space_id,
            expected_revision: space.revision,
            items: vec![harbor.clone(), lighthouse.clone()],
        },
    )
    .expect("seed memories");
    let mut stale = vec![0.0; 128];
    stale[0] = 1.0;
    MemoryEmbeddingRepository::put_ready(
        backend.database(),
        MemoryEmbeddingProjection {
            space_id,
            memory_id: lighthouse.id,
            source_text: lighthouse.text.clone(),
            vector: EmbeddingVector {
                source_revision: "scenario-v1".into(),
                values: stale,
            },
            dimensions: EmbeddingDimensions::D128,
            updated_at: TimestampMillis::new(1_012),
        },
    )
    .expect("seed a vector from another embedding family");
    let engine = CalibratedEmbeddingEngine {
        calibration: eidos_calibration(),
    };
    let work = admit_and_claim(backend.database(), &scenario, 1_015);
    let inference = scripted(vec![text_outcome(
        &format!("{prefix}-response"),
        "Noted.",
        5,
        3,
    )]);
    backend
        .prepared_conversation_generation_runner(&engine, &inference)
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("send with a calibrated embedding model");
    let mut embedded = backend
        .database()
        .list_ready(space_id, "calibrated-v1", EmbeddingDimensions::D128)
        .expect("ready projections")
        .into_iter()
        .map(|projection| projection.memory_id)
        .collect::<Vec<_>>();
    embedded.sort();
    let mut expected_embedded = vec![harbor.id, lighthouse.id];
    expected_embedded.sort();
    assert_eq!(embedded, expected_embedded);
    let selected = MemoryRetrievalRepository::get_retrieval_access(
        backend.database(),
        scenario.conversation_id,
        scenario.turn_id,
        scenario.attempt_id,
    )
    .expect("read retrieval access")
    .map(|receipt| receipt.access.selected_memory_ids)
    .unwrap_or_default();
    (request_system_texts(&inference), selected, vec![harbor.id])
}

#[tokio::test]
async fn calibrated_retrieval_embeds_missing_vectors_and_falls_back_only_when_unset() {
    let (system_texts, selected, harbor) = calibrated_retrieval("eidos-unset", None).await;
    assert_eq!(selected, harbor);
    assert_eq!(
        system_texts,
        [
            "# Key Memories\nImportant facts to remember in this conversation:\n- Mira prefers tea by the harbor."
        ]
    );
    let (system_texts, selected, _) = calibrated_retrieval("eidos-set", Some(5_000)).await;
    assert!(selected.is_empty());
    assert!(
        system_texts.iter().all(|text| !text.contains("Mira")),
        "{system_texts:?}"
    );
    let (system_texts, selected, harbor) = calibrated_retrieval("eidos-low", Some(4_000)).await;
    assert_eq!(selected, harbor);
    assert_eq!(
        system_texts,
        [
            "# Key Memories\nImportant facts to remember in this conversation:\n- Mira prefers tea by the harbor."
        ]
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
                decay_rate: Score::from_basis_points(800).expect("score"),
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
    let toggle_enabled = |enabled: bool| {
        let stored = GlobalSettingsStore::load(database).expect("settings");
        let mut settings = stored.settings;
        settings.dynamic_memory.enabled = enabled;
        GlobalSettingsStore::save(
            database,
            settings,
            stored.default_model_profile_id,
            stored.revision,
        )
        .expect("toggle dynamic memory");
    };
    toggle_enabled(false);
    assert!(
        claim(lettuce_conversations::GenerationOperation::Send)
            .expect("disabled global setting admits nothing")
            .is_empty()
    );
    toggle_enabled(true);
    let set_default_model = |model: Option<lettuce_types::ModelProfileId>| {
        let stored = GlobalSettingsStore::load(database).expect("settings");
        GlobalSettingsStore::save(database, stored.settings, model, stored.revision)
            .expect("default model");
    };
    set_default_model(None);
    let unresolved = claim(lettuce_conversations::GenerationOperation::Send)
        .expect("send admits the interval window")
        .into_iter()
        .next()
        .expect("claimed plain memory work");
    let rescheduled = host
        .run_claimed(
            unresolved,
            CancellationReason::User,
            TimestampMillis::new(1_031),
        )
        .await
        .expect("settle the unresolved cycle");
    let crate::CompanionMemorySettledWork::RetryScheduled { error, job } = rescheduled else {
        panic!("a missing summarisation model reschedules the job");
    };
    assert!(matches!(
        error,
        crate::CompanionMemoryJobRunError::RuntimeInputs(
            crate::CompanionMemoryRuntimeInputError::MissingModel
        )
    ));
    assert_eq!(job.state, JobState::Queued);
    assert!(memory.requests.lock().expect("memory requests").is_empty());
    set_default_model(Some(scenario.model.source_id));
    let work = claim(lettuce_conversations::GenerationOperation::Send)
        .expect("the rescheduled window is claimed again")
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

    let (group, _) = group_scenario(
        &backend,
        "host-group",
        lettuce_characters::SpeakerSelection::RoundRobin,
        false,
    );
    assert!(
        host.after_turn(
            group.conversation_id,
            lettuce_conversations::GenerationOperation::Send,
            WorkerId::new(),
            TimestampMillis::new(1_040),
            LEASE,
            &ResourceAvailability::all(),
        )
        .expect("a group conversation resolves its memory mode without a speaker")
        .is_empty()
    );
}

fn finalized_dynamic_turn(backend: &AppBackend, prefix: &str) -> impl Future<Output = Scenario> {
    let database = backend.database();
    let stored = GlobalSettingsStore::load(database).expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.enabled = true;
    settings.dynamic_memory.summary_message_interval = 2;
    GlobalSettingsStore::save(
        database,
        settings,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("save dynamic memory settings");
    let scenario = scenario_with_resolvable_profile(database, true, prefix, true);
    let generation = admit_and_claim(database, &scenario, 1_015);
    async move {
        let engine = ScenarioEmbeddingEngine;
        let reply = scripted(vec![text_outcome("reply", "Tea it is.", 5, 3)]);
        backend
            .prepared_conversation_generation_runner(&engine, &reply)
            .run(
                &generation,
                ConversationGenerationRuntimeInput::default(),
                TimestampMillis::new(1_020),
            )
            .await
            .expect("finalize dynamic turn");
        scenario
    }
}

fn successful_memory_cycle() -> ScriptedInference {
    scripted(vec![
        call_outcome(
            "drive-summary",
            "write_summary",
            serde_json::json!({"summary": "The user chose tea."}),
            (6, 2),
        ),
        call_outcome(
            "drive-create",
            "create_memory",
            serde_json::json!({"text": "The user prefers tea", "category": "preference"}),
            (7, 2),
        ),
    ])
}

fn stored_summary(database: &Database, scenario: &Scenario) -> Option<String> {
    MemorySummaryRepository::get_summary(database, scenario.space_id.expect("memory space"))
        .expect("summary")
        .map(|summary| summary.text)
}

#[tokio::test]
async fn post_turn_memory_driver_runs_the_due_cycle_and_releases_the_conversation() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let scenario = finalized_dynamic_turn(&backend, "driver").await;
    let engine = ScenarioEmbeddingEngine;
    let memory = successful_memory_cycle();
    let host = backend.companion_memory_host(&engine, &memory);
    let scheduler = crate::PostTurnMemoryScheduler::new();
    let clock = FakeClock::new(TimestampMillis::new(1_030));
    assert!(scheduler.enqueue(scenario.conversation_id));
    assert!(!scheduler.enqueue(scenario.conversation_id));
    host.drive(
        &scheduler,
        scenario.conversation_id,
        WorkerId::new(),
        LEASE,
        &clock,
        &crate::CompanionFollowUpHost::new(backend.database(), &memory),
    )
    .await;
    assert!(!scheduler.is_active(scenario.conversation_id));
    assert_eq!(
        stored_summary(backend.database(), &scenario).as_deref(),
        Some("The user chose tea.")
    );
    assert_eq!(memory.requests.lock().expect("memory requests").len(), 2);
}

async fn crash_during_memory_cycle(prefix: &str) -> (std::path::PathBuf, Scenario, JobId) {
    let path = std::env::temp_dir().join(format!("lettuce-{prefix}-{}.db", ConversationId::new()));
    let backend = AppBackend::open(&path, TimestampMillis::new(1)).expect("backend");
    let scenario = finalized_dynamic_turn(&backend, prefix).await;
    let engine = ScenarioEmbeddingEngine;
    let idle = scripted(Vec::new());
    let work = backend
        .companion_memory_host(&engine, &idle)
        .after_turn(
            scenario.conversation_id,
            lettuce_conversations::GenerationOperation::Send,
            WorkerId::new(),
            TimestampMillis::new(1_030),
            LEASE,
            &ResourceAvailability::all(),
        )
        .expect("admit the due window")
        .into_iter()
        .next()
        .expect("claimed memory work");
    let job_id = work.handle.id();
    drop(backend);
    (path, scenario, job_id)
}

#[tokio::test]
async fn restart_resumes_a_memory_job_whose_window_is_still_due() {
    let (path, scenario, job_id) = crash_during_memory_cycle("memory-resume").await;
    let backend = AppBackend::open(&path, TimestampMillis::new(1_040)).expect("reopen");
    backend
        .recover_after_restart(TimestampMillis::new(1_040))
        .expect("release claims");
    assert_eq!(
        persisted_job(backend.database(), job_id).state,
        JobState::Queued
    );
    let engine = ScenarioEmbeddingEngine;
    let memory = successful_memory_cycle();
    let cancelled = backend
        .companion_memory_host(&engine, &memory)
        .resume_after_restart(
            WorkerId::new(),
            LEASE,
            &FakeClock::new(TimestampMillis::new(1_041)),
            &crate::CompanionFollowUpHost::new(backend.database(), &memory),
        )
        .await
        .expect("resume memory jobs");
    assert!(cancelled.is_empty());
    assert_eq!(
        persisted_job(backend.database(), job_id).state,
        JobState::Succeeded
    );
    assert_eq!(
        stored_summary(backend.database(), &scenario).as_deref(),
        Some("The user chose tea.")
    );
    drop(backend);
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn a_memory_job_the_app_keeps_stopping_during_is_not_run_again() {
    let (path, scenario, job_id) = crash_during_memory_cycle("memory-crash-loop").await;
    let engine = ScenarioEmbeddingEngine;
    let backend = AppBackend::open(&path, TimestampMillis::new(1_040)).expect("reopen");
    backend
        .recover_after_restart(TimestampMillis::new(1_040))
        .expect("release claims");
    let idle = scripted(Vec::new());
    let reclaimed = backend
        .companion_memory_host(&engine, &idle)
        .after_turn(
            scenario.conversation_id,
            lettuce_conversations::GenerationOperation::Send,
            WorkerId::new(),
            TimestampMillis::new(1_041),
            LEASE,
            &ResourceAvailability::all(),
        )
        .expect("reclaim the resumed job")
        .into_iter()
        .next()
        .expect("the same job");
    assert_eq!(reclaimed.handle.id(), job_id);
    drop(backend);

    let backend = AppBackend::open(&path, TimestampMillis::new(1_050)).expect("reopen again");
    backend
        .recover_after_restart(TimestampMillis::new(1_050))
        .expect("release claims again");
    let memory = successful_memory_cycle();
    let host = backend.companion_memory_host(&engine, &memory);
    let cancelled = host
        .resume_after_restart(
            WorkerId::new(),
            LEASE,
            &FakeClock::new(TimestampMillis::new(1_051)),
            &crate::CompanionFollowUpHost::new(backend.database(), &memory),
        )
        .await
        .expect("resume memory jobs");
    assert!(cancelled.is_empty());
    assert_eq!(
        persisted_job(backend.database(), job_id).state,
        JobState::Failed
    );
    assert!(memory.requests.lock().expect("memory requests").is_empty());
    assert!(
        host.after_turn(
            scenario.conversation_id,
            lettuce_conversations::GenerationOperation::Send,
            WorkerId::new(),
            TimestampMillis::new(1_052),
            LEASE,
            &ResourceAvailability::all(),
        )
        .expect("the held window admits nothing")
        .is_empty()
    );
    drop(backend);
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn restart_cancels_a_memory_job_its_conversation_would_no_longer_admit() {
    let (path, scenario, job_id) = crash_during_memory_cycle("memory-stale").await;
    let backend = AppBackend::open(&path, TimestampMillis::new(1_040)).expect("reopen");
    backend
        .recover_after_restart(TimestampMillis::new(1_040))
        .expect("release claims");
    let database = backend.database();
    let stored = GlobalSettingsStore::load(database).expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.enabled = false;
    GlobalSettingsStore::save(
        database,
        settings,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("disable dynamic memory");
    let engine = ScenarioEmbeddingEngine;
    let memory = scripted(Vec::new());
    let cancelled = backend
        .companion_memory_host(&engine, &memory)
        .resume_after_restart(
            WorkerId::new(),
            LEASE,
            &FakeClock::new(TimestampMillis::new(1_041)),
            &crate::CompanionFollowUpHost::new(backend.database(), &memory),
        )
        .await
        .expect("resume memory jobs");
    assert_eq!(cancelled, vec![job_id]);
    assert_eq!(persisted_job(database, job_id).state, JobState::Cancelled);
    assert!(memory.requests.lock().expect("memory requests").is_empty());
    assert_eq!(stored_summary(database, &scenario), None);

    let stored = GlobalSettingsStore::load(database).expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.enabled = true;
    GlobalSettingsStore::save(
        database,
        settings,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("enable dynamic memory again");
    let memory = successful_memory_cycle();
    let host = backend.companion_memory_host(&engine, &memory);
    let retried = host
        .after_turn(
            scenario.conversation_id,
            lettuce_conversations::GenerationOperation::Send,
            WorkerId::new(),
            TimestampMillis::new(1_050),
            LEASE,
            &ResourceAvailability::all(),
        )
        .expect("the cancelled window is admitted again")
        .into_iter()
        .next()
        .expect("a fresh job for the same window");
    assert_ne!(retried.handle.id(), job_id);
    host.run_claimed(
        retried,
        CancellationReason::User,
        TimestampMillis::new(1_051),
    )
    .await
    .expect("run the retried window");
    assert_eq!(
        stored_summary(database, &scenario).as_deref(),
        Some("The user chose tea.")
    );
    drop(backend);
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn post_turn_memory_host_answers_ask_first_with_skip_and_trigger() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let database = backend.database();
    let stored = GlobalSettingsStore::load(database).expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.enabled = true;
    settings.dynamic_memory.summary_message_interval = 2;
    settings.dynamic_memory.run_mode = lettuce_settings::MemoryRunMode::AskFirst;
    GlobalSettingsStore::save(
        database,
        settings,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("save dynamic memory settings");
    let scenario = scenario_with_resolvable_profile(database, true, "host-ask-first", true);
    let generation = admit_and_claim(database, &scenario, 1_015);
    let engine = ScenarioEmbeddingEngine;
    let reply = scripted(vec![text_outcome("ask-first-reply", "Tea it is.", 5, 3)]);
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
            "ask-first-summary",
            "write_summary",
            serde_json::json!({"summary": "The user chose tea."}),
            (6, 2),
        ),
        call_outcome(
            "ask-first-create",
            "create_memory",
            serde_json::json!({"text": "The user prefers tea", "category": "preference"}),
            (7, 2),
        ),
    ]);
    let host = backend.companion_memory_host(&engine, &memory);
    let after_turn = || {
        host.after_turn(
            scenario.conversation_id,
            lettuce_conversations::GenerationOperation::Send,
            WorkerId::new(),
            TimestampMillis::new(1_030),
            LEASE,
            &ResourceAvailability::all(),
        )
    };
    assert!(after_turn().expect("ask_first admits nothing").is_empty());
    assert_eq!(
        host.pending_approval_count(scenario.conversation_id)
            .expect("pending approval"),
        Some(2)
    );
    let skipped = host
        .skip(scenario.conversation_id, TimestampMillis::new(1_031))
        .expect("skip the pending approval")
        .expect("a pending approval was skipped");
    assert!(skipped.skipped && !skipped.pending);
    assert_eq!(
        host.pending_approval_count(scenario.conversation_id)
            .expect("pending approval"),
        None
    );
    let trigger = |model| {
        host.trigger(
            scenario.conversation_id,
            model,
            false,
            WorkerId::new(),
            TimestampMillis::new(1_032),
            LEASE,
            &ResourceAvailability::all(),
        )
    };
    let work = trigger(None)
        .expect("trigger admits the recent window")
        .into_iter()
        .next()
        .expect("claimed plain memory work");
    assert_eq!(
        work.admission.batch.window_selection,
        crate::CompanionMemoryWindowSelection::Recent
    );
    assert_eq!(
        work.admission
            .batch
            .source_messages()
            .expect("plain window")
            .len(),
        2
    );
    let settled = host
        .run_claimed(work, CancellationReason::User, TimestampMillis::new(1_033))
        .await
        .expect("run the triggered cycle");
    assert!(matches!(
        settled,
        crate::CompanionMemorySettledWork::Succeeded { .. }
    ));
    let space_id = scenario.space_id.expect("dynamic memory space");
    assert_eq!(
        MemorySummaryRepository::get_summary(database, space_id)
            .expect("summary")
            .expect("stored summary")
            .window_end,
        2
    );
    assert!(
        trigger(Some(scenario.model.source_id))
            .expect("nothing left to summarize")
            .is_empty()
    );

    let stored = GlobalSettingsStore::load(database).expect("settings");
    let mut manual = stored.settings;
    manual.dynamic_memory.run_mode = lettuce_settings::MemoryRunMode::Manual;
    GlobalSettingsStore::save(
        database,
        manual,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("manual run mode");
    assert!(after_turn().expect("manual admits nothing").is_empty());
}

#[tokio::test]
async fn post_turn_memory_host_honors_active_prompt_overrides_of_the_right_purpose() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let database = backend.database();
    let manager_override = seed_prompt(
        database,
        "Custom memory manager",
        PromptPurpose::DynamicMemoryManager,
    );
    let wrong_purpose = seed_prompt(database, "Not a summarizer", PromptPurpose::DirectChat);
    let stored = GlobalSettingsStore::load(database).expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.enabled = true;
    settings.dynamic_memory.summary_message_interval = 2;
    settings.dynamic_memory_prompts.manager_prompt_id = Some(manager_override);
    settings.dynamic_memory_prompts.summarizer_prompt_id = Some(wrong_purpose);
    GlobalSettingsStore::save(
        database,
        settings,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("save dynamic memory settings");
    let scenario = scenario_with_resolvable_profile(database, true, "host-prompts", true);
    let generation = admit_and_claim(database, &scenario, 1_015);
    let engine = ScenarioEmbeddingEngine;
    let reply = scripted(vec![text_outcome("prompt-reply", "Tea it is.", 5, 3)]);
    backend
        .prepared_conversation_generation_runner(&engine, &reply)
        .run(
            &generation,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("finalize plain dynamic turn");
    let memory = scripted(Vec::new());
    let host = backend.companion_memory_host(&engine, &memory);
    let work = host
        .after_turn(
            scenario.conversation_id,
            lettuce_conversations::GenerationOperation::Send,
            WorkerId::new(),
            TimestampMillis::new(1_030),
            LEASE,
            &ResourceAvailability::all(),
        )
        .expect("send admits the interval window")
        .into_iter()
        .next()
        .expect("claimed plain memory work");
    let inputs = host
        .resolve_runtime_inputs(&work.admission)
        .expect("runtime inputs");
    assert_eq!(inputs.memory_prompt.id, manager_override);
    assert_eq!(
        inputs.summary_prompt.id,
        backend
            .built_in_prompt_ids()
            .get(BuiltInPromptId::DynamicSummary)
    );

    let revision = PromptRepository::get(database, manager_override)
        .expect("prompt")
        .expect("override")
        .revision;
    PromptRepository::archive(
        database,
        manager_override,
        revision,
        TimestampMillis::new(1_031),
    )
    .expect("archive the override");
    let inputs = host
        .resolve_runtime_inputs(&work.admission)
        .expect("runtime inputs after archiving");
    assert_eq!(
        inputs.memory_prompt.id,
        backend
            .built_in_prompt_ids()
            .get(BuiltInPromptId::DynamicMemory)
    );
}

#[tokio::test]
async fn reply_helper_drafts_the_next_user_message_from_live_settings() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let database = backend.database();
    let scenario = scenario_with_resolvable_profile(database, false, "reply-helper", true);
    let generation = admit_and_claim(database, &scenario, 1_015);
    let engine = ScenarioEmbeddingEngine;
    let reply = scripted(vec![text_outcome("reply-helper-reply", "Tea it is.", 5, 3)]);
    backend
        .prepared_conversation_generation_runner(&engine, &reply)
        .run(
            &generation,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("finalize the turn");
    let inference = scripted(vec![text_outcome(
        "reply-helper-draft",
        "\"user: Sounds lovely, pour me a cup.\"",
        9,
        4,
    )]);
    let helper = backend.reply_helper(&inference);
    let request = crate::ReplyHelperRequest {
        conversation_id: scenario.conversation_id,
        request_id: RequestId::new(),
        current_draft: Some("pour me".into()),
        swap_places: false,
    };
    let drafted = helper
        .generate(
            &request,
            WorkerId::new(),
            TimestampMillis::new(1_030),
            LEASE,
            &ResourceAvailability::all(),
        )
        .await
        .expect("draft a reply");
    assert_eq!(drafted.text, "Sounds lovely, pour me a cup.");
    assert_eq!(drafted.job.state, JobState::Succeeded);
    assert_eq!(drafted.job.kind, lettuce_jobs::JobKind::CreationRun);
    let sent = {
        let requests = inference.requests.lock().expect("requests");
        assert_eq!(requests.len(), 1);
        requests[0].clone()
    };
    assert!(sent.tools.is_none());
    assert_eq!(sent.stream_sink, Some(request.request_id));
    assert_eq!(sent.profile.chat_profile.parameters.temperature, Some(0.8));
    assert_eq!(sent.profile.chat_profile.parameters.top_p, Some(1.0));
    assert_eq!(
        sent.profile
            .chat_profile
            .parameters
            .visible_max_output_tokens,
        Some(150)
    );
    let texts = sent
        .context
        .messages
        .iter()
        .map(|message| {
            message
                .parts
                .iter()
                .filter_map(|part| match part {
                    ProviderContextPart::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("")
        })
        .collect::<Vec<_>>();
    assert!(texts[0].contains("helping the user write their next message"));
    assert!(texts.iter().any(|text| text.contains("Name: Ada")));
    assert!(
        texts
            .iter()
            .any(|text| text.contains("The user has started writing: \"pour me\""))
    );
    let input = texts.last().expect("runtime input");
    assert!(input.starts_with("Here is the recent conversation:\n\n"));
    assert!(input.contains("user: Remember tea."));
    assert!(input.contains("Ada: Tea it is."));
    assert!(input.ends_with("Generate a reply for user to say next."));
    assert_eq!(
        sent.context.messages.last().expect("input").role,
        MessageRole::User
    );

    let stored = GlobalSettingsStore::load(database).expect("settings");
    let mut settings = stored.settings;
    settings.help_me_reply.enabled = false;
    GlobalSettingsStore::save(
        database,
        settings,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("disable the reply helper");
    let disabled = helper
        .generate(
            &crate::ReplyHelperRequest {
                request_id: RequestId::new(),
                ..request.clone()
            },
            WorkerId::new(),
            TimestampMillis::new(1_031),
            LEASE,
            &ResourceAvailability::all(),
        )
        .await;
    assert!(matches!(disabled, Err(crate::ReplyHelperError::Disabled)));
    assert_eq!(inference.requests.lock().expect("requests").len(), 1);
}

#[tokio::test]
async fn reply_helper_falls_back_to_a_plain_request_when_the_model_cannot_stream() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let database = backend.database();
    let scenario = scenario_with_resolvable_profile(database, false, "reply-helper-plain", true);
    let generation = admit_and_claim(database, &scenario, 1_015);
    let engine = ScenarioEmbeddingEngine;
    let reply = scripted(vec![text_outcome(
        "reply-helper-plain-reply",
        "Tea it is.",
        5,
        3,
    )]);
    backend
        .prepared_conversation_generation_runner(&engine, &reply)
        .run(
            &generation,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("finalize the turn");
    let mut model = ModelProfileRepository::get(database, scenario.model.source_id)
        .expect("model")
        .expect("model exists");
    let revision = model.revision;
    model.config.capabilities.streaming = lettuce_models::CapabilityStatus::Unsupported;
    ModelProfileRepository::upsert(database, model, Some(revision)).expect("non-streaming model");
    let inference = scripted(vec![text_outcome(
        "reply-helper-plain-draft",
        "Sounds lovely.",
        9,
        4,
    )]);
    let helper = backend.reply_helper(&inference);
    let request = crate::ReplyHelperRequest {
        conversation_id: scenario.conversation_id,
        request_id: RequestId::new(),
        current_draft: None,
        swap_places: false,
    };
    let drafted = helper
        .generate(
            &request,
            WorkerId::new(),
            TimestampMillis::new(1_030),
            LEASE,
            &ResourceAvailability::all(),
        )
        .await
        .expect("draft without streaming");
    assert_eq!(drafted.text, "Sounds lovely.");
    assert_eq!(
        inference.requests.lock().expect("requests")[0].stream_sink,
        None
    );
}

#[tokio::test]
async fn reply_helper_never_leaves_an_unclaimable_job_queued() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let inference = scripted(Vec::new());
    let helper = backend.reply_helper(&inference);
    let request = crate::ReplyHelperRequest {
        conversation_id: lettuce_types::ConversationId::new(),
        request_id: RequestId::new(),
        current_draft: None,
        swap_places: false,
    };
    let generate = |allowed: ResourceAvailability| {
        let helper = &helper;
        let request = &request;
        async move {
            helper
                .generate(
                    request,
                    WorkerId::new(),
                    TimestampMillis::new(1_030),
                    LEASE,
                    &allowed,
                )
                .await
        }
    };
    assert!(matches!(
        generate(ResourceAvailability::none()).await,
        Err(crate::ReplyHelperError::NotClaimed)
    ));
    assert!(matches!(
        generate(ResourceAvailability::all()).await,
        Err(crate::ReplyHelperError::AlreadySettled)
    ));
    assert!(inference.requests.lock().expect("requests").is_empty());
}

#[tokio::test]
async fn reply_helper_drafts_a_group_reply_with_the_whole_cast() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let (scenario, _) = group_scenario(
        &backend,
        "reply-helper-group",
        lettuce_characters::SpeakerSelection::RoundRobin,
        false,
    );
    let inference = scripted(vec![text_outcome(
        "group-reply-helper-draft",
        "Shall we all sit down?",
        9,
        4,
    )]);
    let helper = backend.reply_helper(&inference);
    let drafted = helper
        .generate(
            &crate::ReplyHelperRequest {
                conversation_id: scenario.conversation_id,
                request_id: RequestId::new(),
                current_draft: None,
                swap_places: true,
            },
            WorkerId::new(),
            TimestampMillis::new(1_030),
            LEASE,
            &ResourceAvailability::all(),
        )
        .await
        .expect("draft a group reply");
    assert_eq!(drafted.text, "Shall we all sit down?");
    assert_eq!(drafted.job.state, JobState::Succeeded);
    let sent = {
        let requests = inference.requests.lock().expect("requests");
        assert_eq!(requests.len(), 1);
        requests[0].clone()
    };
    let texts = sent
        .context
        .messages
        .iter()
        .map(|message| {
            message
                .parts
                .iter()
                .filter_map(|part| match part {
                    ProviderContextPart::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("")
        })
        .collect::<Vec<_>>();
    let character_entry = texts
        .iter()
        .find(|text| text.starts_with("# The Character You're Talking To"))
        .expect("character entry");
    assert!(character_entry.contains("Ada") && character_entry.contains("Bea"));
    assert!(character_entry.contains("participants in a group conversation"));
    assert!(
        texts
            .iter()
            .any(|text| text.contains("Generate a fresh, detailed response"))
    );
    let input = texts.last().expect("runtime input");
    assert!(input.starts_with("Here is the recent group conversation:\n\n"));
    assert!(input.contains("user: Hello cast."));
    assert!(input.ends_with("Generate a reply for user to say next in this group chat."));
}

fn companion_conversation_with_processing_effect(
    backend: &AppBackend,
    prefix: &str,
) -> lettuce_types::ConversationId {
    let database = backend.database();
    let model_id = seed_model(database, ProviderProtocol::Ollama, prefix);
    let mut model = ModelProfileRepository::get(database, model_id)
        .expect("model")
        .expect("model exists");
    let revision = model.revision;
    model.config.chat_parameters.temperature = None;
    model.config.capabilities.streaming = lettuce_models::CapabilityStatus::Supported;
    model.config.capabilities.tools = lettuce_models::CapabilityStatus::Supported;
    ModelProfileRepository::upsert(database, model, Some(revision)).expect("resolvable model");
    set_application_default_model(database, model_id);
    let character_id = seed_character(database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.interaction_mode = InteractionMode::Companion;
        defaults.memory_policy = MemoryPolicy::Dynamic;
        defaults.companion_soul = Some(lettuce_companions::CompanionSoulConfig::default());
    });
    let launched = ConversationLaunchPlanner::new(database)
        .launch_direct(&request(character_id, &format!("{prefix}-launch")), NOW)
        .expect("launch companion");
    let sent = CompanionTurnCoordinator::<_, ScenarioEmotionEngine>::new(database, None)
        .begin_send(
            &direct_send_command(
                &launched.value.conversation,
                &format!("{prefix}-send"),
                "I missed you.",
            ),
            TimestampMillis::new(NOW.get() + 1),
            &CancellationToken::new(),
        )
        .expect("send companion message");
    let turn_id = sent.value.turn.id;
    let attempt_id = sent.value.attempt.id;
    let mut turn = sent.value.turn;
    let operation = |value: &str| OperationToken {
        key: key(value),
        request_digest: ContentHash::parse("ef".repeat(32)).expect("digest"),
    };
    for (sequence, status) in [
        GenerationTurnStatus::Preparing,
        GenerationTurnStatus::ContextPrepared,
        GenerationTurnStatus::Running,
    ]
    .into_iter()
    .enumerate()
    {
        turn = database
            .append_event(
                turn_id,
                turn.revision,
                &operation(&format!("{prefix}-stage-{sequence}")),
                GenerationCheckpointEnvelope {
                    turn_id,
                    attempt_id,
                    job_id: None,
                    correlation_id: None,
                    sequence: u64::try_from(sequence + 1).expect("sequence"),
                    event: GenerationCheckpointEvent::Stage { status },
                },
                TimestampMillis::new(NOW.get() + 2 + i64::try_from(sequence).expect("time")),
            )
            .expect("advance companion turn")
            .value;
    }
    let conversation = ConversationReader::get(database, launched.value.conversation.id)
        .expect("current conversation")
        .conversation;
    let ConversationKind::Direct(details) = &conversation.kind else {
        panic!("expected direct conversation");
    };
    let model = match &details.model {
        SnapshotSelection::Inherited(model) | SnapshotSelection::Explicit(model) => model.clone(),
        SnapshotSelection::Disabled => panic!("expected resolved model"),
    };
    database
        .finalize_generation(
            turn_id,
            attempt_id,
            conversation.revision,
            turn.revision,
            &operation(&format!("{prefix}-finalize")),
            FinalizationDraft {
                parts: vec![MessagePart::Text {
                    text: "I missed you too.".into(),
                }],
                ordinal: 0,
                model,
                replay: None,
                outcome: GenerationCheckpointEvent::Completed,
            },
            UsageEventId::new(),
            TimestampMillis::new(NOW.get() + 10),
        )
        .expect("finalize assistant");
    conversation.id
}

#[tokio::test]
async fn post_turn_memory_host_admits_only_this_companion_conversation_effects() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let database = backend.database();
    let stored = GlobalSettingsStore::load(database).expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.enabled = true;
    settings.dynamic_memory.summary_message_interval = 2;
    GlobalSettingsStore::save(
        database,
        settings,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("save dynamic memory settings");
    let first = companion_conversation_with_processing_effect(&backend, "companion-host-a");
    let second = companion_conversation_with_processing_effect(&backend, "companion-host-b");
    assert_eq!(
        CompanionTurnEffectRepository::list_processing(database, 512)
            .expect("processing effects")
            .len(),
        2
    );
    let engine = ScenarioEmbeddingEngine;
    let memory = scripted(Vec::new());
    let host = backend.companion_memory_host(&engine, &memory);
    let work = host
        .after_turn(
            first,
            lettuce_conversations::GenerationOperation::Send,
            WorkerId::new(),
            TimestampMillis::new(1_030),
            LEASE,
            &ResourceAvailability::all(),
        )
        .expect("companion after_turn");
    assert_eq!(work.len(), 1);
    assert_eq!(work[0].admission.batch.conversation_id, first);
    let effects = work[0].admission.batch.effects();
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].conversation_id, first);
    let inputs = host
        .resolve_runtime_inputs(&work[0].admission)
        .expect("runtime inputs for a companion conversation");
    assert!(inputs.supersession_enabled);
    assert!(!inputs.time_awareness_enabled);
    {
        use lettuce_conversations::ConversationRepository as _;
        database
            .update_settings(
                lettuce_conversations::PreparedConversationSettingsUpdate::new(
                    lettuce_conversations::UpdateConversationSettings {
                        conversation_id: first,
                        expected_settings_revision: None,
                        operation: lettuce_conversations::OperationToken {
                            key: lettuce_jobs::IdempotencyKey::new("companion-host-clock")
                                .expect("key"),
                            request_digest: lettuce_types::ContentHash::parse("ab".repeat(32))
                                .expect("digest"),
                        },
                        patch: lettuce_conversations::CurrentConversationSettingsPatch {
                            companion_clock: lettuce_conversations::PatchValue::Set(
                                lettuce_conversations::CompanionClockSettings {
                                    time_awareness_enabled: true,
                                    time_override:
                                        lettuce_conversations::CompanionTimeOverride::Live,
                                },
                            ),
                            ..lettuce_conversations::CurrentConversationSettingsPatch::default()
                        },
                    },
                    Vec::new(),
                )
                .expect("prepared clock settings"),
                TimestampMillis::new(1_031),
            )
            .expect("enable companion time awareness");
    }
    assert!(
        host.resolve_runtime_inputs(&work[0].admission)
            .expect("time-aware runtime inputs")
            .time_awareness_enabled
    );
    assert!(
        CompanionTurnEffectRepository::list_processing(database, 512)
            .expect("processing effects")
            .iter()
            .any(|effect| effect.conversation_id == second),
        "the other companion conversation's effect stays untouched"
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
async fn cancellation_without_streamed_text_retains_usage_and_settles_cancelled() {
    let database = database();
    let scenario = scenario(&database, false, "cancel-late");
    let work = admit_and_claim(&database, &scenario, 1_015);
    let inference = BlockingInference::new(text_outcome("blocking", "  ", 9, 4));
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
        prompt_cache_key: None,
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

#[tokio::test]
async fn time_aware_generation_strips_echoed_time_stamps_before_finalizing() {
    let database = database();
    let scenario = scenario(&database, false, "strip-stamps");
    let work = admit_and_claim(&database, &scenario, 1_015);
    let inference = scripted(vec![text_outcome(
        "strip-stamps-response",
        "<time>2026-03-12 18:00</time> Hey, you're up late.",
        9,
        3,
    )]);
    let mut stamped_input = input(&scenario);
    stamped_input.strip_time_stamps = true;
    let result = ConversationGenerationJobRunner::new(&database, &inference)
        .run(&work, stamped_input, TimestampMillis::new(1_020))
        .await
        .expect("run with time awareness");
    assert_eq!(
        result.candidate.parts,
        vec![lettuce_conversations::MessagePart::Text {
            text: "Hey, you're up late.".into(),
        }]
    );
}

#[tokio::test]
async fn direct_replies_hand_back_their_scene_tag_before_finalizing() {
    let database = database();
    let scenario = scenario(&database, false, "scene-tag");
    let work = admit_and_claim(&database, &scenario, 1_015);
    let inference = scripted(vec![text_outcome(
        "scene-tag-response",
        "She smiles. <img>a harbor at dusk</img>",
        9,
        3,
    )]);
    let mut tagged_input = input(&scenario);
    tagged_input.reply_images = Some(crate::ReplyImageFacts {
        scene_mode: Some(lettuce_settings::SceneGenerationMode::AskFirst),
    });
    let result = ConversationGenerationJobRunner::new(&database, &inference)
        .run(&work, tagged_input, TimestampMillis::new(1_020))
        .await
        .expect("run with a scene tag");
    assert_eq!(
        result.candidate.parts,
        vec![lettuce_conversations::MessagePart::Text {
            text: "She smiles.".into(),
        }]
    );
    assert_eq!(
        result.scene_image,
        Some(crate::SceneImageFollowUp {
            prompt: "a harbor at dusk".into(),
            ask_first: true,
        })
    );
}

#[tokio::test]
async fn a_synced_delete_moves_the_head_so_later_messages_attach_on_both_devices() {
    let a = database();
    let b = database();
    let scenario = scenario(&a, false, "synced-delete-head");
    let work = admit_and_claim(&a, &scenario, 1_015);
    let inference = scripted(vec![text_outcome("synced-delete", "Deleted reply", 20, 5)]);
    let result = ConversationGenerationJobRunner::new(&a, &inference)
        .run(&work, input(&scenario), TimestampMillis::new(1_020))
        .await
        .expect("run");
    sync_prompts(&a, &b, 2_000);
    let token = |name: &str| OperationToken {
        key: key(name),
        request_digest: ContentHash::parse("ed".repeat(32)).expect("digest"),
    };
    let revision = |database: &Database| {
        ConversationReader::get(database, scenario.conversation_id)
            .expect("conversation")
            .conversation
            .revision
    };
    lettuce_conversations::ConversationRepository::tombstone_message(
        &a,
        &lettuce_conversations::TombstoneMessage {
            conversation_id: scenario.conversation_id,
            message_id: result.candidate.message_id,
            expected_revision: revision(&a),
            operation: token("synced-delete-reply"),
            descendants: lettuce_conversations::DescendantPolicy::Preserve,
        },
        TimestampMillis::new(2_100),
    )
    .expect("delete the reply on a");
    sync_prompts(&a, &b, 2_200);
    let head = |database: &Database| {
        let aggregate =
            ConversationReader::get(database, scenario.conversation_id).expect("conversation");
        aggregate
            .branches
            .iter()
            .find(|branch| branch.id == aggregate.conversation.active_branch_id)
            .expect("active branch")
            .head_message_id
    };
    assert_eq!(
        head(&b),
        head(&a),
        "legacy deleted the reply on every device"
    );
    assert_ne!(head(&b), Some(result.candidate.message_id));
    let aggregate = ConversationReader::get(&a, scenario.conversation_id).expect("a");
    let user = aggregate
        .conversation
        .participants
        .iter()
        .find(|participant| participant.role == ParticipantRole::User)
        .expect("user participant")
        .id;
    let sent = lettuce_conversations::ConversationRepository::begin_send(
        &a,
        &lettuce_conversations::SendConversation {
            conversation_id: scenario.conversation_id,
            branch_id: aggregate.conversation.active_branch_id,
            expected_revision: revision(&a),
            operation: token("synced-delete-next"),
            message: lettuce_conversations::MessageDraft {
                role: MessageRole::User,
                author_participant_id: Some(user),
                parts: vec![MessagePart::Text {
                    text: "After the delete".into(),
                }],
                visibility: MessageVisibility::Visible,
                pinned: false,
                scene_edited: false,
            },
            swap_roles: false,
        },
        TimestampMillis::new(2_300),
    )
    .expect("send after the delete on a");
    let next = scripted(vec![text_outcome("synced-next", "Next reply", 20, 5)]);
    let next_scenario = Scenario {
        conversation_id: scenario.conversation_id,
        turn_id: sent.value.turn.id,
        attempt_id: sent.value.attempt.id,
        model: scenario.model.clone(),
        profile: scenario.profile.clone(),
        space_id: scenario.space_id,
    };
    let next_work = admit_and_claim(&a, &next_scenario, 2_310);
    ConversationGenerationJobRunner::new(&a, &next)
        .run(
            &next_work,
            input(&next_scenario),
            TimestampMillis::new(2_320),
        )
        .await
        .expect("next reply on a");
    sync_prompts(&a, &b, 2_400);
    assert_eq!(
        head(&b),
        head(&a),
        "the next message extends the retreated head on b"
    );
}

#[tokio::test]
async fn generated_messages_sync_with_their_turns_and_usage() {
    let a = database();
    let b = database();
    let scenario = scenario(&a, false, "synced-generation");
    let work = admit_and_claim(&a, &scenario, 1_015);
    let inference = scripted(vec![text_outcome("synced-1", "Synced reply", 20, 5)]);
    let result = ConversationGenerationJobRunner::new(&a, &inference)
        .run(&work, input(&scenario), TimestampMillis::new(1_020))
        .await
        .expect("run");
    assert_eq!(result.turn.status, GenerationTurnStatus::Succeeded);
    let event = UsageLedger::get(&a, result.usage_event_id)
        .expect("usage")
        .expect("usage exists");
    let basis = lettuce_usage::UsageCostBasis {
        openrouter: None,
        model_profile_id: event.record.model_profile_id.expect("model"),
        provider_account_id: event.record.provider_account_id.expect("provider"),
        source: "OpenRouter endpoint snapshot".into(),
        captured_at: TimestampMillis::new(1_030),
        pricing: lettuce_usage::ModelPricing {
            prompt: "0.001".into(),
            completion: "0.002".into(),
            request: String::new(),
            image: String::new(),
            image_output: String::new(),
            web_search: String::new(),
            internal_reasoning: String::new(),
            input_cache_read: String::new(),
            input_cache_write: String::new(),
        },
        input: lettuce_usage::OpenRouterCostInput {
            prompt_tokens: 20,
            completion_tokens: 5,
            ..Default::default()
        },
    };
    let cost = lettuce_usage::UsageCostLedger::record_cost(&a, event.id, basis).expect("cost");

    sync_prompts(&a, &b, 2_000);

    assert_eq!(
        lettuce_usage::UsageCostLedger::get_cost(&b, event.id)
            .expect("b cost")
            .map(|stored| stored.basis),
        Some(cost.basis)
    );
    assert_eq!(
        b.job_usage(work.handle.id()).expect("b job usage"),
        a.job_usage(work.handle.id()).expect("a job usage")
    );
    assert!(
        !a.job_usage(work.handle.id())
            .expect("a job usage")
            .is_empty()
    );

    let conversation = ConversationReader::get(&a, scenario.conversation_id).expect("a");
    let timeline = |database: &Database| {
        ConversationReader::timeline_page(
            database,
            scenario.conversation_id,
            conversation.conversation.active_branch_id,
            &lettuce_types::PageRequest::default(),
        )
        .expect("timeline")
        .items
    };
    let on_a = timeline(&a);
    assert!(on_a.len() >= 2);
    assert_eq!(timeline(&b), on_a);
    assert_eq!(
        ConversationReader::get_turn(&b, scenario.turn_id).expect("b turn"),
        ConversationReader::get_turn(&a, scenario.turn_id).expect("a turn")
    );
    assert_eq!(
        UsageLedger::get(&b, result.usage_event_id).expect("b usage"),
        UsageLedger::get(&a, result.usage_event_id).expect("a usage")
    );

    let reply = on_a.last().expect("reply").message.id;
    lettuce_conversations::ConversationRepository::edit_message(
        &b,
        &lettuce_conversations::EditMessage {
            conversation_id: scenario.conversation_id,
            message_id: reply,
            expected_revision: ConversationReader::get(&b, scenario.conversation_id)
                .expect("b conversation")
                .conversation
                .revision,
            operation: OperationToken {
                key: key("synced-edit"),
                request_digest: ContentHash::parse("ef".repeat(32)).expect("digest"),
            },
            draft: lettuce_conversations::MessageEditDraft {
                parts: vec![MessagePart::Text {
                    text: "Edited on b".into(),
                }],
                visibility: MessageVisibility::Visible,
                pinned: true,
                scene_edited: false,
            },
        },
        TimestampMillis::new(2_500),
    )
    .expect("edit on b");
    sync_prompts(&b, &a, 3_000);
    let shared = |database: &Database| {
        timeline(database)
            .into_iter()
            .map(|mut item| {
                item.message.revision = Revision::INITIAL;
                item.message.updated_at = item.message.created_at;
                if let Some(revision) = &mut item.active_revision {
                    revision.sequence = Revision::INITIAL;
                }
                item
            })
            .collect::<Vec<_>>()
    };
    let edited = shared(&a);
    assert_eq!(edited, shared(&b));
    assert!(edited.last().expect("reply").message.pinned);
    assert_eq!(
        edited
            .last()
            .expect("reply")
            .active_revision
            .as_ref()
            .map(|revision| &revision.parts),
        Some(&vec![MessagePart::Text {
            text: "Edited on b".into()
        }])
    );
    let edit = |database: &Database, operation: &str, text: &str, at: i64| {
        lettuce_conversations::ConversationRepository::edit_message(
            database,
            &lettuce_conversations::EditMessage {
                conversation_id: scenario.conversation_id,
                message_id: reply,
                expected_revision: ConversationReader::get(database, scenario.conversation_id)
                    .expect("conversation")
                    .conversation
                    .revision,
                operation: OperationToken {
                    key: key(operation),
                    request_digest: ContentHash::parse("ef".repeat(32)).expect("digest"),
                },
                draft: lettuce_conversations::MessageEditDraft {
                    parts: vec![MessagePart::Text { text: text.into() }],
                    visibility: MessageVisibility::Visible,
                    pinned: false,
                    scene_edited: false,
                },
            },
            TimestampMillis::new(at),
        )
        .expect("concurrent edit");
    };
    edit(&a, "concurrent-a", "From a", 3_100);
    edit(&b, "concurrent-b", "From b", 3_200);
    sync_prompts(&a, &b, 3_300);
    sync_prompts(&b, &a, 3_400);
    sync_prompts(&a, &b, 3_500);
    let merged = shared(&a);
    assert_eq!(merged, shared(&b));
    assert_eq!(
        ConversationReader::page_message_revisions(
            &a,
            reply,
            &lettuce_types::PageRequest::default(),
        )
        .expect("revisions")
        .items
        .len(),
        4
    );
    {
        use lettuce_sync::LocalChangeJournal;
        assert_eq!(
            a.journal_current_state(TimestampMillis::new(4_000))
                .expect("a rescan"),
            0
        );
        assert_eq!(
            b.journal_current_state(TimestampMillis::new(4_000))
                .expect("b rescan"),
            0
        );
    }
}

async fn send_and_generate(
    database: &Database,
    scenario: &Scenario,
    prefix: &str,
    reply: &str,
    at: i64,
) -> GenerationTurnId {
    let conversation = ConversationReader::get(database, scenario.conversation_id)
        .expect("conversation")
        .conversation;
    let sent = database
        .begin_send(
            &direct_send_command(&conversation, &format!("{prefix}-send"), prefix),
            TimestampMillis::new(at),
        )
        .expect("send")
        .value;
    let turn = Scenario {
        conversation_id: scenario.conversation_id,
        turn_id: sent.turn.id,
        attempt_id: sent.attempt.id,
        model: scenario.model.clone(),
        profile: scenario.profile.clone(),
        space_id: scenario.space_id,
    };
    let work = admit_and_claim(database, &turn, at + 1);
    let inference = scripted(vec![text_outcome(prefix, reply, 10, 5)]);
    let result = ConversationGenerationJobRunner::new(database, &inference)
        .run(&work, input(&turn), TimestampMillis::new(at + 5))
        .await
        .expect("run");
    assert_eq!(result.turn.status, GenerationTurnStatus::Succeeded);
    turn.turn_id
}

fn branch_timeline(
    database: &Database,
    conversation_id: ConversationId,
    branch_id: lettuce_types::ConversationBranchId,
) -> Vec<lettuce_conversations::TimelineItem> {
    ConversationReader::timeline_page(
        database,
        conversation_id,
        branch_id,
        &lettuce_types::PageRequest::default(),
    )
    .expect("timeline")
    .items
    .into_iter()
    .map(|mut item| {
        item.message.revision = Revision::INITIAL;
        item.message.updated_at = item.message.created_at;
        item
    })
    .collect()
}

fn assert_rescans_are_empty(databases: &[&Database], at: i64) {
    use lettuce_sync::LocalChangeJournal;
    for database in databases {
        assert_eq!(
            database
                .journal_current_state(TimestampMillis::new(at))
                .expect("rescan"),
            0
        );
    }
}

#[tokio::test]
async fn forked_branches_sync_with_their_messages() {
    let a = database();
    let b = database();
    let scenario = scenario(&a, false, "synced-fork");
    let work = admit_and_claim(&a, &scenario, 1_015);
    ConversationGenerationJobRunner::new(
        &a,
        &scripted(vec![text_outcome("fork-0", "First", 10, 5)]),
    )
    .run(&work, input(&scenario), TimestampMillis::new(1_020))
    .await
    .expect("run");
    let conversation = ConversationReader::get(&a, scenario.conversation_id)
        .expect("conversation")
        .conversation;
    let root = conversation.active_branch_id;
    let user_message = branch_timeline(&a, scenario.conversation_id, root)
        .into_iter()
        .find(|item| item.message.role == MessageRole::User)
        .expect("user message")
        .message
        .id;
    let fork = a
        .fork_branch(
            &lettuce_conversations::ForkBranch {
                conversation_id: scenario.conversation_id,
                source_branch_id: root,
                at_message_id: Some(user_message),
                expected_revision: conversation.revision,
                operation: OperationToken {
                    key: key("synced-fork-branch"),
                    request_digest: ContentHash::parse("fa".repeat(32)).expect("digest"),
                },
            },
            TimestampMillis::new(1_100),
        )
        .expect("fork")
        .value
        .branch
        .id;
    send_and_generate(&a, &scenario, "on-fork", "Fork reply", 1_200).await;

    sync_prompts(&a, &b, 2_000);

    let on_a = branch_timeline(&a, scenario.conversation_id, fork);
    assert!(on_a.len() >= 3);
    assert_eq!(branch_timeline(&b, scenario.conversation_id, fork), on_a);
    assert_eq!(
        branch_timeline(&b, scenario.conversation_id, root),
        branch_timeline(&a, scenario.conversation_id, root)
    );
    assert_rescans_are_empty(&[&a, &b], 3_000);
}

#[tokio::test]
async fn concurrent_replies_fork_into_a_branch_and_notify_both_devices() {
    use lettuce_sync::ConversationForkRepository;
    let a = database();
    let b = database();
    let scenario = scenario(&a, false, "synced-concurrent");
    let work = admit_and_claim(&a, &scenario, 1_015);
    ConversationGenerationJobRunner::new(&a, &scripted(vec![text_outcome("c-0", "First", 10, 5)]))
        .run(&work, input(&scenario), TimestampMillis::new(1_020))
        .await
        .expect("run");
    sync_prompts(&a, &b, 1_500);

    let turn_a = send_and_generate(&a, &scenario, "from-a", "Reply to a", 2_000).await;
    let turn_b = send_and_generate(&b, &scenario, "from-b", "Reply to b", 2_100).await;
    sync_prompts(&a, &b, 3_000);
    sync_prompts(&b, &a, 3_100);
    sync_prompts(&a, &b, 3_200);

    let root = ConversationReader::get(&a, scenario.conversation_id)
        .expect("conversation")
        .conversation
        .active_branch_id;
    let path = branch_timeline(&a, scenario.conversation_id, root);
    assert_eq!(path, branch_timeline(&b, scenario.conversation_id, root));
    assert_eq!(path.len(), 4);
    let forks_a = a.unresolved_conversation_forks(10).expect("a forks");
    let forks_b = b.unresolved_conversation_forks(10).expect("b forks");
    assert_eq!(forks_a.len(), 1);
    assert_eq!(forks_b.len(), 1);
    assert_eq!(forks_a[0].branch_id, forks_b[0].branch_id);
    assert_ne!(forks_a[0].holds_local, forks_b[0].holds_local);
    let fork = forks_a[0].branch_id;
    let forked = branch_timeline(&a, scenario.conversation_id, fork);
    assert_eq!(forked, branch_timeline(&b, scenario.conversation_id, fork));
    assert_eq!(forked.len(), 4);
    let shown = |items: &[lettuce_conversations::TimelineItem]| {
        items
            .iter()
            .filter_map(|item| {
                item.active_revision
                    .as_ref()
                    .map(|revision| revision.parts.clone())
                    .or_else(|| {
                        item.active_candidate
                            .as_ref()
                            .map(|candidate| candidate.parts.clone())
                    })
            })
            .collect::<Vec<_>>()
    };
    let mut replies = shown(&path)[..2]
        .iter()
        .chain(shown(&forked)[..2].iter())
        .cloned()
        .collect::<Vec<_>>();
    replies.sort_by_key(|parts| format!("{parts:?}"));
    assert_eq!(
        replies,
        vec![
            vec![MessagePart::Text {
                text: "Reply to a".into()
            }],
            vec![MessagePart::Text {
                text: "Reply to b".into()
            }],
            vec![MessagePart::Text {
                text: "from-a".into()
            }],
            vec![MessagePart::Text {
                text: "from-b".into()
            }],
        ]
    );
    let (loser, loser_turn) = if forks_a[0].holds_local {
        (&a, turn_a)
    } else {
        (&b, turn_b)
    };
    let GenerationInput::UserMessage { message_id } =
        ConversationReader::get_turn(loser, loser_turn)
            .expect("turn")
            .input
    else {
        panic!("send turn");
    };
    lettuce_conversations::ConversationRepository::edit_message(
        loser,
        &lettuce_conversations::EditMessage {
            conversation_id: scenario.conversation_id,
            message_id,
            expected_revision: ConversationReader::get(loser, scenario.conversation_id)
                .expect("conversation")
                .conversation
                .revision,
            operation: OperationToken {
                key: key("edit-moved-original"),
                request_digest: ContentHash::parse("ee".repeat(32)).expect("digest"),
            },
            draft: lettuce_conversations::MessageEditDraft {
                parts: vec![MessagePart::Text {
                    text: "Edited after the fork".into(),
                }],
                visibility: MessageVisibility::Visible,
                pinned: false,
                scene_edited: false,
            },
        },
        TimestampMillis::new(3_300),
    )
    .expect("edit moved original");
    sync_prompts(&a, &b, 3_400);
    sync_prompts(&b, &a, 3_500);
    sync_prompts(&a, &b, 3_600);
    let refreshed = branch_timeline(&a, scenario.conversation_id, fork);
    assert_eq!(
        refreshed,
        branch_timeline(&b, scenario.conversation_id, fork)
    );
    assert!(shown(&refreshed).contains(&vec![MessagePart::Text {
        text: "Edited after the fork".into()
    }]));
    a.resolve_conversation_fork(scenario.conversation_id, fork, TimestampMillis::new(4_000))
        .expect("resolve");
    assert!(
        a.unresolved_conversation_forks(10)
            .expect("a forks")
            .is_empty()
    );
    assert_rescans_are_empty(&[&a, &b], 5_000);
}

#[tokio::test]
async fn memory_spaces_sync_their_items_and_summary_under_their_owner() {
    let a = database();
    let b = database();
    let scenario = scenario(&a, false, "synced-memory");
    let work = admit_and_claim(&a, &scenario, 1_015);
    ConversationGenerationJobRunner::new(
        &a,
        &scripted(vec![text_outcome("m-0", "Tea noted", 10, 5)]),
    )
    .run(&work, input(&scenario), TimestampMillis::new(1_020))
    .await
    .expect("run");
    let space = MemoryRepository::get_for_conversation(&a, scenario.conversation_id)
        .expect("space")
        .expect("space exists");
    let item = MemoryItem {
        id: MemoryId::new(),
        short_id: lettuce_memory::MemoryShortId::new(4242).expect("short id"),
        text: "Mira drinks jasmine tea.".into(),
        category: MemoryCategory::Preference,
        source_message_id: None,
        source_role: None,
        observed_at: None,
        observed_time_precision: None,
        superseded_by: None,
        superseded_at: None,
        supersedes: vec![],
        token_count: 5,
        is_cold: false,
        is_pinned: true,
        importance: Score::FULL,
        persistence_importance: Score::FULL,
        prompt_importance: Score::FULL,
        volatility: Score::LEGACY_VOLATILITY,
        access_count: 0,
        created_at: TimestampMillis::new(1_030),
        last_accessed_at: TimestampMillis::new(1_030),
    };
    let space = MemoryRepository::compare_and_apply(
        &a,
        MemoryChangeSet {
            space_id: space.id,
            expected_revision: space.revision,
            items: vec![item.clone()],
        },
    )
    .expect("memory on a");
    let sources = ConversationReader::timeline_page(
        &a,
        scenario.conversation_id,
        ConversationReader::get(&a, scenario.conversation_id)
            .expect("conversation")
            .conversation
            .active_branch_id,
        &lettuce_types::PageRequest::default(),
    )
    .expect("timeline")
    .items
    .into_iter()
    .rev()
    .map(|item| item.message.id)
    .collect::<Vec<_>>();
    lettuce_memory::MemorySummaryRepository::compare_and_apply_summary(
        &a,
        lettuce_memory::MemorySummaryChange {
            expected_revision: space.revision,
            summary: lettuce_memory::MemorySummary {
                space_id: space.id,
                text: "They talked about tea.".into(),
                token_count: 5,
                window_start: 0,
                window_end: 2,
                source_message_ids: sources,
                updated_at: TimestampMillis::new(1_040),
            },
        },
    )
    .expect("summary on a");

    sync_prompts(&a, &b, 2_000);

    let on_b = MemoryRepository::get_for_conversation(&b, scenario.conversation_id)
        .expect("b space")
        .expect("b space exists");
    assert_ne!(on_b.id, space.id);
    assert_eq!(
        on_b.items
            .iter()
            .map(|stored| MemoryItem {
                short_id: item.short_id,
                ..stored.clone()
            })
            .collect::<Vec<_>>(),
        vec![item.clone()]
    );
    let summary_b = lettuce_memory::MemorySummaryRepository::get_summary(&b, on_b.id)
        .expect("b summary")
        .expect("b summary exists");
    assert_eq!(summary_b.text, "They talked about tea.");
    assert_eq!(
        lettuce_memory::MemorySummaryRepository::summary_cursor(
            &b,
            on_b.id,
            scenario.conversation_id
        )
        .expect("cursor"),
        2
    );

    let edited = MemoryItem {
        text: "Mira drinks oolong now.".into(),
        ..item
    };
    MemoryRepository::compare_and_apply(
        &b,
        MemoryChangeSet {
            space_id: on_b.id,
            expected_revision: MemoryRepository::get(&b, on_b.id)
                .expect("b space")
                .expect("present")
                .revision,
            items: vec![edited.clone()],
        },
    )
    .expect("memory on b");
    sync_prompts(&b, &a, 3_000);
    assert_eq!(
        MemoryRepository::get(&a, space.id)
            .expect("a space")
            .expect("present")
            .items,
        vec![edited.clone()]
    );

    let add = |database: &Database, space_id: MemorySpaceId, text: &str| {
        let current = MemoryRepository::get(database, space_id)
            .expect("space")
            .expect("present");
        let id = MemoryId::new();
        let mut items = current.items.clone();
        items.push(MemoryItem {
            id,
            short_id: lettuce_memory::MemoryShortId::derived(id),
            text: text.into(),
            ..edited.clone()
        });
        MemoryRepository::compare_and_apply(
            database,
            MemoryChangeSet {
                space_id,
                expected_revision: current.revision,
                items,
            },
        )
        .expect("add memory");
        id
    };
    let from_a = add(&a, space.id, "Added on a.");
    let from_b = add(&b, on_b.id, "Added on b.");
    sync_prompts(&a, &b, 3_100);
    sync_prompts(&b, &a, 3_200);
    let ids = |database: &Database, space_id: MemorySpaceId| {
        let mut ids = MemoryRepository::get(database, space_id)
            .expect("space")
            .expect("present")
            .items
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>();
        ids.sort();
        ids
    };
    let mut expected = vec![edited.id, from_a, from_b];
    expected.sort();
    assert_eq!(ids(&a, space.id), expected);
    assert_eq!(ids(&b, on_b.id), expected);

    let current = MemoryRepository::get(&a, space.id)
        .expect("space")
        .expect("present");
    MemoryRepository::compare_and_apply(
        &a,
        MemoryChangeSet {
            space_id: space.id,
            expected_revision: current.revision,
            items: current
                .items
                .into_iter()
                .filter(|item| item.id != from_b)
                .collect(),
        },
    )
    .expect("delete on a");
    sync_prompts(&a, &b, 3_300);
    expected.retain(|id| *id != from_b);
    assert_eq!(ids(&b, on_b.id), expected);
    assert_rescans_are_empty(&[&a, &b], 4_000);
}

#[tokio::test]
async fn retrieval_embeds_memories_without_a_current_vector_first() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let stored_settings = GlobalSettingsStore::load(backend.database()).expect("settings");
    let mut settings = stored_settings.settings;
    settings.dynamic_memory.enabled = true;
    settings.dynamic_memory.retrieval_limit = 1;
    GlobalSettingsStore::save(
        backend.database(),
        settings,
        stored_settings.default_model_profile_id,
        stored_settings.revision,
    )
    .expect("save dynamic memory settings");
    let scenario =
        scenario_with_resolvable_profile(backend.database(), true, "pending-embeddings", true);
    let space_id = scenario.space_id.expect("dynamic memory space");
    let stored = MemoryRepository::get(backend.database(), space_id)
        .expect("memory")
        .expect("memory exists");
    let memory_id = MemoryId::new();
    let synced = MemoryItem {
        id: memory_id,
        short_id: lettuce_memory::MemoryShortId::derived(memory_id),
        text: "Mira arrived from another device.".into(),
        category: MemoryCategory::Preference,
        source_message_id: None,
        source_role: None,
        observed_at: None,
        observed_time_precision: None,
        superseded_by: None,
        superseded_at: None,
        supersedes: vec![],
        token_count: 5,
        is_cold: true,
        is_pinned: false,
        importance: Score::from_basis_points(3_000).expect("score"),
        persistence_importance: Score::from_basis_points(8_000).expect("score"),
        prompt_importance: Score::from_basis_points(8_000).expect("score"),
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
            items: vec![synced.clone()],
        },
    )
    .expect("seed memory");
    let work = admit_and_claim(backend.database(), &scenario, 1_015);
    let inference = scripted(vec![]);
    let engine = ScenarioEmbeddingEngine;
    backend
        .prepared_conversation_generation_runner(&engine, &inference)
        .build_input(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("build");
    let ready = MemoryEmbeddingRepository::list_ready(
        backend.database(),
        space_id,
        "scenario-v1",
        EmbeddingDimensions::D128,
    )
    .expect("projections");
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].memory_id, memory_id);
    assert_eq!(ready[0].source_text, synced.text);
}

fn hard_delete_root(prefix: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("lettuce-{prefix}-{}", ConversationId::new()));
    std::fs::create_dir_all(root.join("private-persistent-v2").join("databases")).expect("root");
    root
}

fn hard_delete_path(root: &std::path::Path, name: &str) -> std::path::PathBuf {
    root.join("private-persistent-v2")
        .join("databases")
        .join(name)
}

fn hard_delete_location(root: &std::path::Path) -> crate::AppDatabaseLocation {
    use lettuce_platform::{DirectorySnapshot, FilesystemAuthority};
    let snapshot = DirectorySnapshot::new(root).expect("directory snapshot");
    let authority = FilesystemAuthority::new(&snapshot).expect("filesystem authority");
    crate::AppDatabaseLocation::new(root.join("private-persistent-v2"), &authority)
        .expect("database location")
}

fn hard_delete_media(
    database_path: &std::path::Path,
    root: &std::path::Path,
) -> lettuce_media::LocalMediaBlobStore<Database, Database> {
    use lettuce_platform::{DirectorySnapshot, FilesystemAuthority, ManagedRoot};
    let snapshot = DirectorySnapshot::new(root).expect("directory snapshot");
    let authority = FilesystemAuthority::new(&snapshot).expect("filesystem authority");
    lettuce_media::LocalMediaBlobStore::new(
        authority.managed_files(),
        authority
            .read_capability(ManagedRoot::MediaBlobs)
            .expect("read capability"),
        authority
            .write_capability(ManagedRoot::MediaBlobs)
            .expect("write capability"),
        Database::open(database_path).expect("blob database"),
        Database::open(database_path).expect("asset database"),
    )
}

fn media_object(root: &std::path::Path, hash: &ContentHash) -> std::path::PathBuf {
    root.join("platform-v2")
        .join("media-blobs")
        .join("objects")
        .join(&hash.as_str()[..2])
        .join(&hash.as_str()[2..4])
        .join(hash.as_str())
}

fn chat_image(
    media: &lettuce_media::LocalMediaBlobStore<Database, Database>,
    marker: &[u8],
) -> lettuce_media::IngestedMedia {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend_from_slice(&13_u32.to_be_bytes());
    bytes.extend_from_slice(b"IHDR");
    bytes.extend_from_slice(&2_u32.to_be_bytes());
    bytes.extend_from_slice(&3_u32.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes.extend_from_slice(marker);
    media
        .ingest(
            bytes.as_slice(),
            lettuce_media::IngestRequest::new(
                lettuce_media::AssetKind::MessageImage,
                lettuce_media::AssetOrigin::Upload,
                lettuce_media::RetentionClass::Persistent,
                lettuce_media::AssetProvenanceV1::default(),
            ),
        )
        .expect("ingest image")
}

async fn generated_direct_chat(database: &Database, prefix: &str) -> (Scenario, MessageId) {
    let scenario = scenario(database, false, prefix);
    let work = admit_and_claim(database, &scenario, 1_015);
    ConversationGenerationJobRunner::new(
        database,
        &scripted(vec![text_outcome(prefix, "Reply", 10, 5)]),
    )
    .run(&work, input(&scenario), TimestampMillis::new(1_020))
    .await
    .expect("run");
    let conversation = ConversationReader::get(database, scenario.conversation_id)
        .expect("conversation")
        .conversation;
    let reply = branch_timeline(database, conversation.id, conversation.active_branch_id)
        .last()
        .expect("reply")
        .message
        .id;
    (scenario, reply)
}

fn attach_image(
    database: &Database,
    conversation_id: ConversationId,
    message_id: MessageId,
    asset_id: lettuce_types::AssetId,
    at: i64,
) {
    lettuce_conversations::ConversationRepository::edit_message(
        database,
        &lettuce_conversations::EditMessage {
            conversation_id,
            message_id,
            expected_revision: ConversationReader::get(database, conversation_id)
                .expect("conversation")
                .conversation
                .revision,
            operation: OperationToken {
                key: key(&format!("attach-{asset_id}")),
                request_digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
            },
            draft: lettuce_conversations::MessageEditDraft {
                parts: vec![
                    MessagePart::Text {
                        text: "Look".into(),
                    },
                    MessagePart::MediaAsset {
                        asset_id,
                        role: lettuce_conversations::MediaAssetRole::Inline,
                    },
                ],
                visibility: MessageVisibility::Visible,
                pinned: false,
                scene_edited: false,
            },
        },
        TimestampMillis::new(at),
    )
    .expect("attach image");
}

fn direct_character(database: &Database, conversation_id: ConversationId) -> CharacterId {
    ConversationReader::get(database, conversation_id)
        .expect("conversation")
        .conversation
        .participants
        .iter()
        .find_map(|participant| match participant.source {
            lettuce_conversations::ParticipantSource::Character(id) => Some(id),
            _ => None,
        })
        .expect("character participant")
}

/// Legacy `session_delete` (old-code/src-tauri/src/storage_manager/sessions.rs:3794)
/// removed a chat for good; usage records outlived it. Its media go once unused, the
/// peer purges it too, and a file left by an interrupted collection is swept.
#[tokio::test]
async fn a_deleted_chat_goes_with_its_media_here_and_on_the_sync_peer() {
    let root = hard_delete_root("hard-delete-chat");
    let path = hard_delete_path(&root, "a.sqlite3");
    let a = Database::open(&path).expect("a");
    let b = database();
    let media = hard_delete_media(&path, &root);
    let location = hard_delete_location(&root);
    let scope = crate::MediaGarbageScope {
        store: &media,
        location: &location,
        open_database: &path,
    };
    let (scenario, reply) = generated_direct_chat(&a, "hard-delete-chat").await;
    let id = scenario.conversation_id;
    sync_prompts(&a, &b, 3_000);
    sync_prompts(&b, &a, 3_100);
    assert!(ConversationReader::get(&b, id).is_ok());
    let image = chat_image(&media, b"chat");
    attach_image(&a, id, reply, image.asset.id, 3_200);
    let usage_id = ConversationReader::get_turn(&a, scenario.turn_id)
        .expect("turn")
        .attempts[0]
        .usage_event_id
        .expect("usage");
    assert!(media_object(&root, &image.blob.content_hash).exists());

    let deletion =
        crate::delete_conversation(&a, &scope, id, TimestampMillis::new(4_000)).expect("delete");
    assert_eq!(deletion.receipt.conversations, vec![id]);
    assert_eq!(deletion.media.removed, 1);
    assert_eq!(deletion.media.failed, 0);
    assert!(!media_object(&root, &image.blob.content_hash).exists());
    assert!(ConversationReader::get(&a, id).is_err());
    assert!(UsageLedger::get(&a, usage_id).expect("usage").is_some());
    assert_eq!(
        lettuce_media::MediaAssetRepository::get(&a, image.asset.id).expect("asset"),
        None
    );

    sync_prompts(&a, &b, 5_000);
    assert!(ConversationReader::get(&b, id).is_err());
    assert!(UsageLedger::get(&b, usage_id).expect("usage").is_some());
    assert_rescans_are_empty(&[&a, &b], 6_000);

    let stray = ContentHash::parse("ab".repeat(32)).expect("hash");
    let stray_path = media_object(&root, &stray);
    std::fs::create_dir_all(stray_path.parent().expect("parent")).expect("stray dir");
    std::fs::write(&stray_path, b"orphan").expect("stray");
    let kept = chat_image(&media, b"kept");
    let swept =
        crate::sweep_orphan_media_files(&a, &scope, TimestampMillis::new(7_000)).expect("sweep");
    assert_eq!(swept.removed, 1);
    assert!(!stray_path.exists());
    assert!(media_object(&root, &kept.blob.content_hash).exists());
    drop((a, media));
    std::fs::remove_dir_all(root).expect("cleanup");
}

/// Legacy `session_delete` dropped only the session's own memories
/// (sessions.rs:3798); `character_delete` dropped the companion's shared memory and
/// its soul, notes and episodes (characters.rs:1066-1101, db.rs:803-861).
#[tokio::test]
async fn a_companion_pool_outlives_its_chats_and_goes_with_the_companion() {
    let root = hard_delete_root("hard-delete-companion");
    let path = hard_delete_path(&root, "app.sqlite3");
    let backend = AppBackend::open(&path, TimestampMillis::new(1)).expect("backend");
    let media = hard_delete_media(&path, &root);
    let location = hard_delete_location(&root);
    let scope = crate::MediaGarbageScope {
        store: &media,
        location: &location,
        open_database: &path,
    };
    let database = backend.database();
    enable_retrieval_only_dynamic_memory(database);
    let scenario = direct_scenario_with(
        database,
        true,
        "hard-delete-companion",
        true,
        false,
        |defaults| {
            defaults.interaction_mode = InteractionMode::Companion;
            defaults.companion_soul = Some(lettuce_companions::CompanionSoulConfig::default());
        },
    );
    let pool = scenario.space_id.expect("companion pool");
    seed_retrieved_and_hot_memories(database, pool);
    let work = admit_and_claim(database, &scenario, 1_015);
    backend
        .prepared_conversation_generation_runner(
            &ScenarioEmbeddingEngine,
            &scripted(vec![text_outcome("hard-delete-companion", "Tea.", 9, 3)]),
        )
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("companion turn");
    let character = direct_character(database, scenario.conversation_id);

    crate::delete_conversation(
        database,
        &scope,
        scenario.conversation_id,
        TimestampMillis::new(2_000),
    )
    .expect("delete chat");
    assert!(ConversationReader::get(database, scenario.conversation_id).is_err());
    assert_eq!(
        MemoryRepository::get(database, pool)
            .expect("pool")
            .expect("pool kept")
            .items
            .len(),
        2
    );
    assert!(
        lettuce_companions::SoulRepository::get(
            database,
            lettuce_companions::SoulOwner::Character(character)
        )
        .expect("soul")
        .is_some()
    );

    let deletion =
        crate::delete_character(database, &scope, character, TimestampMillis::new(3_000))
            .expect("delete companion");
    assert_eq!(deletion.receipt.characters, vec![character]);
    assert_eq!(MemoryRepository::get(database, pool).expect("pool"), None);
    assert_eq!(
        lettuce_companions::SoulRepository::get(
            database,
            lettuce_companions::SoulOwner::Character(character)
        )
        .expect("soul"),
        None
    );
    assert_eq!(
        CharacterRepository::get(database, character).expect("character"),
        None
    );
    drop((backend, media));
    std::fs::remove_dir_all(root).expect("cleanup");
}

/// Legacy `character_delete` (old-code/src-tauri/src/storage_manager/characters.rs:1066)
/// had no group check: it cascaded to the character's sessions (db.rs:735), left group
/// chats alone and left its id in `group_characters.character_ids`, which group reads
/// then skipped (useGroupSettingsController.ts:62). Here the character leaves each
/// group; a group left with one member cannot exist and goes with a notice.
#[tokio::test]
async fn deleting_a_character_takes_its_direct_chats_and_leaves_its_groups() {
    let root = hard_delete_root("hard-delete-character");
    let path = hard_delete_path(&root, "app.sqlite3");
    let database = Database::open(&path).expect("database");
    let media = hard_delete_media(&path, &root);
    let location = hard_delete_location(&root);
    let scope = crate::MediaGarbageScope {
        store: &media,
        location: &location,
        open_database: &path,
    };
    let (chat, _) = generated_direct_chat(&database, "hard-delete-character").await;
    let character = direct_character(&database, chat.conversation_id);
    let bea = seed_named_character(&database, "Bea");
    let cy = seed_named_character(&database, "Cy");
    let trio = seed_group(
        &database,
        vec![member(bea, 0), member(character, 1), member(cy, 2)],
        None,
        |_| {},
    );
    let pair = seed_group(
        &database,
        vec![member(character, 0), member(bea, 1)],
        None,
        |_| {},
    );

    let deletion =
        crate::delete_character(&database, &scope, character, TimestampMillis::new(3_000))
            .expect("delete character");
    assert_eq!(deletion.receipt.conversations, vec![chat.conversation_id]);
    assert_eq!(deletion.receipt.characters, vec![character]);
    assert!(ConversationReader::get(&database, chat.conversation_id).is_err());
    let trio = lettuce_characters::GroupRepository::get(&database, trio)
        .expect("trio")
        .expect("trio kept");
    assert_eq!(
        trio.group
            .members
            .iter()
            .map(|member| (member.character_id, member.ordinal))
            .collect::<Vec<_>>(),
        vec![(bea, 0), (cy, 1)]
    );
    let before_pair = pair;
    let pair = lettuce_characters::GroupRepository::get(&database, pair)
        .expect("pair")
        .expect("a group left with one member keeps its settings");
    assert_eq!(pair.group.name, "Cast");
    assert_eq!(
        pair.group
            .members
            .iter()
            .map(|member| (member.character_id, member.ordinal))
            .collect::<Vec<_>>(),
        vec![(bea, 0)]
    );
    assert!(
        lettuce_characters::GroupRepository::rename(
            &database,
            before_pair,
            pair.group.revision,
            "Renamed cast".into(),
            TimestampMillis::new(3_100),
        )
        .is_ok(),
        "a degraded group can still be edited"
    );
    let notices = database.purge_notices().expect("notices");
    assert_eq!(notices.len(), 1);
    assert_eq!(
        notices[0].entity,
        lettuce_database::PurgeNoticeEntity::Group
    );
    assert_eq!(notices[0].entity_id, before_pair.to_string());
    assert_eq!(
        notices[0].reason,
        lettuce_database::PurgeNoticeReason::GroupBelowTwoMembers
    );
    drop((database, media));
    std::fs::remove_dir_all(root).expect("cleanup");
}

/// Legacy `group_session_delete` (old-code/src-tauri/src/storage_manager/group_sessions.rs:1962)
/// removed the group session and its messages; the reusable group stayed. A group chat
/// stays readable after one of its characters is deleted.
#[tokio::test]
async fn a_group_chat_outlives_a_deleted_member_and_is_deleted_on_its_own() {
    let root = hard_delete_root("hard-delete-group");
    let path = hard_delete_path(&root, "app.sqlite3");
    let backend = AppBackend::open(&path, TimestampMillis::new(1)).expect("backend");
    let media = hard_delete_media(&path, &root);
    let location = hard_delete_location(&root);
    let scope = crate::MediaGarbageScope {
        store: &media,
        location: &location,
        open_database: &path,
    };
    let (scenario, _) = group_scenario(
        &backend,
        "hard-delete-group",
        lettuce_characters::SpeakerSelection::Heuristic,
        true,
    );
    let work = admit_and_claim(backend.database(), &scenario, 1_015);
    backend
        .prepared_conversation_generation_runner(
            &ScenarioEmbeddingEngine,
            &scripted(vec![text_outcome(
                "hard-delete-group",
                "Ada answers.",
                10,
                3,
            )]),
        )
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
        )
        .await
        .expect("group turn");
    let database = backend.database();
    let before = ConversationReader::get(database, scenario.conversation_id).expect("group chat");
    let characters: Vec<CharacterId> = before
        .conversation
        .participants
        .iter()
        .filter_map(|participant| match participant.source {
            lettuce_conversations::ParticipantSource::Character(id) => Some(id),
            _ => None,
        })
        .collect();
    assert_eq!(characters.len(), 2);
    let branch = before.conversation.active_branch_id;
    let timeline = branch_timeline(database, scenario.conversation_id, branch);

    let deletion =
        crate::delete_character(database, &scope, characters[1], TimestampMillis::new(2_000))
            .expect("delete a member");
    assert!(deletion.receipt.conversations.is_empty());
    assert_eq!(
        ConversationReader::get(database, scenario.conversation_id).expect("still readable"),
        before
    );
    assert_eq!(
        branch_timeline(database, scenario.conversation_id, branch),
        timeline
    );
    let notices = database.purge_notices().expect("notices");
    assert_eq!(notices.len(), 1);
    let group = lettuce_characters::GroupRepository::get(
        database,
        notices[0].entity_id.parse().expect("group id"),
    )
    .expect("group")
    .expect("the source group stays");
    assert_eq!(
        group
            .group
            .members
            .iter()
            .map(|member| member.character_id)
            .collect::<Vec<_>>(),
        vec![characters[0]]
    );

    let next = next_group_turn(
        database,
        &scenario,
        "hard-delete-group-after",
        "Still there?",
        2_100,
    );
    let work = admit_and_claim(database, &next, 2_101);
    let result = backend
        .prepared_conversation_generation_runner(
            &ScenarioEmbeddingEngine,
            &scripted(vec![text_outcome(
                "hard-delete-group-after",
                "Ada still answers.",
                10,
                3,
            )]),
        )
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(2_110),
        )
        .await
        .expect("a turn after a member was deleted");
    assert_eq!(result.turn.status, GenerationTurnStatus::Succeeded);
    let speaker = result
        .turn
        .selected_speaker
        .expect("speaker")
        .participant_id;
    let speaker_source = ConversationReader::get(database, scenario.conversation_id)
        .expect("chat")
        .conversation
        .participants
        .into_iter()
        .find(|participant| participant.id == speaker)
        .expect("speaker participant")
        .source;
    assert_eq!(
        speaker_source,
        lettuce_conversations::ParticipantSource::Character(characters[0])
    );

    let deletion = crate::delete_conversation(
        database,
        &scope,
        scenario.conversation_id,
        TimestampMillis::new(3_000),
    )
    .expect("delete group chat");
    assert_eq!(
        deletion.receipt.conversations,
        vec![scenario.conversation_id]
    );
    assert!(ConversationReader::get(database, scenario.conversation_id).is_err());
    assert!(
        CharacterRepository::get(database, characters[0])
            .expect("character")
            .is_some()
    );
    drop((backend, media));
    std::fs::remove_dir_all(root).expect("cleanup");
}

/// A restore keeps the previous database file; media that file catalogs survive
/// collection and sweeps of the new one.
#[tokio::test]
async fn media_a_kept_database_names_survives_collection_and_sweeps() {
    let root = hard_delete_root("hard-delete-kept-database");
    let path = hard_delete_path(&root, "new.sqlite3");
    let kept_path = hard_delete_path(&root, "kept.sqlite3");
    let database = Database::open(&path).expect("database");
    let kept = Database::open(&kept_path).expect("kept database");
    let media = hard_delete_media(&path, &root);
    let kept_media = hard_delete_media(&kept_path, &root);
    let (scenario, reply) = generated_direct_chat(&database, "hard-delete-kept").await;
    let image = chat_image(&media, b"shared with the kept database");
    chat_image(&kept_media, b"shared with the kept database");
    attach_image(
        &database,
        scenario.conversation_id,
        reply,
        image.asset.id,
        2_000,
    );
    drop(kept);
    let location = hard_delete_location(&root);
    let scope = crate::MediaGarbageScope {
        store: &media,
        location: &location,
        open_database: &path,
    };

    let deletion = crate::delete_conversation(
        &database,
        &scope,
        scenario.conversation_id,
        TimestampMillis::new(3_000),
    )
    .expect("delete");
    assert_eq!(deletion.media.removed, 0);
    assert_eq!(
        lettuce_media::MediaAssetRepository::get(&database, image.asset.id).expect("asset"),
        None
    );
    assert!(media_object(&root, &image.blob.content_hash).exists());
    assert_eq!(
        crate::sweep_orphan_media_files(&database, &scope, TimestampMillis::new(3_100))
            .expect("sweep")
            .removed,
        0
    );
    assert!(media_object(&root, &image.blob.content_hash).exists());

    let broken = hard_delete_path(&root, "broken.sqlite3");
    std::fs::write(&broken, b"not a database").expect("broken file");
    assert_eq!(
        crate::sweep_orphan_media_files(&database, &scope, TimestampMillis::new(3_200))
            .expect("skipped sweep"),
        lettuce_media::MediaObjectRemoval::default()
    );
    let notices = database.purge_notices().expect("notices");
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].entity_id, "broken.sqlite3");
    assert_eq!(
        notices[0].reason,
        lettuce_database::PurgeNoticeReason::MediaCollectionSkipped
    );
    std::fs::remove_file(&broken).expect("remove broken file");

    drop(kept_media);
    for suffix in ["", "-wal", "-shm"] {
        let file = std::path::PathBuf::from(format!("{}{suffix}", kept_path.display()));
        if file.exists() {
            std::fs::remove_file(file).expect("remove kept database");
        }
    }
    assert_eq!(
        crate::sweep_orphan_media_files(&database, &scope, TimestampMillis::new(3_300))
            .expect("sweep without the kept database")
            .removed,
        1
    );
    drop((database, media));
    std::fs::remove_dir_all(root).expect("cleanup");
}

/// A delete received while this device has messages the deleting device never saw
/// is refused: the chat stays here, the user is told, and the chat goes back to the
/// other device whole.
#[tokio::test]
async fn a_received_delete_keeps_a_chat_with_unsent_local_messages() {
    let a = database();
    let b = database();
    let (scenario, _) = generated_direct_chat(&a, "hard-delete-unsent").await;
    let id = scenario.conversation_id;
    sync_prompts(&a, &b, 3_000);
    sync_prompts(&b, &a, 3_100);
    a.purge_conversation(id, TimestampMillis::new(3_200))
        .expect("delete on a");
    send_and_generate(&b, &scenario, "hard-delete-unsent-b", "Still here", 3_300).await;

    sync_prompts(&a, &b, 4_000);
    let kept = ConversationReader::get(&b, id).expect("kept on b");
    let notices = b.purge_notices().expect("notices");
    assert_eq!(notices.len(), 1);
    assert_eq!(
        notices[0].entity,
        lettuce_database::PurgeNoticeEntity::Conversation
    );
    assert_eq!(notices[0].entity_id, id.to_string());
    assert_eq!(
        notices[0].reason,
        lettuce_database::PurgeNoticeReason::KeptUnsentLocalChanges
    );

    sync_prompts(&b, &a, 4_100);
    sync_prompts(&a, &b, 4_200);
    let back = ConversationReader::get(&a, id).expect("back on a");
    assert_eq!(back.conversation.title, kept.conversation.title);
    let branch = kept.conversation.active_branch_id;
    let texts = |database: &Database| {
        branch_timeline(database, id, branch)
            .into_iter()
            .map(|item| item.message.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(texts(&a), texts(&b));
    assert_eq!(texts(&a).len(), 4);
    assert!(a.purge_notices().expect("a notices").is_empty());
    assert_rescans_are_empty(&[&a, &b], 5_000);
}

/// A delete that waited behind a running memory cycle is decided again at the
/// next sync: the user wrote in the chat meanwhile, so it is kept and sent back.
#[tokio::test]
async fn a_delete_queued_behind_a_memory_run_is_decided_again_after_new_messages() {
    let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let scenario = finalized_dynamic_turn(&backend, "hard-delete-queued").await;
    let blocking = BlockingInference::new(text_outcome("hard-delete-memory", "Noted.", 1, 1));
    {
        let engine = ScenarioEmbeddingEngine;
        let host = backend.companion_memory_host(&engine, &blocking);
        let scheduler = crate::PostTurnMemoryScheduler::new();
        assert!(scheduler.enqueue(scenario.conversation_id));
        let clock = FakeClock::new(TimestampMillis::new(1_030));
        let follow_up = crate::CompanionFollowUpHost::new(backend.database(), &blocking);
        tokio::select! {
            () = host.drive(
                &scheduler,
                scenario.conversation_id,
                WorkerId::new(),
                LEASE,
                &clock,
                &follow_up,
            ) => panic!("the memory cycle must stay in flight"),
            () = blocking.entered.notified() => {}
        }
    }
    let b = backend.database();
    let a = database();
    let id = scenario.conversation_id;
    sync_prompts(b, &a, 3_000);
    sync_prompts(&a, b, 3_100);
    a.purge_conversation(id, TimestampMillis::new(3_200))
        .expect("delete on a");
    sync_prompts(&a, b, 3_300);
    assert!(
        ConversationReader::get(b, id).is_ok(),
        "the delete waits behind the memory run"
    );
    assert!(b.purge_notices().expect("notices").is_empty());

    send_and_generate(b, &scenario, "hard-delete-queued-b", "Still here", 3_400).await;
    sync_prompts(&a, b, 3_500);
    assert!(ConversationReader::get(b, id).is_ok());
    let notices = b.purge_notices().expect("notices");
    assert_eq!(notices.len(), 1);
    assert_eq!(
        notices[0].reason,
        lettuce_database::PurgeNoticeReason::KeptUnsentLocalChanges
    );
    sync_prompts(b, &a, 3_600);
    sync_prompts(&a, b, 3_700);
    let branch = ConversationReader::get(b, id)
        .expect("b")
        .conversation
        .active_branch_id;
    let messages = |database: &Database| {
        branch_timeline(database, id, branch)
            .into_iter()
            .map(|item| item.message.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(messages(&a), messages(b));
    assert_eq!(messages(&a).len(), 4);
}

/// Chat content journaled again by the device that kept it meets a third
/// device that still holds the same content as a no-op: no conflict, no fork.
#[tokio::test]
async fn re_sent_chats_meet_a_peer_that_still_holds_them_without_conflicts() {
    use lettuce_sync::ConversationForkRepository;
    let a = database();
    let b = database();
    let c = database();
    let (scenario, _) = generated_direct_chat(&a, "hard-delete-three").await;
    let id = scenario.conversation_id;
    for (from, to, at) in [
        (&a, &b, 3_000),
        (&b, &a, 3_010),
        (&a, &c, 3_020),
        (&c, &a, 3_030),
    ] {
        sync_prompts(from, to, at);
    }
    sync_prompts(&b, &c, 3_040);
    sync_prompts(&c, &b, 3_050);
    let conflicts_before = c.unresolved_sync_conflict_count().expect("c conflicts");
    a.purge_conversation(id, TimestampMillis::new(3_100))
        .expect("delete on a");
    send_and_generate(&b, &scenario, "hard-delete-three-b", "Still here", 3_200).await;
    sync_prompts(&a, &b, 3_300);
    assert_eq!(b.purge_notices().expect("b notices").len(), 1);

    sync_prompts(&b, &c, 3_400);
    assert_eq!(
        c.unresolved_sync_conflict_count().expect("c conflicts"),
        conflicts_before
    );
    assert!(
        c.unresolved_conversation_forks(10)
            .expect("c forks")
            .is_empty()
    );
    sync_prompts(&a, &c, 3_500);
    assert!(
        ConversationReader::get(&c, id).is_ok(),
        "the delete did not see b's new content"
    );
    assert!(c.purge_notices().expect("c notices").is_empty());

    for (from, to, at) in [
        (&b, &a, 3_600),
        (&c, &a, 3_610),
        (&a, &b, 3_620),
        (&a, &c, 3_630),
        (&b, &c, 3_640),
        (&c, &b, 3_650),
    ] {
        sync_prompts(from, to, at);
    }
    let branch = ConversationReader::get(&b, id)
        .expect("b")
        .conversation
        .active_branch_id;
    let messages = |database: &Database| {
        branch_timeline(database, id, branch)
            .into_iter()
            .map(|item| item.message.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(messages(&b).len(), 4);
    assert_eq!(messages(&a), messages(&b));
    assert_eq!(messages(&c), messages(&b));
    for database in [&a, &b, &c] {
        assert!(
            database
                .unresolved_conversation_forks(10)
                .expect("forks")
                .is_empty()
        );
    }
    assert_eq!(
        c.unresolved_sync_conflict_count().expect("c conflicts"),
        conflicts_before
    );
    assert_rescans_are_empty(&[&a, &b, &c], 4_000);
}
