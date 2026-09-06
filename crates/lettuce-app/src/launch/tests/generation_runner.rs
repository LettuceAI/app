use super::*;
use lettuce_conversations::ToolExecution;
use lettuce_conversations::{
    ConversationManager, GenerationAttemptStatus, GenerationFailureCode, InitialInferenceBinding,
    InitialInferenceRepository, InitialInferenceResult, ModelSelectionSnapshot, OperationKind,
    ResolveGroupSpeaker, SelectedSpeakerDecision, SpeakerDecisionMethod, SpeakerFallback,
    ToolExecutionOwner, UsageCounters, UsageOutcome, UsageUnavailableReason,
};
use lettuce_embeddings::{
    EmbeddingDimensions, MemoryEmbeddingProjection, MemoryEmbeddingRepository,
};
use lettuce_jobs::{JobErrorCode, JobSnapshot, JobStore, events::JobEvent};
use lettuce_memory::{MemoryRepositoryError, MemoryRetrievalAccess, MemoryRetrievalRepository};
use lettuce_types::{ConversationId, GenerationAttemptId, GenerationTurnId};
use lettuce_usage::{JobInferenceUsageResult, JobUsageLedger, UsageEvent, UsageLedger};

use crate::conversation_generation::{ConversationGenerationOperation, operation_token};
use crate::{
    ConversationGenerationClaimedWork, ConversationGenerationDispatchCoordinator,
    ConversationGenerationDispatchError, ConversationGenerationInput,
    ConversationGenerationJobRunner, ConversationGenerationMemoryInput,
    ConversationGenerationRunError, ConversationGenerationRuntimeInput,
    ConversationGenerationSettledWork, GenerationUsageEvidence,
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

fn group_scenario(
    backend: &AppBackend,
    prefix: &str,
    speaker_selection: lettuce_characters::SpeakerSelection,
    mute_second: bool,
) -> (Scenario, Vec<lettuce_types::ConversationParticipantId>) {
    let database = backend.database();
    let model_id = seed_model(database, ProviderProtocol::Ollama, "ollama");
    let mut stored_model = ModelProfileRepository::get(database, model_id)
        .expect("model")
        .expect("model exists");
    let model_revision = stored_model.revision;
    stored_model.config.chat_parameters.temperature = None;
    stored_model.config.capabilities.streaming = lettuce_models::CapabilityStatus::Supported;
    ModelProfileRepository::upsert(database, stored_model, Some(model_revision))
        .expect("resolvable model profile");
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
            &direct_send_command(&conversation, &format!("{prefix}-send"), "Hello cast."),
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
        .run(
            &work,
            runtime.clone(),
            TimestampMillis::new(1_020),
            |_| vec![],
        )
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
            |_| vec![],
        )
        .await
        .expect("run manual continuation");
    assert!(continued_inference
        .requests
        .lock()
        .expect("continuation requests")[0]
        .context
        .messages
        .iter()
        .any(|message| message.parts.iter().any(|part| {
            matches!(part, ProviderContextPart::Text { text } if text.contains("- Mira keeps a handwritten tea journal."))
        })));

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
                guidance: None,
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
            |_| vec![],
        )
        .await
        .expect("run manual regeneration");
    assert_eq!(regenerated_result.candidate.ordinal, 1);
    assert!(regenerated_inference
        .requests
        .lock()
        .expect("regeneration requests")[0]
        .context
        .messages
        .iter()
        .any(|message| message.parts.iter().any(|part| {
            matches!(part, ProviderContextPart::Text { text } if text.contains("- Mira keeps a handwritten tea journal."))
        })));

    let replay = runner
        .run(
            &work,
            ConversationGenerationRuntimeInput {
                stream_sink: Some(RequestId::new()),
                ..runtime
            },
            TimestampMillis::new(1_031),
            |_| vec![],
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
                    |_| vec![],
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
                |_| vec![],
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
    let runner = backend.prepared_conversation_generation_runner(&engine, &inference);
    let result = runner
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
            |_| vec![],
        )
        .await
        .expect("run mentioned group speaker");
    assert_eq!(result.candidate.author_participant_id, speakers[1]);
    assert_eq!(result.turn.selected_speaker, Some(mentioned));
    assert!(
        inference.requests.lock().expect("requests")[0]
            .context
            .messages
            .iter()
            .any(|message| message.parts.iter().any(|part| {
                matches!(part, ProviderContextPart::Text { text } if text == "Hello cast.")
            }))
    );
    let replay = runner
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_021),
            |_| vec![],
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
            |_| vec![],
        )
        .await
        .expect("run director continuation");
    assert_eq!(
        continued_result.candidate.author_participant_id,
        speakers[0]
    );
    assert!(continued_result.turn.selected_speaker.is_none());

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
            |_| vec![],
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
                |_| vec![],
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
    let space_id = scenario.space_id.expect("dynamic memory space");
    let memory_id = MemoryId::new();
    let stored = MemoryRepository::get(backend.database(), space_id)
        .expect("memory")
        .expect("memory exists");
    let memory = MemoryItem {
        id: memory_id,
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
    let inference = scripted(vec![
        call_outcome(
            "prepared-dynamic-tool",
            "pin_memory",
            serde_json::json!({"id": memory_id}),
            (20, 5),
        ),
        text_outcome("prepared-dynamic-response", "I will remember that.", 5, 3),
    ]);
    let engine = ScenarioEmbeddingEngine;
    let runner = backend.prepared_conversation_generation_runner(&engine, &inference);
    let result = runner
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_020),
            |_| vec![],
        )
        .await
        .expect("prepared dynamic send");
    assert_eq!(result.rounds, 1);
    assert_eq!(result.outcomes.len(), 2);
    let plans = DynamicMemoryPreparationRepository::list_preparation_plans(
        backend.database(),
        scenario.conversation_id,
        scenario.turn_id,
        scenario.attempt_id,
    )
    .expect("preparation plans");
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].policy.max_entries, 12);
    assert_eq!(plans[0].policy.hot_token_budget, 321);
    assert_eq!(plans[0].duplicate_threshold.basis_points(), 8_800);
    {
        let requests = inference.requests.lock().expect("requests");
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].tools, Some(dynamic_memory_tool_request()));
        assert_eq!(requests[0].profile.tool_policy, ToolPolicy::Allowed);
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
    assert_eq!(accessed_item.importance, Score::FULL);
    assert!(accessed_item.is_pinned);
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
        .expect("retrieval access exists")
        .access,
        retrieval_access
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
    let dispatch_evidence = backend
        .database()
        .job_usage(work.handle.id())
        .expect("dispatch evidence");
    assert_eq!(dispatch_evidence.len(), 2);
    let aggregate_event = UsageLedger::get(backend.database(), result.usage_event_id)
        .expect("usage")
        .expect("usage exists");
    assert_eq!(
        aggregate_event.record.usage,
        UsageCounters::Known(usage(25, 8).expect("aggregate usage"))
    );
    let replay = runner
        .run(
            &work,
            ConversationGenerationRuntimeInput::default(),
            TimestampMillis::new(1_030),
            |_| vec![],
        )
        .await
        .expect("prepared dynamic replay");
    assert!(replay.replayed);
    assert_eq!(replay.candidate.id, result.candidate.id);
    assert_eq!(inference.requests.lock().expect("requests").len(), 2);
    assert_eq!(
        MemoryRepository::get(backend.database(), space_id)
            .expect("memory after replay")
            .expect("memory exists after replay"),
        accessed
    );
    assert_eq!(
        backend
            .database()
            .job_usage(work.handle.id())
            .expect("unchanged dispatch evidence"),
        dispatch_evidence
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
    let engine = ScenarioEmbeddingEngine;
    let result = ConversationGenerationJobRunner::new(&engine, &database, &inference)
        .run(
            &work,
            input(&scenario, false),
            TimestampMillis::new(1_020),
            |_| vec![],
        )
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
async fn settled_tool_round_is_replayed_before_continuation_without_reapplying_memory() {
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
            Ok(call_outcome(
                "round-second",
                "pin_memory",
                serde_json::json!({"id": memory_id}),
                (8, 2),
            )),
            Err(PortError::Unavailable),
            Ok(text_outcome(
                "round-recovered",
                "I will remember that.",
                7,
                3,
            )),
        ])),
        requests: Mutex::new(vec![]),
    };
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
    assert_eq!(inference.requests.lock().expect("requests").len(), 3);
    let stored_memory = MemoryRepository::get(&database, space_id)
        .expect("memory")
        .expect("memory exists");
    assert_eq!(stored_memory.revision, Revision::new(3));
    let result = runner
        .run(
            &work,
            input(&scenario, true),
            TimestampMillis::new(1_021),
            seeds,
        )
        .await
        .expect("resume settled round");
    assert_eq!(result.rounds, 2);
    assert_eq!(result.outcomes.len(), 2);
    assert_eq!(inference.requests.lock().expect("requests").len(), 4);
    let replayed_parts = inference.requests.lock().expect("requests")[3]
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
        .count();
    assert_eq!(replayed_parts, 4);
    let job_id = work.handle.id();
    let settled = ConversationGenerationDispatchCoordinator::new(&database, &database)
        .settle(
            work,
            Ok(result),
            CancellationReason::Recovery,
            TimestampMillis::new(1_022),
        )
        .expect("settle");
    let ConversationGenerationSettledWork::Succeeded { job, .. } = settled else {
        panic!("expected resumed success: {settled:?}");
    };
    assert_eq!(job.state, JobState::Succeeded);
    assert_eq!(persisted_job(&database, job_id).state, JobState::Succeeded);
    let turn = ConversationReader::get_turn(&database, scenario.turn_id).expect("turn");
    assert_eq!(turn.status, GenerationTurnStatus::Succeeded);
    assert_eq!(turn.attempts.len(), 1);
    let usage_event = attempt_usage(&database, scenario.turn_id, 0);
    assert_eq!(usage_event.record.outcome, UsageOutcome::Succeeded);
    assert_eq!(
        usage_event.record.usage,
        UsageCounters::Unavailable(UsageUnavailableReason::TransportFailed)
    );
    assert_eq!(
        MemoryRepository::get(&database, space_id)
            .expect("memory")
            .expect("memory exists")
            .revision,
        Revision::new(3)
    );
}

