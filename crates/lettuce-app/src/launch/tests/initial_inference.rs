use super::*;
use lettuce_conversations::{
    InitialInferenceBinding, InitialInferenceRepository, InitialInferenceResult,
    ProviderReplayArtifactPort,
};
use lettuce_jobs::JobStore;
use lettuce_types::ConversationId;
use lettuce_usage::{JobInferenceUsageResult, JobUsageLedger};

fn operation(name: &str) -> OperationToken {
    OperationToken {
        key: key(name),
        request_digest: ContentHash::parse("ac".repeat(32)).expect("digest"),
    }
}

fn fixture(database: &Database) -> (ConversationId, InferenceRequest, JobHandle) {
    let model_id = seed_model(database, ProviderProtocol::Ollama, "ollama");
    set_application_default_model(database, model_id);
    let character = plain_character(database);
    let launched = ConversationLaunchPlanner::new(database)
        .launch_direct(&request(character, "initial-checkpoint"), NOW)
        .expect("launch")
        .value;
    let conversation = launched.conversation;
    let model = match &conversation
        .participants
        .iter()
        .find(|p| p.role == ParticipantRole::Character)
        .expect("character")
        .model_selection
    {
        SnapshotSelection::Inherited(model) | SnapshotSelection::Explicit(model) => model.clone(),
        SnapshotSelection::Disabled => panic!("model required"),
    };
    let sent = database
        .begin_send(
            &SendConversation {
                conversation_id: conversation.id,
                branch_id: conversation.active_branch_id,
                expected_revision: conversation.revision,
                operation: operation("initial-send"),
                swap_roles: false,
                message: MessageDraft {
                    role: MessageRole::User,
                    author_participant_id: Some(
                        conversation
                            .participants
                            .iter()
                            .find(|p| p.role == ParticipantRole::User)
                            .expect("user")
                            .id,
                    ),
                    parts: vec![MessagePart::Text {
                        text: "Remember tea.".into(),
                    }],
                    visibility: MessageVisibility::Visible,
                    pinned: false,
                    scene_edited: false,
                },
            },
            TimestampMillis::new(1_010),
        )
        .expect("send")
        .value;
    let job = database
        .create_or_get(
            lettuce_jobs::JobSpec::new(
                JobKind::MemoryExtraction,
                lettuce_jobs::JobSubject::new(
                    lettuce_jobs::SubjectKind::Conversation,
                    conversation.id.to_string(),
                )
                .expect("subject"),
                OutcomeRef::GenerationTurn(sent.turn.id),
            )
            .with_resources(vec![ResourceClass::Network]),
        )
        .expect("job")
        .job;
    database
        .attach_attempt_job(
            &AttachAttemptJob {
                conversation_id: conversation.id,
                turn_id: sent.turn.id,
                attempt_id: sent.attempt.id,
                expected_revision: sent.conversation.revision,
                expected_turn_revision: sent.turn.revision,
                operation: operation("initial-attach"),
                job_id: job.id,
            },
            TimestampMillis::new(1_011),
        )
        .expect("attach");
    let preparing = database
        .append_event(
            sent.turn.id,
            ConversationReader::get_turn(database, sent.turn.id)
                .expect("attached turn")
                .revision,
            &operation("initial-preparing"),
            GenerationCheckpointEnvelope {
                turn_id: sent.turn.id,
                attempt_id: sent.attempt.id,
                job_id: Some(job.id),
                correlation_id: None,
                sequence: 1,
                event: GenerationCheckpointEvent::Stage {
                    status: GenerationTurnStatus::Preparing,
                },
            },
            TimestampMillis::new(1_012),
        )
        .expect("preparing")
        .value;
    let prepared = database
        .prepare_generation(
            &lettuce_conversations::PrepareGeneration {
                conversation_id: conversation.id,
                turn_id: sent.turn.id,
                attempt_id: sent.attempt.id,
                job_id: job.id,
                expected_revision: ConversationReader::get(database, conversation.id)
                    .expect("conversation")
                    .conversation
                    .revision,
                expected_turn_revision: preparing.revision,
                operation: operation("initial-prepare"),
                model: model.clone(),
                attributions: Default::default(),
            },
            TimestampMillis::new(1_013),
        )
        .expect("prepare")
        .value;
    database
        .append_event(
            sent.turn.id,
            prepared.revision,
            &operation("initial-running"),
            GenerationCheckpointEnvelope {
                turn_id: sent.turn.id,
                attempt_id: sent.attempt.id,
                job_id: Some(job.id),
                correlation_id: None,
                sequence: 2,
                event: GenerationCheckpointEvent::Stage {
                    status: GenerationTurnStatus::Running,
                },
            },
            TimestampMillis::new(1_014),
        )
        .expect("running");
    let mut profile = ModelProfileRepository::get(database, model_id)
        .expect("profile")
        .expect("profile");
    profile.config.chat_parameters.temperature = None;
    let account = ProviderAccountRepository::get(database, model.provider_account_id)
        .expect("account")
        .expect("account");
    let profile = lettuce_models::resolve_chat_profile(
        &model.expected_chat_identity(),
        &profile,
        &account,
        &Default::default(),
        &Default::default(),
    )
    .expect("resolved profile");
    (
        conversation.id,
        InferenceRequest {
            turn_id: sent.turn.id,
            attempt_id: sent.attempt.id,
            operation: lettuce_conversations::GenerationOperation::Send,
            profile: ResolvedInferenceProfile {
                chat_profile: profile,
                tool_policy: ToolPolicy::Allowed,
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
            cancellation: Some(job.id),
            stream_sink: None,
            media_grants: vec![],
            tools: Some(dynamic_memory_tool_request()),
        },
        JobHandle::new(job.id),
    )
}

fn outcome() -> InferenceOutcome {
    InferenceOutcome {
        provider_response_id: Some("initial-response-id".into()),
        candidates: vec![InferenceCandidate {
            ordinal: 0,
            parts: vec![MessagePart::Text {
                text: "Saved context".into(),
            }],
            tool_calls: vec![],
            provider_replay: None,
        }],
        usage: Some(InferenceUsage {
            input_tokens: 20,
            output_tokens: 5,
            cached_input_tokens: Some(3),
            reasoning_tokens: Some(2),
            cache_write_tokens: Some(1),
            web_search_requests: Some(0),
            provider_reported_cost: lettuce_conversations::ProviderReportedCost::new(0.001),
        }),
        finish_reason: lettuce_conversations::FinishReason::Stop,
        provider_finish_reason: Some("stop".into()),
        provider_request_id: Some("initial-request-id".into()),
        warning_codes: vec![lettuce_conversations::InferenceWarningCode::ProviderDegraded],
    }
}

#[tokio::test]
async fn initial_checkpoint_reopens_with_exact_response_usage_and_signed_replay() {
    let path = std::env::temp_dir().join(format!("lettuce-initial-{}.db", ConversationId::new()));
    let database = Database::open(&path).expect("database");
    let (conversation_id, request, handle) = fixture(&database);
    let bytes = lettuce_conversations::ProtectedArtifactBytes::new(
        b"[{\"signature\":\"signed\"}]".to_vec(),
    )
    .expect("bytes");
    let draft = lettuce_conversations::ReplayArtifactDraft {
        artifact_id: lettuce_types::ReplayArtifactId::new(),
        codec: lettuce_conversations::ArtifactCodec::Json,
        retention: lettuce_conversations::ArtifactRetention::Conversation,
        schema_version: 1,
        digest: bytes.digest(),
        byte_size: bytes.len() as u64,
        bytes,
    };
    let reference = database.stage_provider_replay(draft).expect("stage replay");
    let mut expected = outcome();
    expected.candidates[0].provider_replay = Some(reference.clone());
    expected.candidates[0].tool_calls.push(ProposedToolCall {
        provider_call_id: Some("create-tea".into()),
        name: "create_memory".into(),
        arguments: serde_json::json!({"text":"Tea is preferred", "category":"preference"}),
        raw_arguments: None,
        provider_replay: Some(reference.clone()),
    });
    let inference = ScriptedInference {
        outcomes: Mutex::new(VecDeque::from([expected.clone()])),
        requests: Mutex::new(vec![]),
    };
    let coordinator = crate::ConversationInitialInferenceCoordinator::new(&database, &inference);
    assert_eq!(
        coordinator
            .run(
                conversation_id,
                &handle,
                request.clone(),
                TimestampMillis::new(1_020)
            )
            .await
            .expect("initial response"),
        expected
    );
    let binding =
        InitialInferenceBinding::from_request(conversation_id, &request).expect("binding");
    let record = database
        .initial_inference(&binding)
        .expect("record")
        .expect("saved");
    assert_eq!(
        database
            .settle_initial_inference(
                &binding,
                &InitialInferenceResult::Response(expected.clone()),
                TimestampMillis::new(1_025)
            )
            .expect("exact settlement replay"),
        record
    );
    let mut changed_outcome = expected.clone();
    changed_outcome.provider_request_id = Some("changed-request".into());
    assert!(matches!(
        database.settle_initial_inference(
            &binding,
            &InitialInferenceResult::Response(changed_outcome),
            TimestampMillis::new(1_025)
        ),
        Err(lettuce_conversations::ConversationRepositoryError::Conflict)
    ));
    let evidence = database.job_usage(handle.id()).expect("usage");
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].id, record.usage_event_id);
    assert_eq!(
        evidence[0].result,
        Some(JobInferenceUsageResult::Response {
            usage: expected.usage.clone(),
            provider_response_id: expected.provider_response_id.clone()
        })
    );
    for change in 0..4 {
        let mut changed = request.clone();
        match change {
            0 => {
                changed.context.messages[0].parts = vec![ProviderContextPart::Text {
                    text: "Different context".into(),
                }]
            }
            1 => changed.profile.chat_profile.parameters.temperature = Some(0.91),
            2 => {
                changed.tools.as_mut().expect("tools").choice =
                    lettuce_conversations::ToolChoice::Auto
            }
            _ => changed.media_grants.push(lettuce_types::AssetId::new()),
        }
        assert!(matches!(
            coordinator
                .run(
                    conversation_id,
                    &handle,
                    changed,
                    TimestampMillis::new(1_021)
                )
                .await,
            Err(crate::ConversationInitialInferenceError::Repository(
                lettuce_conversations::ConversationRepositoryError::Conflict
            ))
        ));
    }
    database
        .cleanup_orphan_provider_replay(reference.artifact_id)
        .expect("retained by checkpoint");
    database
        .materialize_provider_replay(&reference)
        .expect("signed bytes retained");
    drop(database);
    let database = Database::open(&path).expect("reopen");
    let mut reattached = request.clone();
    reattached.stream_sink = Some(RequestId::new());
    handle.request_cancel();
    assert_eq!(
        crate::ConversationInitialInferenceCoordinator::new(&database, &inference)
            .run(
                conversation_id,
                &handle,
                reattached,
                TimestampMillis::new(1_030)
            )
            .await
            .expect("replay before cancellation"),
        expected
    );
    assert_eq!(
        database.initial_inference(&binding).expect("record"),
        Some(record)
    );
    assert_eq!(
        database.job_usage(handle.id()).expect("usage unchanged"),
        evidence
    );
    assert_eq!(inference.requests.lock().expect("requests").len(), 1);
    let admitted = lettuce_conversations::ConversationManager::new(&database)
        .request_tool_executions(
            lettuce_conversations::ToolExecutionOwner {
                conversation_id,
                turn_id: request.turn_id,
                attempt_id: request.attempt_id,
            },
            request.tools.as_ref().expect("tools"),
            expected.candidates[0].tool_calls.clone(),
            TimestampMillis::new(1_031),
        )
        .expect("admit saved calls");
    assert_eq!(admitted.len(), 1);
    database
        .materialize_provider_replay(&reference)
        .expect("replay remains materializable");
    drop(database);
    std::fs::remove_file(path).expect("remove database");
}