#[tokio::test]
async fn rejected_round_recovery_still_fails_closed_without_redispatch() {
    let database = database();
    let scenario = scenario(&database, true, "rejected-round");
    let space_id = scenario.space_id.expect("space");
    let work = admit_and_claim(&database, &scenario, 1_015);
    let inference = scripted(vec![call_outcome(
        "rejected-round-initial",
        "create_memory",
        serde_json::json!({"text": "Mira prefers tea", "category": "preference"}),
        (20, 5),
    )]);
    let engine = ScenarioEmbeddingEngine;
    let runner = ConversationGenerationJobRunner::new(&engine, &database, &inference);
    let first = runner
        .run(
            &work,
            input(&scenario, true),
            TimestampMillis::new(1_020),
            |_| vec![],
        )
        .await
        .expect_err("missing create seed rejects the round");
    assert!(matches!(
        first,
        ConversationGenerationRunError::Round { .. }
    ));
    let rerun = runner
        .run(
            &work,
            input(&scenario, true),
            TimestampMillis::new(1_021),
            |_| vec![],
        )
        .await
        .expect_err("rejected round cannot be resumed");
    assert!(matches!(
        rerun,
        ConversationGenerationRunError::ToolRoundsUnrecoverable { .. }
    ));
    assert_eq!(inference.requests.lock().expect("requests").len(), 1);
    assert_eq!(
        MemoryRepository::get(&database, space_id)
            .expect("memory")
            .expect("memory exists")
            .revision,
        Revision::INITIAL
    );
}