#[tokio::test]
async fn initial_pending_admission_blocks_concurrent_and_reopened_dispatch() {
    let path = std::env::temp_dir().join(format!("lettuce-pending-{}.db", ConversationId::new()));
    let database = Database::open(&path).expect("database");
    let (conversation_id, request, handle) = fixture(&database);
    let inference = BlockingInference::new(outcome());
    let coordinator = crate::ConversationInitialInferenceCoordinator::new(&database, &inference);
    {
        let run = coordinator.run(
            conversation_id,
            &handle,
            request.clone(),
            TimestampMillis::new(1_020),
        );
        tokio::pin!(run);
        tokio::select! {
            _ = inference.entered.notified() => {},
            result = &mut run => panic!("provider must still be pending: {result:?}"),
        }
        let second = Database::open(&path).expect("second database handle");
        assert!(matches!(
            crate::ConversationInitialInferenceCoordinator::new(&second, &inference)
                .run(
                    conversation_id,
                    &handle,
                    request.clone(),
                    TimestampMillis::new(1_021)
                )
                .await,
            Err(crate::ConversationInitialInferenceError::Pending)
        ));
        let binding =
            InitialInferenceBinding::from_request(conversation_id, &request).expect("binding");
        let pending = second
            .initial_inference(&binding)
            .expect("pending")
            .expect("admitted");
        assert!(pending.result.is_none());
        assert_eq!(
            second.job_usage(handle.id()).expect("raw evidence")[0].result,
            None
        );
        inference.release.notify_one();
        run.await.expect("first caller completes");
        assert_eq!(inference.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        drop(second);
    }
    drop(database);
    std::fs::remove_file(path).expect("remove database");
}

#[tokio::test]
async fn initial_provider_failures_and_cancellation_replay_without_redispatch() {
    for result in [
        Err(PortError::Unavailable),
        Err(PortError::Cancelled),
        Ok(InferenceOutcome {
            candidates: vec![],
            ..outcome()
        }),
    ] {
        let database = database();
        let (conversation_id, request, handle) = fixture(&database);
        let inference = FallibleScriptedInference {
            outcomes: Mutex::new(VecDeque::from([result])),
            requests: Mutex::new(vec![]),
        };
        let coordinator =
            crate::ConversationInitialInferenceCoordinator::new(&database, &inference);
        assert!(
            coordinator
                .run(
                    conversation_id,
                    &handle,
                    request.clone(),
                    TimestampMillis::new(1_020)
                )
                .await
                .is_err()
        );
        let binding =
            InitialInferenceBinding::from_request(conversation_id, &request).expect("binding");
        let record = database
            .initial_inference(&binding)
            .expect("saved failure")
            .expect("record");
        assert!(matches!(
            record.result,
            Some(InitialInferenceResult::Failed(_))
        ));
        assert!(
            coordinator
                .run(
                    conversation_id,
                    &handle,
                    request.clone(),
                    TimestampMillis::new(1_030)
                )
                .await
                .is_err()
        );
        assert_eq!(inference.requests.lock().expect("requests").len(), 1);
        assert_eq!(database.job_usage(handle.id()).expect("evidence").len(), 1);
    }
}

#[tokio::test]
async fn initial_checkpoint_failure_stays_pending_until_a_fresh_recovery_attempt() {
    let path = std::env::temp_dir().join(format!(
        "lettuce-initial-recovery-{}.db",
        ConversationId::new()
    ));
    let database = Database::open(&path).expect("database");
    let (conversation_id, request, handle) = fixture(&database);
    let mut missing_replay = outcome();
    missing_replay.candidates[0].provider_replay = Some(lettuce_conversations::ReplayArtifactRef {
        artifact_id: lettuce_types::ReplayArtifactId::new(),
        digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
        schema_version: 1,
        byte_size: 4,
        codec: lettuce_conversations::ReplayCodec::Json,
        retention: lettuce_conversations::ReplayRetention::Conversation,
    });
    let inference = ScriptedInference {
        outcomes: Mutex::new(VecDeque::from([missing_replay, outcome()])),
        requests: Mutex::new(vec![]),
    };
    let coordinator = crate::ConversationInitialInferenceCoordinator::new(&database, &inference);
    assert!(matches!(
        coordinator
            .run(
                conversation_id,
                &handle,
                request.clone(),
                TimestampMillis::new(1_020)
            )
            .await,
        Err(crate::ConversationInitialInferenceError::Repository(_))
    ));
    let binding =
        InitialInferenceBinding::from_request(conversation_id, &request).expect("binding");
    let pending = database
        .initial_inference(&binding)
        .expect("pending")
        .expect("record");
    assert!(pending.result.is_none());
    let evidence = database.job_usage(handle.id()).expect("response usage");
    assert!(matches!(
        evidence[0].result,
        Some(JobInferenceUsageResult::Response { .. })
    ));
    drop(database);
    let database = Database::open(&path).expect("reopen pending");
    let coordinator = crate::ConversationInitialInferenceCoordinator::new(&database, &inference);
    assert!(matches!(
        coordinator
            .run(
                conversation_id,
                &handle,
                request.clone(),
                TimestampMillis::new(1_021)
            )
            .await,
        Err(crate::ConversationInitialInferenceError::Pending)
    ));
    assert_eq!(inference.requests.lock().expect("requests").len(), 1);
    let turn = database.get_turn(request.turn_id).expect("turn");
    let interrupted = database
        .interrupt_generation(
            request.turn_id,
            request.attempt_id,
            ConversationReader::get(&database, conversation_id)
                .expect("conversation")
                .conversation
                .revision,
            turn.revision,
            &operation("initial-interrupt"),
            UsageEventId::new(),
            TimestampMillis::new(1_022),
        )
        .expect("interrupt parent")
        .value;
    let child = database
        .recover_generation(
            request.turn_id,
            request.attempt_id,
            ConversationReader::get(&database, conversation_id)
                .expect("conversation")
                .conversation
                .revision,
            interrupted.revision,
            &operation("initial-recover"),
            TimestampMillis::new(1_023),
        )
        .expect("recover child")
        .value;
    let job = database
        .create_or_get(
            lettuce_jobs::JobSpec::new(
                JobKind::MemoryExtraction,
                lettuce_jobs::JobSubject::new(
                    lettuce_jobs::SubjectKind::Conversation,
                    conversation_id.to_string(),
                )
                .expect("subject"),
                OutcomeRef::GenerationTurn(turn.id),
            )
            .with_resources(vec![ResourceClass::Network]),
        )
        .expect("child job")
        .job;
    database
        .attach_attempt_job(
            &AttachAttemptJob {
                conversation_id,
                turn_id: turn.id,
                attempt_id: child.attempt.id,
                expected_revision: ConversationReader::get(&database, conversation_id)
                    .expect("conversation")
                    .conversation
                    .revision,
                expected_turn_revision: child.turn.revision,
                operation: operation("initial-child-attach"),
                job_id: job.id,
            },
            TimestampMillis::new(1_024),
        )
        .expect("attach child");
    let turn = database.get_turn(turn.id).expect("attached child");
    database
        .append_event(
            turn.id,
            turn.revision,
            &operation("initial-child-running"),
            GenerationCheckpointEnvelope {
                turn_id: turn.id,
                attempt_id: child.attempt.id,
                job_id: Some(job.id),
                correlation_id: None,
                sequence: 1,
                event: GenerationCheckpointEvent::Stage {
                    status: GenerationTurnStatus::Running,
                },
            },
            TimestampMillis::new(1_025),
        )
        .expect("run recovered prepared turn");
    let child_request = InferenceRequest {
        attempt_id: child.attempt.id,
        cancellation: Some(job.id),
        ..request.clone()
    };
    assert_eq!(
        coordinator
            .run(
                conversation_id,
                &JobHandle::new(job.id),
                child_request,
                TimestampMillis::new(1_026)
            )
            .await
            .expect("fresh child dispatch"),
        outcome()
    );
    assert_eq!(inference.requests.lock().expect("requests").len(), 2);
    assert_eq!(
        database
            .initial_inference(&binding)
            .expect("retained parent"),
        Some(pending)
    );
    assert_eq!(
        database
            .job_usage(handle.id())
            .expect("retained parent usage"),
        evidence
    );
    drop(database);
    std::fs::remove_file(path).expect("remove database");
}

#[tokio::test]
async fn initial_cancellation_during_provider_retains_usage_without_response_replay() {
    let database = database();
    let (conversation_id, request, handle) = fixture(&database);
    let inference = BlockingInference::new(outcome());
    let coordinator = crate::ConversationInitialInferenceCoordinator::new(&database, &inference);
    let run = coordinator.run(
        conversation_id,
        &handle,
        request.clone(),
        TimestampMillis::new(1_020),
    );
    tokio::pin!(run);
    tokio::select! {
        _ = inference.entered.notified() => {},
        result = &mut run => panic!("provider must still be pending: {result:?}"),
    }
    handle.request_cancel();
    inference.release.notify_one();
    assert!(matches!(
        run.await,
        Err(crate::ConversationInitialInferenceError::Cancelled)
    ));
    let binding =
        InitialInferenceBinding::from_request(conversation_id, &request).expect("binding");
    let record = database
        .initial_inference(&binding)
        .expect("record")
        .expect("settled");
    assert_eq!(
        record.result,
        Some(InitialInferenceResult::Failed(PortError::Cancelled))
    );
    assert!(matches!(
        database.job_usage(handle.id()).expect("response evidence")[0].result,
        Some(JobInferenceUsageResult::Response { .. })
    ));
    assert!(matches!(
        coordinator
            .run(
                conversation_id,
                &JobHandle::new(handle.id()),
                request,
                TimestampMillis::new(1_030)
            )
            .await,
        Err(crate::ConversationInitialInferenceError::Cancelled)
    ));
    assert_eq!(inference.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}