#[tokio::test]
async fn interrupted_planned_tail_is_cloned_and_continued_in_the_child() {
    let database = database();
    let scenario = scenario(&database, true, "interrupted-tail");
    let space_id = scenario.space_id.expect("space");
    let memory_id = MemoryId::new();
    let work = admit_and_claim(&database, &scenario, 1_015);
    let inference = FallibleScriptedInference {
        outcomes: Mutex::new(VecDeque::from([
            Ok(call_outcome(
                "interrupted-initial",
                "create_memory",
                serde_json::json!({"text": "Mira prefers tea", "category": "preference"}),
                (20, 5),
            )),
            Err(PortError::Unavailable),
            Ok(text_outcome(
                "interrupted-recovered",
                "I kept that in mind.",
                6,
                3,
            )),
        ])),
        requests: Mutex::new(vec![]),
    };
    let engine = ScenarioEmbeddingEngine;
    let runner = ConversationGenerationJobRunner::new(&engine, &database, &inference);
    let first = runner
        .run(
            &work,
            input(&scenario, true),
            TimestampMillis::new(1_020),
            |executions| {
                executions
                    .iter()
                    .map(|execution| crate::MemoryCreateSeed {
                        execution_id: execution.id,
                        id: memory_id,
                        token_count: 4,
                        created_at: TimestampMillis::new(1_016),
                    })
                    .collect()
            },
        )
        .await
        .expect_err("continuation provider failed");
    let evidence = first.evidence();
    let requested = ConversationManager::new(&database)
        .request_tool_executions(
            ToolExecutionOwner {
                conversation_id: scenario.conversation_id,
                turn_id: scenario.turn_id,
                attempt_id: scenario.attempt_id,
            },
            &dynamic_memory_tool_request(),
            vec![ProposedToolCall {
                provider_call_id: Some("interrupted-pin".into()),
                name: "pin_memory".into(),
                arguments: serde_json::json!({"id": memory_id}),
                raw_arguments: None,
                provider_replay: None,
            }],
            TimestampMillis::new(1_021),
        )
        .expect("admit interrupted tail");
    let validated = database
        .transition_tool_execution_batch(
            &[ToolExecutionTransition {
                id: requested[0].id,
                expected_revision: requested[0].revision,
                next: ToolExecutionStatus::Validated,
                output: None,
                failure: None,
            }],
            TimestampMillis::new(1_022),
        )
        .expect("validate interrupted tail");
    let running = crate::DynamicMemoryHandler::new(&database)
        .start_validated_round(&validated, TimestampMillis::new(1_023))
        .expect("start interrupted tail");
    let snapshot = MemoryRepository::get(&database, space_id)
        .expect("memory")
        .expect("memory exists");
    let memory_input = memory_input(space_id);
    crate::DynamicMemoryCreatePreparer::new(&engine, &database)
        .prepare_and_persist_admitted(
            space_id,
            snapshot.revision,
            &memory_input.policy,
            &running,
            &[],
            memory_input.duplicate_threshold,
            &work.claim,
            &work.handle,
        )
        .expect("persist interrupted plan");
    let parent_job_id = work.handle.id();
    let interrupted = ConversationGenerationDispatchCoordinator::new(&database, &database)
        .settle(
            work,
            Err(ConversationGenerationRunError::Pending { evidence }),
            CancellationReason::Recovery,
            TimestampMillis::new(1_024),
        )
        .expect("recover child");
    let ConversationGenerationSettledWork::Interrupted {
        child_attempt_id,
        child_job,
        ..
    } = interrupted
    else {
        panic!("expected interrupted parent: {interrupted:?}");
    };
    assert_eq!(
        persisted_job(&database, parent_job_id).state,
        JobState::Interrupted
    );
    let parent_usage = attempt_usage(&database, scenario.turn_id, 0);
    assert_eq!(parent_usage.record.outcome, UsageOutcome::Interrupted);
    assert_eq!(
        parent_usage.record.usage,
        UsageCounters::Unavailable(UsageUnavailableReason::TransportFailed)
    );
    let child_work = ConversationGenerationDispatchCoordinator::new(&database, &database)
        .claim(
            scenario.turn_id,
            child_attempt_id,
            WorkerId::new(),
            TimestampMillis::new(1_025),
            LEASE,
            &ResourceAvailability::all(),
        )
        .expect("claim child")
        .expect("child work");
    assert_eq!(child_work.handle.id(), child_job.id);
    let result = runner
        .run(
            &child_work,
            input(&scenario, true),
            TimestampMillis::new(1_026),
            |_| vec![],
        )
        .await
        .expect("continue child");
    assert_eq!(result.rounds, 2);
    assert_eq!(result.outcomes.len(), 1);
    assert_eq!(inference.requests.lock().expect("requests").len(), 3);
    let memory = MemoryRepository::get(&database, space_id)
        .expect("memory")
        .expect("memory exists");
    assert_eq!(memory.revision, Revision::new(3));
    assert!(memory.items[0].is_pinned);
    let child_executions = ToolExecutionRepository::list_tool_executions(
        &database,
        scenario.conversation_id,
        scenario.turn_id,
        child_attempt_id,
    )
    .expect("child executions");
    assert_eq!(child_executions.len(), 1);
    assert_eq!(
        child_executions[0].provider_call_id.as_deref(),
        Some("interrupted-pin")
    );
    assert_eq!(child_executions[0].status, ToolExecutionStatus::Succeeded);
    let child_usage = attempt_usage(&database, scenario.turn_id, 1);
    assert_eq!(child_usage.record.outcome, UsageOutcome::Succeeded);
    assert_eq!(
        child_usage.record.usage,
        UsageCounters::Known(usage(6, 3).expect("usage"))
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
