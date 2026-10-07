use super::tests::{Reply, harness, launch};
use super::turns_tests::replied_chat;
use super::*;
use lettuce_contracts::{self as dto, ApiErrorCode, ApiErrorDetails};

fn make_dynamic(harness: &super::tests::Harness, enabled: bool) {
    use lettuce_characters::CharacterRepository;
    use lettuce_settings::GlobalSettingsStore;
    let database = harness.context.backend().database();
    let character = CharacterRepository::get(database, harness.character_id)
        .expect("character")
        .expect("present")
        .character;
    if character.defaults.memory_policy != lettuce_characters::MemoryPolicy::Dynamic {
        let mut defaults = character.defaults;
        defaults.memory_policy = lettuce_characters::MemoryPolicy::Dynamic;
        CharacterRepository::update_defaults(
            database,
            harness.character_id,
            character.revision,
            defaults,
            harness.context.now(),
        )
        .expect("dynamic character");
    }
    let stored = GlobalSettingsStore::load(database).expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.enabled = enabled;
    settings.dynamic_memory.summary_message_interval = 2;
    GlobalSettingsStore::save(
        database,
        settings,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("dynamic settings");
}

fn trigger_request(chat: &str, key: &str) -> dto::MemoryTriggerRequest {
    dto::MemoryTriggerRequest {
        conversation_id: chat.into(),
        client_operation_id: key.into(),
    }
}

fn gate_of(error: &dto::ApiError) -> dto::MemoryGateReason {
    match &error.details {
        Some(ApiErrorDetails::MemoryGate { gate }) => *gate,
        other => panic!("expected a memory gate, got {other:?}"),
    }
}

#[tokio::test]
async fn forced_cycles_name_their_gates_and_queue_one_replayable_job() {
    use lettuce_jobs::{JobKind, JobState, JobStore};
    let harness = harness(Reply::Text("reply"));
    let (chat, _) = replied_chat(&harness, "forced").await;

    let manual = memory_trigger(&harness.context, trigger_request(&chat, "forced-manual"))
        .await
        .expect_err("manual memory chat");
    assert_eq!(manual.code, ApiErrorCode::Unsupported);
    assert_eq!(gate_of(&manual), dto::MemoryGateReason::NotDynamic);

    make_dynamic(&harness, false);
    let disabled = memory_trigger(&harness.context, trigger_request(&chat, "forced-disabled"))
        .await
        .expect_err("global switch off");
    assert_eq!(disabled.code, ApiErrorCode::Unsupported);
    assert_eq!(gate_of(&disabled), dto::MemoryGateReason::Disabled);

    make_dynamic(&harness, true);
    let empty = launch(&harness, "forced-empty").await;
    let nothing = memory_trigger(&harness.context, trigger_request(&empty, "forced-nothing"))
        .await
        .expect_err("no dialogue");
    assert_eq!(nothing.code, ApiErrorCode::Unavailable);
    assert_eq!(gate_of(&nothing), dto::MemoryGateReason::NothingToSummarise);

    let accepted = memory_trigger(&harness.context, trigger_request(&chat, "forced-start"))
        .await
        .expect("trigger");
    let database = harness.context.backend().database();
    let job = JobStore::get(database, accepted.job_id.parse().expect("job id"))
        .expect("job")
        .expect("stored");
    assert_eq!(job.kind, JobKind::MemoryExtraction);
    assert_eq!(job.state, JobState::Queued);
    assert_eq!(
        memory_trigger(&harness.context, trigger_request(&chat, "forced-start"))
            .await
            .expect("replay"),
        accepted
    );
    assert_eq!(
        memory_trigger(&harness.context, trigger_request(&empty, "forced-start"))
            .await
            .expect_err("changed request")
            .code,
        ApiErrorCode::Conflict
    );
    assert_eq!(
        memory_trigger(&harness.context, trigger_request(&chat, "forced-again"))
            .await
            .expect("the queued cycle for that window is returned"),
        accepted
    );
}

#[tokio::test]
async fn retry_names_the_gate_and_validates_the_chosen_model() {
    let harness = harness(Reply::Text("reply"));
    let (chat, _) = replied_chat(&harness, "retry").await;
    let error = memory_retry(
        &harness.context,
        dto::MemoryRetryRequest {
            conversation_id: chat.clone(),
            model_profile_id: None,
            client_operation_id: "retry-manual".into(),
        },
    )
    .await
    .expect_err("manual memory chat");
    assert_eq!(gate_of(&error), dto::MemoryGateReason::NotDynamic);
    make_dynamic(&harness, true);
    let unknown = memory_retry(
        &harness.context,
        dto::MemoryRetryRequest {
            conversation_id: chat.clone(),
            model_profile_id: Some(lettuce_types::ModelProfileId::new().to_string()),
            client_operation_id: "retry-unknown".into(),
        },
    )
    .await
    .expect_err("unknown model");
    assert_eq!(unknown.code, ApiErrorCode::NotFound);
    let model = lettuce_settings::GlobalSettingsStore::load(harness.context.backend().database())
        .expect("settings")
        .default_model_profile_id
        .expect("default model");
    let accepted = memory_retry(
        &harness.context,
        dto::MemoryRetryRequest {
            conversation_id: chat,
            model_profile_id: Some(model.to_string()),
            client_operation_id: "retry-model".into(),
        },
    )
    .await
    .expect("retry with a model");
    assert!(!accepted.job_id.is_empty());
}

#[tokio::test]
async fn skip_without_a_pending_approval_and_cancelling_idle_work_succeed_silently() {
    use lettuce_jobs::{JobState, JobStore};
    let harness = harness(Reply::Text("reply"));
    let (chat, _) = replied_chat(&harness, "skip").await;
    make_dynamic(&harness, true);
    let skip = dto::MemorySkipRequest {
        conversation_id: chat.clone(),
        client_operation_id: "skip-nothing".into(),
    };
    memory_skip(&harness.context, skip.clone())
        .await
        .expect("nothing pending");
    memory_skip(&harness.context, skip.clone())
        .await
        .expect("replay");
    assert_eq!(
        memory_skip(
            &harness.context,
            dto::MemorySkipRequest {
                conversation_id: launch(&harness, "skip-other").await,
                ..skip
            }
        )
        .await
        .expect_err("changed request")
        .code,
        ApiErrorCode::Conflict
    );
    let accepted = memory_trigger(&harness.context, trigger_request(&chat, "skip-trigger"))
        .await
        .expect("trigger");
    let cancel = dto::JobCancelRequest {
        job_id: accepted.job_id.clone(),
    };
    job_cancel(&harness.context, cancel.clone())
        .await
        .expect("cancel the queued cycle");
    job_cancel(&harness.context, cancel)
        .await
        .expect("cancelling an ended cycle succeeds silently");
    let job = JobStore::get(
        harness.context.backend().database(),
        accepted.job_id.parse().expect("job id"),
    )
    .expect("job")
    .expect("stored");
    assert_eq!(job.state, JobState::Cancelled);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_triggered_cycle_runs_through_the_job_runner_and_fails_typed_without_a_model() {
    use lettuce_settings::GlobalSettingsStore;
    let harness = harness(Reply::Text("reply"));
    let (chat, _) = replied_chat(&harness, "runner").await;
    make_dynamic(&harness, true);
    let database = harness.context.backend().database();
    let stored = GlobalSettingsStore::load(database).expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.run_mode = lettuce_settings::MemoryRunMode::Manual;
    GlobalSettingsStore::save(database, settings, None, stored.revision)
        .expect("manual runs and no default model");
    let workers = startup(&harness.context).await.expect("startup");
    workers.started().await;
    let accepted = memory_trigger(&harness.context, trigger_request(&chat, "runner-trigger"))
        .await
        .expect("trigger");
    let settled = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        harness.events.until(|events| {
            events.iter().any(|event| {
                matches!(event, dto::ApiEvent::JobUpdated { job }
                    if job.id == accepted.job_id && job.state == dto::JobStateDto::Failed)
            })
        }),
    )
    .await;
    workers.stop().await;
    assert!(settled.is_ok(), "the cycle never settled");
    let view = memory_get(
        &harness.context,
        dto::ConversationRequest {
            conversation_id: chat,
        },
    )
    .await
    .expect("status");
    assert_eq!(view.status.latest_job_id, Some(accepted.job_id));
    assert_eq!(
        view.status.latest_cycle_status,
        Some(dto::MemoryCycleStatus::Failed)
    );
    assert_eq!(
        view.status.failure,
        Some(dto::MemoryFailureCode::ModelMissing)
    );
    assert_eq!(
        job_get(
            &harness.context,
            dto::JobGetRequest {
                job_id: view.status.latest_job_id.expect("job"),
            }
        )
        .await
        .expect("job")
        .state,
        dto::JobStateDto::Failed
    );
}

#[tokio::test]
async fn skip_answers_an_ask_first_approval_and_a_trigger_clears_it() {
    use lettuce_jobs::{ResourceAvailability, WorkerId};
    use lettuce_settings::GlobalSettingsStore;
    for skip in [true, false] {
        let harness = harness(Reply::Text("reply"));
        let (chat, _) = replied_chat(&harness, &format!("approval-{skip}")).await;
        make_dynamic(&harness, true);
        let database = harness.context.backend().database();
        let stored = GlobalSettingsStore::load(database).expect("settings");
        let mut settings = stored.settings;
        settings.dynamic_memory.run_mode = lettuce_settings::MemoryRunMode::AskFirst;
        GlobalSettingsStore::save(
            database,
            settings,
            stored.default_model_profile_id,
            stored.revision,
        )
        .expect("ask first");
        let embedding = harness.context.embedding();
        let prompted = harness
            .context
            .backend()
            .companion_memory_host(embedding.as_ref(), harness.context.inference())
            .after_turn(
                chat.parse().expect("id"),
                lettuce_conversations::GenerationOperation::Send,
                WorkerId::new(),
                harness.context.now(),
                std::time::Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("prompt");
        assert!(prompted.is_empty());
        let request = dto::ConversationRequest {
            conversation_id: chat.clone(),
        };
        let pending = memory_get(&harness.context, request.clone())
            .await
            .expect("pending")
            .status;
        assert!(pending.pending_approval_count.is_some());
        assert!(!pending.skipped);
        if skip {
            memory_skip(
                &harness.context,
                dto::MemorySkipRequest {
                    conversation_id: chat.clone(),
                    client_operation_id: "approval-skip".into(),
                },
            )
            .await
            .expect("skip");
        } else {
            memory_trigger(&harness.context, trigger_request(&chat, "approval-start"))
                .await
                .expect("start");
        }
        let answered = memory_get(&harness.context, request)
            .await
            .expect("answered")
            .status;
        assert_eq!(answered.pending_approval_count, None);
        assert_eq!(answered.skipped, skip);
    }
}

struct RecordedCycle {
    run_id: String,
    item: String,
}

async fn recorded_cycle(
    harness: &super::tests::Harness,
    chat: &str,
    summary: &str,
    text: &str,
    at: i64,
    start: u64,
) -> RecordedCycle {
    use lettuce_conversations::{ConversationOverviewReader, ConversationReader};
    use lettuce_jobs::{
        JobKind, JobSpec, JobStore, JobSubject, OutcomeRef, ResourceClass, SubjectKind,
    };
    use lettuce_memory::{
        DynamicMemoryAttemptStatus, DynamicMemoryBackgroundRoundCommit,
        DynamicMemoryInferenceRound, DynamicMemoryRoundFinishReason, DynamicMemoryRoundKind,
        DynamicMemoryRunRepository, DynamicMemorySourceMessage, DynamicMemorySummaryCommit,
        ListedMemory, MemoryChangeSet, MemoryItem, MemoryRepository, MemoryShortId,
        MemoryToolOutcome, MemoryToolResult, NewDynamicMemoryInferenceRound,
        NewDynamicMemoryRunAttempt, NewDynamicMemoryToolCall,
    };
    use lettuce_types::{
        DynamicMemoryAttemptId, DynamicMemoryRunId, MemoryId, TimestampMillis, ToolExecutionId,
    };
    let _ = std::any::type_name::<(DynamicMemoryInferenceRound, DynamicMemoryRoundFinishReason)>();
    let database = harness.context.backend().database();
    let conversation_id: lettuce_types::ConversationId = chat.parse().expect("conversation");
    let branch_id = ConversationReader::get(database, conversation_id)
        .expect("conversation")
        .conversation
        .active_branch_id;
    let view = super::turns_tests::open(harness, chat).await;
    let mut sources = Vec::new();
    for message in view.messages.items.iter().rev() {
        let item = ConversationOverviewReader::timeline_anchor(
            database,
            conversation_id,
            branch_id,
            message.id.parse().expect("message"),
        )
        .expect("anchor")
        .item;
        sources.push(DynamicMemorySourceMessage {
            message_id: item.message.id,
            role: item.message.role,
            render_source: item.message.active_render_source,
            effective_time: item.message.effective_time,
        });
    }
    let memory = MemoryRepository::get_for_branch(database, conversation_id, branch_id)
        .expect("memory")
        .expect("space");
    let job = JobStore::create_or_get(
        database,
        JobSpec::new(
            JobKind::MemoryExtraction,
            JobSubject::new(SubjectKind::Conversation, format!("{chat}-{at}")).expect("subject"),
            OutcomeRef::Conversation(conversation_id),
        )
        .with_resources(vec![ResourceClass::Cpu]),
    )
    .expect("job")
    .job;
    let run_id = DynamicMemoryRunId::new();
    let attempt_id = DynamicMemoryAttemptId::new();
    let admitted = database
        .admit_dynamic_memory_run_attempt(NewDynamicMemoryRunAttempt {
            run_id,
            attempt_id,
            conversation_id,
            branch_id,
            space_id: memory.id,
            cycle_start_change: None,
            starting_memory: memory.clone(),
            source_messages: sources.clone(),
            profile: crate::companion::companion_memory_run::tests::profile(),
            time_awareness_enabled: false,
            supersession_enabled: false,
            structured_fallback_format: lettuce_memory::DynamicMemoryStructuredFallbackFormat::Xml,
            summary_window: lettuce_memory::DynamicMemorySummaryWindow {
                message_interval: 2,
                start,
                end: start + u64::try_from(sources.len()).expect("count"),
            },
            tool_request: lettuce_memory::dynamic_memory_tool_request_for_run(
                lettuce_memory::DynamicMemoryToolOptions {
                    group: false,
                    supersession_enabled: false,
                    require_source_message_id: false,
                },
                &|key| key.to_owned(),
            ),
            job_id: job.id,
            job_attempt: None,
            now: TimestampMillis::new(at),
        })
        .expect("run");
    let processing = database
        .transition_dynamic_memory_attempt(
            attempt_id,
            admitted.attempt.revision,
            DynamicMemoryAttemptStatus::Processing,
            None,
            TimestampMillis::new(at),
        )
        .expect("processing");
    let context = lettuce_conversations::ProviderNeutralContext {
        messages: vec![lettuce_conversations::ProviderNeutralMessage {
            role: lettuce_conversations::MessageRole::User,
            parts: vec![lettuce_conversations::ProviderContextPart::Text {
                text: "summary request".into(),
            }],
        }],
        attributions: Default::default(),
        budget: Default::default(),
    };
    database
        .commit_dynamic_memory_summary(
            DynamicMemorySummaryCommit {
                run_id,
                attempt_id,
                expected_memory_revision: memory.revision,
                text: summary.into(),
                token_count: 5,
                request_context: context,
                usage: None,
                provider_request_id: None,
            },
            TimestampMillis::new(at + 1),
        )
        .expect("summary checkpoint");
    let call_id = ToolExecutionId::new();
    database
        .admit_dynamic_memory_inference_round(
            run_id,
            attempt_id,
            0,
            0,
            NewDynamicMemoryInferenceRound {
                ordinal: 0,
                request_context: lettuce_conversations::ProviderNeutralContext {
                    messages: vec![],
                    attributions: Default::default(),
                    budget: Default::default(),
                },
                parts: vec![],
                provider_replay: None,
                usage: None,
                finish_reason: DynamicMemoryRoundFinishReason::Stop,
                kind: DynamicMemoryRoundKind::Manager,
                provider_request_id: None,
                calls: vec![NewDynamicMemoryToolCall {
                    id: call_id,
                    definition_version: 1,
                    call: lettuce_conversations::ProposedToolCall {
                        provider_call_id: Some("create".into()),
                        name: "create_memory".into(),
                        arguments: serde_json::json!({"text": text}),
                        raw_arguments: None,
                        provider_replay: None,
                    },
                }],
                admitted_at: TimestampMillis::new(at + 2),
            },
        )
        .expect("round");
    let current = MemoryRepository::get(database, memory.id)
        .expect("memory")
        .expect("space");
    let id = MemoryId::new();
    let item = MemoryItem::written(
        id,
        MemoryShortId::derived(id),
        text.into(),
        TimestampMillis::new(at + 3),
    );
    let mut items = current.items.clone();
    items.push(item.clone());
    database
        .commit_dynamic_memory_background_round(
            DynamicMemoryBackgroundRoundCommit {
                run_id,
                attempt_id,
                round_ordinal: 0,
                space_id: memory.id,
                expected_memory_revision: current.revision,
                change: Some(MemoryChangeSet {
                    space_id: memory.id,
                    expected_revision: current.revision,
                    items,
                }),
                results: vec![MemoryToolResult {
                    execution_id: call_id,
                    outcome: MemoryToolOutcome::Created {
                        id,
                        short_id: item.short_id,
                        memories: vec![ListedMemory {
                            short_id: item.short_id,
                            text: text.into(),
                        }],
                    },
                }],
            },
            TimestampMillis::new(at + 3),
        )
        .expect("create tool");
    database
        .transition_dynamic_memory_attempt(
            attempt_id,
            processing.revision,
            DynamicMemoryAttemptStatus::Succeeded,
            None,
            TimestampMillis::new(at + 4),
        )
        .expect("finish");
    RecordedCycle {
        run_id: run_id.to_string(),
        item: id.to_string(),
    }
}

#[tokio::test]
async fn the_activity_log_lists_outcomes_pages_newest_first_and_names_blockers() {
    let harness = harness(Reply::Text("reply"));
    let (chat, _) = replied_chat(&harness, "log").await;
    make_dynamic(&harness, true);
    let first = recorded_cycle(&harness, &chat, "First summary", "First fact", 1_000, 0).await;
    let second = recorded_cycle(&harness, &chat, "Second summary", "Second fact", 2_000, 2).await;
    let page = memory_cycles(
        &harness.context,
        dto::MemoryCyclesRequest {
            conversation_id: chat.clone(),
            cursor: None,
            limit: Some(1),
        },
    )
    .await
    .expect("first page");
    assert_eq!(page.items.len(), 1);
    let newest = &page.items[0];
    assert_eq!(newest.run_id, second.run_id);
    assert_eq!(newest.label, "2-4");
    assert_eq!(newest.status, dto::MemoryCycleStatus::Complete);
    assert_eq!(newest.summary.as_deref(), Some("Second summary"));
    assert!(newest.revertable);
    assert_eq!(newest.blocked_by, None);
    assert_eq!(newest.actions.len(), 1);
    assert_eq!(newest.actions[0].kind, dto::MemoryCycleActionKind::Created);
    assert_eq!(
        newest.actions[0].memory_id.as_deref(),
        Some(second.item.as_str())
    );
    assert_eq!(newest.actions[0].text.as_deref(), Some("Second fact"));
    let rest = memory_cycles(
        &harness.context,
        dto::MemoryCyclesRequest {
            conversation_id: chat.clone(),
            cursor: page.next_cursor.clone(),
            limit: Some(5),
        },
    )
    .await
    .expect("second page");
    assert_eq!(rest.next_cursor, None);
    assert_eq!(rest.items.len(), 1);
    assert_eq!(rest.items[0].run_id, first.run_id);
    assert!(!rest.items[0].revertable);
    assert_eq!(rest.items[0].blocked_by, Some(second.run_id));
    assert_eq!(
        memory_cycles(
            &harness.context,
            dto::MemoryCyclesRequest {
                conversation_id: chat,
                cursor: Some(lettuce_types::DynamicMemoryRunId::new().to_string()),
                limit: None,
            },
        )
        .await
        .expect_err("unknown cursor")
        .code,
        ApiErrorCode::InvalidInput
    );
}

#[tokio::test]
async fn reverting_a_cycle_replays_conflicts_and_names_the_dependent_later_cycle() {
    let harness = harness(Reply::Text("reply"));
    let (chat, _) = replied_chat(&harness, "revert").await;
    make_dynamic(&harness, true);
    let first = recorded_cycle(&harness, &chat, "First summary", "First fact", 1_000, 0).await;
    let second = recorded_cycle(&harness, &chat, "Second summary", "Second fact", 2_000, 2).await;
    let view = || async {
        memory_get(
            &harness.context,
            dto::ConversationRequest {
                conversation_id: chat.clone(),
            },
        )
        .await
        .expect("memory")
    };
    let before = view().await;
    let request = |run: &str, revision: u64, key: &str| dto::MemoryCycleRevertRequest {
        conversation_id: chat.clone(),
        run_id: run.into(),
        expected_revision: revision,
        client_operation_id: key.into(),
    };
    let blocked = memory_cycle_revert(
        &harness.context,
        request(&first.run_id, before.revision, "revert-middle"),
    )
    .await
    .expect_err("a later cycle depends on it");
    assert_eq!(blocked.code, ApiErrorCode::Conflict);
    assert_eq!(
        blocked.details,
        Some(ApiErrorDetails::MemoryCycleDependent {
            later_run_id: second.run_id.clone()
        })
    );
    assert_eq!(view().await, before);
    assert_eq!(
        memory_cycle_revert(
            &harness.context,
            request(&second.run_id, before.revision + 1, "revert-stale"),
        )
        .await
        .expect_err("stale revision")
        .code,
        ApiErrorCode::Conflict
    );
    let reverted = memory_cycle_revert(
        &harness.context,
        request(&second.run_id, before.revision, "revert-latest"),
    )
    .await
    .expect("revert the latest cycle");
    assert_eq!(reverted.revision, before.revision + 1);
    assert_eq!(
        memory_cycle_revert(
            &harness.context,
            request(&second.run_id, before.revision, "revert-latest"),
        )
        .await
        .expect("replay"),
        reverted
    );
    assert_eq!(
        memory_cycle_revert(
            &harness.context,
            request(&first.run_id, before.revision, "revert-latest"),
        )
        .await
        .expect_err("changed request")
        .code,
        ApiErrorCode::Conflict
    );
    let after = view().await;
    assert_eq!(after.revision, reverted.revision);
    assert!(after.items.iter().all(|item| item.id != second.item));
    assert!(after.items.iter().any(|item| item.id == first.item));
    assert_eq!(after.summary.expect("summary").text, "First summary");
    let log = memory_cycles(
        &harness.context,
        dto::MemoryCyclesRequest {
            conversation_id: chat.clone(),
            cursor: None,
            limit: None,
        },
    )
    .await
    .expect("log");
    assert!(log.items[0].reverted);
    assert!(!log.items[0].revertable);
    assert!(log.items[1].revertable);
    assert_eq!(
        memory_cycle_revert(
            &harness.context,
            request(&second.run_id, reverted.revision, "revert-again"),
        )
        .await
        .expect_err("already reverted")
        .code,
        ApiErrorCode::Conflict
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn dismissing_a_failure_hides_it_until_a_newer_cycle_fails() {
    use lettuce_settings::GlobalSettingsStore;
    let harness = harness(Reply::Text("reply"));
    let (chat, _) = replied_chat(&harness, "dismiss").await;
    make_dynamic(&harness, true);
    let database = harness.context.backend().database();
    let stored = GlobalSettingsStore::load(database).expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.run_mode = lettuce_settings::MemoryRunMode::Manual;
    GlobalSettingsStore::save(database, settings, None, stored.revision).expect("no model");
    let workers = startup(&harness.context).await.expect("startup");
    workers.started().await;
    let accepted = memory_trigger(&harness.context, trigger_request(&chat, "dismiss-trigger"))
        .await
        .expect("trigger");
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        harness.events.until(|events| {
            events.iter().any(|event| {
                matches!(event, dto::ApiEvent::JobUpdated { job }
                    if job.id == accepted.job_id && job.state == dto::JobStateDto::Failed)
            })
        }),
    )
    .await
    .expect("the cycle failed");
    workers.stop().await;
    let read = || async {
        memory_get(
            &harness.context,
            dto::ConversationRequest {
                conversation_id: chat.clone(),
            },
        )
        .await
        .expect("memory")
        .status
    };
    assert_eq!(
        read().await.failure,
        Some(dto::MemoryFailureCode::ModelMissing)
    );
    let dismiss = dto::MemoryErrorDismissRequest {
        conversation_id: chat.clone(),
        client_operation_id: "dismiss-failure".into(),
    };
    memory_error_dismiss(&harness.context, dismiss.clone())
        .await
        .expect("dismiss");
    let hidden = read().await;
    assert_eq!(hidden.failure, None);
    assert_eq!(hidden.latest_cycle_status, None);
    assert_eq!(hidden.latest_job_id, None);
    memory_error_dismiss(&harness.context, dismiss.clone())
        .await
        .expect("replay");
    assert_eq!(
        memory_error_dismiss(
            &harness.context,
            dto::MemoryErrorDismissRequest {
                conversation_id: launch(&harness, "dismiss-other").await,
                ..dismiss
            },
        )
        .await
        .expect_err("changed request")
        .code,
        ApiErrorCode::Conflict
    );
    memory_error_dismiss(
        &harness.context,
        dto::MemoryErrorDismissRequest {
            conversation_id: chat,
            client_operation_id: "dismiss-nothing".into(),
        },
    )
    .await
    .expect("nothing to dismiss succeeds silently");
}

#[tokio::test(flavor = "multi_thread")]
async fn cycle_reverts_and_dismissed_failures_round_trip_through_backup() {
    use lettuce_settings::GlobalSettingsStore;
    use lettuce_transfer::{ProviderBackupRestoreWriter, ProviderBackupSource};
    let harness = harness(Reply::Text("reply"));
    let (chat, _) = replied_chat(&harness, "backup").await;
    make_dynamic(&harness, true);
    let database = harness.context.backend().database();
    let stored = GlobalSettingsStore::load(database).expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.run_mode = lettuce_settings::MemoryRunMode::Manual;
    GlobalSettingsStore::save(database, settings, None, stored.revision).expect("no model");
    let cycle = recorded_cycle(
        &harness,
        &chat,
        "Backed up summary",
        "Backed up fact",
        1_000,
        0,
    )
    .await;
    let revision = memory_get(
        &harness.context,
        dto::ConversationRequest {
            conversation_id: chat.clone(),
        },
    )
    .await
    .expect("memory")
    .revision;
    memory_cycle_revert(
        &harness.context,
        dto::MemoryCycleRevertRequest {
            conversation_id: chat.clone(),
            run_id: cycle.run_id.clone(),
            expected_revision: revision,
            client_operation_id: "backup-revert".into(),
        },
    )
    .await
    .expect("revert");
    let workers = startup(&harness.context).await.expect("startup");
    workers.started().await;
    let accepted = memory_trigger(&harness.context, trigger_request(&chat, "backup-trigger"))
        .await
        .expect("trigger");
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        harness.events.until(|events| {
            events.iter().any(|event| {
                matches!(event, dto::ApiEvent::JobUpdated { job }
                    if job.id == accepted.job_id && job.state == dto::JobStateDto::Failed)
            })
        }),
    )
    .await
    .expect("the cycle failed");
    workers.stop().await;
    memory_error_dismiss(
        &harness.context,
        dto::MemoryErrorDismissRequest {
            conversation_id: chat,
            client_operation_id: "backup-dismiss".into(),
        },
    )
    .await
    .expect("dismiss");
    let mut graph = database.read_provider_backup_graph().expect("export");
    lettuce_transfer::canonicalize_and_validate(&mut graph).expect("valid");
    assert!(graph.dynamic_memory.runs.iter().any(|run| !run.changed_item_ids.is_empty()));
    assert_eq!(graph.dynamic_memory.cycle_reverts.len(), 1);
    assert_eq!(graph.memory.error_dismissals.len(), 1);
    let restored = lettuce_database::Database::open_in_memory().expect("target");
    use lettuce_conversations::{ConversationArtifactTransferPort, TrustedArtifactDescriptor};
    let artifacts = lettuce_transfer::provider_backup_artifact_requirements(&graph)
        .expect("requirements")
        .into_iter()
        .map(|descriptor| {
            let mut sink = super::memory_tests::MemoryArtifactBytes(Vec::new());
            match &descriptor {
                TrustedArtifactDescriptor::Snapshot(reference) => {
                    database.export_snapshot(reference.artifact_id, &mut sink)
                }
                TrustedArtifactDescriptor::Replay(reference) => {
                    database.export_replay(reference.artifact_id, &mut sink)
                }
            }
            .expect("export artifact");
            lettuce_transfer::BackupConversationArtifact {
                descriptor,
                bytes: zeroize::Zeroizing::new(sink.0),
            }
        })
        .collect::<Vec<_>>();
    restored
        .restore_provider_backup_graph(&graph, &artifacts)
        .expect("restore");
    let mut again = restored.read_provider_backup_graph().expect("re-export");
    lettuce_transfer::canonicalize_and_validate(&mut again).expect("valid again");
    assert_eq!(
        again.dynamic_memory.cycle_reverts,
        graph.dynamic_memory.cycle_reverts
    );
    assert_eq!(
        again.dynamic_memory.runs.iter().map(|entry| (entry.run.id, &entry.changed_item_ids)).collect::<Vec<_>>(),
        graph.dynamic_memory.runs.iter().map(|entry| (entry.run.id, &entry.changed_item_ids)).collect::<Vec<_>>(),
    );
    assert_eq!(again.memory.error_dismissals, graph.memory.error_dismissals);
    let mut forged = graph.clone();
    forged.dynamic_memory.cycle_reverts[0].space_id = lettuce_types::MemorySpaceId::new();
    assert!(lettuce_transfer::canonicalize_and_validate(&mut forged).is_err());
    let mut dangling = graph;
    dangling.memory.error_dismissals[0].space_id = lettuce_types::MemorySpaceId::new();
    assert!(lettuce_transfer::canonicalize_and_validate(&mut dangling).is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_duplicated_companion_chat_shows_the_pools_memory_status_not_its_private_state() {
    use lettuce_companions::{
        CompanionStateOwner, CompanionStateReplacement, CompanionStateRepository,
    };
    use lettuce_settings::GlobalSettingsStore;
    let harness = harness(Reply::Text("reply"));
    let database = harness.context.backend().database();
    let character_id = super::tests::create_character(
        database,
        "Pooled Companion",
        lettuce_characters::CharacterDefaults {
            interaction_mode: lettuce_characters::InteractionMode::Companion,
            companion_soul: Some(lettuce_companions::CompanionSoulConfig::default()),
            memory_policy: lettuce_characters::MemoryPolicy::Dynamic,
            ..lettuce_characters::CharacterDefaults::default()
        },
    );
    let source = conversation_launch_direct(
        &harness.context,
        dto::LaunchDirectRequest {
            character_id: character_id.to_string(),
            title: None,
            scene_id: None,
            starter_id: None,
            client_operation_id: "pool-source".into(),
        },
    )
    .await
    .expect("source companion")
    .conversation_id;
    let first = conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: source.clone(),
            text: "I like tea".into(),
            expected_revision: 1,
            client_operation_id: "pool-first".into(),
        },
    )
    .await
    .expect("first message");
    conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: source.clone(),
            text: "And I keep bees".into(),
            expected_revision: first.revision,
            client_operation_id: "pool-second".into(),
        },
    )
    .await
    .expect("second message");
    let copy = conversation_duplicate(
        &harness.context,
        dto::ConversationDuplicateRequest {
            conversation_id: source.clone(),
            title: None,
            with_messages: true,
            client_operation_id: "pool-duplicate".into(),
        },
    )
    .await
    .expect("duplicate")
    .conversation_id;
    let owner = CompanionStateOwner {
        conversation_id: copy.parse().expect("copy id"),
        character_id,
        persona_id: None,
    };
    let private = CompanionStateRepository::get(database, owner)
        .expect("private state")
        .expect("exists");
    let mut changed = private.state.clone();
    changed.relationship_state.closeness = -0.31;
    CompanionStateRepository::replace(
        database,
        owner,
        lettuce_types::OperationRecordId::new(),
        CompanionStateReplacement {
            expected_session_revision: private.session_revision,
            expected_relationship_revision: private.relationship_revision,
            state: changed.clone(),
            applied_at: harness.context.now(),
        },
    )
    .expect("diverge the private state");
    make_dynamic(&harness, true);
    let stored = GlobalSettingsStore::load(database).expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.run_mode = lettuce_settings::MemoryRunMode::Manual;
    GlobalSettingsStore::save(database, settings, None, stored.revision).expect("no model");
    let workers = startup(&harness.context).await.expect("startup");
    workers.started().await;
    let accepted = memory_trigger(&harness.context, trigger_request(&source, "pool-trigger"))
        .await
        .expect("trigger in the source chat");
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        harness.events.until(|events| {
            events.iter().any(|event| {
                matches!(event, dto::ApiEvent::JobUpdated { job }
                    if job.id == accepted.job_id && job.state == dto::JobStateDto::Failed)
            })
        }),
    )
    .await
    .expect("the cycle failed");
    workers.stop().await;
    let status = |chat: String| {
        let context = harness.context.clone();
        async move {
            memory_get(
                &context,
                dto::ConversationRequest {
                    conversation_id: chat,
                },
            )
            .await
            .expect("memory")
            .status
        }
    };
    let in_source = status(source.clone()).await;
    let in_copy = status(copy.clone()).await;
    assert_eq!(
        in_source.failure,
        Some(dto::MemoryFailureCode::ModelMissing)
    );
    assert_eq!(in_copy, in_source);
    assert_eq!(in_copy.latest_job_id, Some(accepted.job_id));
    assert_eq!(
        CompanionStateRepository::get(database, owner)
            .expect("private state")
            .expect("exists")
            .state,
        changed
    );
    memory_error_dismiss(
        &harness.context,
        dto::MemoryErrorDismissRequest {
            conversation_id: copy,
            client_operation_id: "pool-dismiss".into(),
        },
    )
    .await
    .expect("dismiss from the copy");
    assert_eq!(status(source).await.failure, None);
}

#[tokio::test]
async fn a_fork_lists_and_reverts_only_its_own_cycles_and_never_touches_the_parent() {
    use lettuce_conversations::ConversationReader;
    use lettuce_memory::MemoryRepository;
    let harness = harness(Reply::Text("reply"));
    let (chat, reply) = replied_chat(&harness, "fork").await;
    make_dynamic(&harness, true);
    let parent_run =
        recorded_cycle(&harness, &chat, "Parent summary", "Parent fact", 1_000, 0).await;
    let database = harness.context.backend().database();
    let conversation_id: lettuce_types::ConversationId = chat.parse().expect("id");
    let root = ConversationReader::get(database, conversation_id)
        .expect("conversation")
        .conversation
        .active_branch_id;
    let revision = ConversationReader::get(database, conversation_id)
        .expect("conversation")
        .conversation
        .revision
        .get();
    conversation_branch_fork(
        &harness.context,
        dto::ConversationBranchForkRequest {
            conversation_id: chat.clone(),
            message_id: reply,
            expected_revision: revision,
            client_operation_id: "fork-here".into(),
        },
    )
    .await
    .expect("fork");
    let parent_before = MemoryRepository::get_for_branch(database, conversation_id, root)
        .expect("parent")
        .expect("space");
    let child = memory_get(
        &harness.context,
        dto::ConversationRequest {
            conversation_id: chat.clone(),
        },
    )
    .await
    .expect("child memory");
    let log = memory_cycles(
        &harness.context,
        dto::MemoryCyclesRequest {
            conversation_id: chat.clone(),
            cursor: None,
            limit: None,
        },
    )
    .await
    .expect("child log");
    assert!(log.items.is_empty());
    let refused = memory_cycle_revert(
        &harness.context,
        dto::MemoryCycleRevertRequest {
            conversation_id: chat,
            run_id: parent_run.run_id,
            expected_revision: child.revision,
            client_operation_id: "fork-revert".into(),
        },
    )
    .await
    .expect_err("the parent's cycle is not the child's");
    assert_eq!(refused.code, ApiErrorCode::NotFound);
    assert_eq!(
        MemoryRepository::get_for_branch(database, conversation_id, root)
            .expect("parent")
            .expect("space"),
        parent_before
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_fork_after_a_finished_turn_does_not_move_the_parents_due_window_to_the_child() {
    use lettuce_conversations::ConversationReader;
    use lettuce_jobs::{JobCatalog, JobKind, JobListFilter};
    use lettuce_settings::GlobalSettingsStore;
    let harness = harness(Reply::Text("reply"));
    let (chat, reply) = replied_chat(&harness, "driver-fork").await;
    make_dynamic(&harness, true);
    let database = harness.context.backend().database();
    let conversation_id: lettuce_types::ConversationId = chat.parse().expect("id");
    let root = ConversationReader::get(database, conversation_id)
        .expect("conversation")
        .conversation
        .active_branch_id;
    let revision = ConversationReader::get(database, conversation_id)
        .expect("conversation")
        .conversation
        .revision
        .get();
    let fork = conversation_branch_fork(
        &harness.context,
        dto::ConversationBranchForkRequest {
            conversation_id: chat.clone(),
            message_id: reply,
            expected_revision: revision,
            client_operation_id: "driver-fork-here".into(),
        },
    )
    .await
    .expect("fork right after the turn");
    assert_ne!(fork.branch_id, root.to_string());
    let stored = GlobalSettingsStore::load(database).expect("settings");
    GlobalSettingsStore::save(database, stored.settings, None, stored.revision)
        .expect("no default model");
    let workers = startup(&harness.context).await.expect("startup");
    workers.started().await;
    let mut admitted = None;
    for _ in 0..60 {
        let page = database
            .list_jobs(&JobListFilter {
                kinds: vec![JobKind::MemoryExtraction],
                states: vec![],
                subject: None,
                page: lettuce_types::PageRequest {
                    cursor: None,
                    limit: lettuce_types::PageLimit::new(10),
                },
            })
            .expect("jobs");
        if let Some(job) = page.items.first() {
            admitted = Some(job.id);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    workers.stop().await;
    let job = admitted.expect("the finished turn admitted its cycle");
    let detail = database
        .job_detail(job)
        .expect("detail")
        .expect("frozen admission");
    let batch = crate::companion::companion_memory_job::decode_memory_admission(detail.detail)
        .expect("admission");
    assert_eq!(batch.branch_id, root);
}

async fn status_of(harness: &super::tests::Harness, chat: &str) -> dto::MemoryView {
    memory_get(
        &harness.context,
        dto::ConversationRequest {
            conversation_id: chat.into(),
        },
    )
    .await
    .expect("memory")
}

async fn set_user_summary(harness: &super::tests::Harness, chat: &str, key: &str) {
    let revision = status_of(harness, chat).await.revision;
    memory_summary_update(
        &harness.context,
        dto::MemorySummaryUpdateRequest {
            conversation_id: chat.into(),
            summary: dto::MemorySummaryEdit::Set {
                text: "Mine".into(),
            },
            expected_revision: revision,
            client_operation_id: key.into(),
        },
    )
    .await
    .expect("user summary");
}

#[tokio::test]
async fn a_user_summary_set_or_clear_keeps_the_model_cycle_cursor() {
    let harness = harness(Reply::Text("reply"));
    let (chat, _) = replied_chat(&harness, "cursor-model").await;
    make_dynamic(&harness, true);
    recorded_cycle(&harness, &chat, "Model summary", "A fact", 1_000, 0).await;
    let before = status_of(&harness, &chat).await.status;
    assert_eq!(before.messages_since_last_cycle, 0);
    set_user_summary(&harness, &chat, "cursor-set").await;
    let set = status_of(&harness, &chat).await.status;
    assert_eq!(set.messages_since_last_cycle, 0);
    assert_eq!(
        set.messages_until_next_cycle,
        before.messages_until_next_cycle
    );
    let revision = status_of(&harness, &chat).await.revision;
    memory_summary_update(
        &harness.context,
        dto::MemorySummaryUpdateRequest {
            conversation_id: chat.clone(),
            summary: dto::MemorySummaryEdit::Clear,
            expected_revision: revision,
            client_operation_id: "cursor-clear".into(),
        },
    )
    .await
    .expect("clear");
    let cleared = status_of(&harness, &chat).await.status;
    assert_eq!(cleared.messages_since_last_cycle, 0);
}

#[tokio::test]
async fn a_user_summary_over_an_imported_one_keeps_its_coverage_even_when_cleared() {
    use lettuce_conversations::ConversationReader;
    use lettuce_memory::{
        MemoryOrigin, MemoryRepository, MemorySummary, MemorySummaryChange, MemorySummaryRepository,
    };
    let harness = harness(Reply::Text("reply"));
    let (chat, _) = replied_chat(&harness, "cursor-import").await;
    make_dynamic(&harness, true);
    let database = harness.context.backend().database();
    let conversation_id: lettuce_types::ConversationId = chat.parse().expect("id");
    let branch_id = ConversationReader::get(database, conversation_id)
        .expect("conversation")
        .conversation
        .active_branch_id;
    let space = MemoryRepository::get_for_branch(database, conversation_id, branch_id)
        .expect("memory")
        .expect("space");
    let view = super::turns_tests::open(&harness, &chat).await;
    let sources = view
        .messages
        .items
        .iter()
        .rev()
        .map(|message| message.id.parse().expect("message"))
        .collect::<Vec<_>>();
    database
        .compare_and_apply_summary(MemorySummaryChange {
            expected_revision: space.revision,
            summary: MemorySummary {
                origin: MemoryOrigin::Import,
                space_id: space.id,
                branch_id,
                text: "Imported".into(),
                token_count: Some(3),
                window_start: 0,
                window_end: u64::try_from(sources.len()).expect("count"),
                source_message_ids: sources,
                updated_at: harness.context.now(),
            },
        })
        .expect("imported summary");
    let before = status_of(&harness, &chat).await.status;
    assert_eq!(before.messages_since_last_cycle, 0);
    set_user_summary(&harness, &chat, "cursor-import-set").await;
    assert_eq!(
        status_of(&harness, &chat)
            .await
            .status
            .messages_since_last_cycle,
        0
    );
    let revision = status_of(&harness, &chat).await.revision;
    memory_summary_update(
        &harness.context,
        dto::MemorySummaryUpdateRequest {
            conversation_id: chat.clone(),
            summary: dto::MemorySummaryEdit::Clear,
            expected_revision: revision,
            client_operation_id: "cursor-import-clear".into(),
        },
    )
    .await
    .expect("clear");
    assert_eq!(
        status_of(&harness, &chat)
            .await
            .status
            .messages_since_last_cycle,
        0
    );
}

struct OpenRound {
    run_id: lettuce_types::DynamicMemoryRunId,
    attempt_id: lettuce_types::DynamicMemoryAttemptId,
    claim: lettuce_jobs::Claim,
    handle: lettuce_jobs::handle::JobHandle,
}

async fn open_round(
    harness: &super::tests::Harness,
    chat: &str,
    calls: Vec<(&str, serde_json::Value)>,
) -> OpenRound {
    use lettuce_conversations::{ConversationOverviewReader, ConversationReader};
    use lettuce_jobs::{
        JobKind, JobSpec, JobStore, JobSubject, OutcomeRef, ResourceAvailability, ResourceClass,
        SubjectKind, WorkerId,
    };
    use lettuce_memory::{
        DynamicMemoryAttemptStatus, DynamicMemoryRoundFinishReason, DynamicMemoryRoundKind,
        DynamicMemoryRunRepository, DynamicMemorySourceMessage, MemoryRepository,
        NewDynamicMemoryInferenceRound, NewDynamicMemoryRunAttempt, NewDynamicMemoryToolCall,
    };
    use lettuce_types::{
        DynamicMemoryAttemptId, DynamicMemoryRunId, TimestampMillis, ToolExecutionId,
    };
    let database = harness.context.backend().database();
    let conversation_id: lettuce_types::ConversationId = chat.parse().expect("conversation");
    let branch_id = ConversationReader::get(database, conversation_id)
        .expect("conversation")
        .conversation
        .active_branch_id;
    let view = super::turns_tests::open(harness, chat).await;
    let mut sources = Vec::new();
    for message in view.messages.items.iter().rev() {
        let item = ConversationOverviewReader::timeline_anchor(
            database,
            conversation_id,
            branch_id,
            message.id.parse().expect("message"),
        )
        .expect("anchor")
        .item;
        sources.push(DynamicMemorySourceMessage {
            message_id: item.message.id,
            role: item.message.role,
            render_source: item.message.active_render_source,
            effective_time: item.message.effective_time,
        });
    }
    let memory = MemoryRepository::get_for_branch(database, conversation_id, branch_id)
        .expect("memory")
        .expect("space");
    let job = JobStore::create_or_get(
        database,
        JobSpec::new(
            JobKind::MemoryExtraction,
            JobSubject::new(SubjectKind::Conversation, format!("{chat}-open")).expect("subject"),
            OutcomeRef::Conversation(conversation_id),
        )
        .with_resources(vec![
            ResourceClass::Network,
            ResourceClass::ModelLoad,
            ResourceClass::DiskRead,
            ResourceClass::DiskWrite,
            ResourceClass::Cpu,
        ])
        .with_policies(
            lettuce_jobs::RecoveryPolicy::Restart,
            lettuce_jobs::CancellationPolicy::Cooperative,
        ),
    )
    .expect("job")
    .job;
    let claim = JobStore::claim(
        database,
        job.id,
        WorkerId::new(),
        harness.context.now(),
        std::time::Duration::from_secs(600),
        &ResourceAvailability::all(),
    )
    .expect("claim")
    .expect("claimed");
    let run_id = DynamicMemoryRunId::new();
    let attempt_id = DynamicMemoryAttemptId::new();
    let at = harness.context.now();
    let admitted = database
        .admit_dynamic_memory_run_attempt(NewDynamicMemoryRunAttempt {
            run_id,
            attempt_id,
            conversation_id,
            branch_id,
            space_id: memory.id,
            cycle_start_change: None,
            starting_memory: memory,
            source_messages: sources.clone(),
            profile: crate::companion::companion_memory_run::tests::profile(),
            time_awareness_enabled: false,
            supersession_enabled: false,
            structured_fallback_format: lettuce_memory::DynamicMemoryStructuredFallbackFormat::Xml,
            summary_window: lettuce_memory::DynamicMemorySummaryWindow {
                message_interval: 2,
                start: 0,
                end: u64::try_from(sources.len()).expect("count"),
            },
            tool_request: lettuce_memory::dynamic_memory_tool_request_for_run(
                lettuce_memory::DynamicMemoryToolOptions {
                    group: false,
                    supersession_enabled: false,
                    require_source_message_id: false,
                },
                &|key| key.to_owned(),
            ),
            job_id: job.id,
            job_attempt: None,
            now: at,
        })
        .expect("run");
    database
        .transition_dynamic_memory_attempt(
            attempt_id,
            admitted.attempt.revision,
            DynamicMemoryAttemptStatus::Processing,
            None,
            at,
        )
        .expect("processing");
    database
        .admit_dynamic_memory_inference_round(
            run_id,
            attempt_id,
            0,
            0,
            NewDynamicMemoryInferenceRound {
                ordinal: 0,
                request_context: lettuce_conversations::ProviderNeutralContext {
                    messages: vec![],
                    attributions: Default::default(),
                    budget: Default::default(),
                },
                parts: vec![],
                provider_replay: None,
                usage: None,
                finish_reason: DynamicMemoryRoundFinishReason::Stop,
                kind: DynamicMemoryRoundKind::Manager,
                provider_request_id: None,
                calls: calls
                    .into_iter()
                    .enumerate()
                    .map(|(index, (name, arguments))| NewDynamicMemoryToolCall {
                        id: ToolExecutionId::new(),
                        definition_version: 1,
                        call: lettuce_conversations::ProposedToolCall {
                            provider_call_id: Some(format!("{name}-{index}")),
                            name: name.into(),
                            arguments,
                            raw_arguments: None,
                            provider_replay: None,
                        },
                    })
                    .collect(),
                admitted_at: TimestampMillis::new(at.get() + 1),
            },
        )
        .expect("round");
    let _ = DynamicMemoryRunRepository::load_dynamic_memory_run(database, run_id).expect("run");
    OpenRound {
        run_id,
        attempt_id,
        handle: lettuce_jobs::handle::JobHandle::new(job.id),
        claim,
    }
}

fn settle(
    harness: &super::tests::Harness,
    round: &OpenRound,
) -> lettuce_memory::DynamicMemoryBackgroundRoundSettlement {
    use lettuce_memory::DynamicMemoryRunRepository;
    let rounds = harness.context.backend().database().list_dynamic_memory_inference_rounds(round.run_id, round.attempt_id).expect("rounds");
    let seeds = rounds[0].calls.iter().filter(|call| call.call.name == "create_memory")
        .map(|call| crate::MemoryCreateSeed {
            execution_id: call.id, id: lettuce_types::MemoryId::new(), token_count: None,
            created_at: harness.context.now(),
        }).collect::<Vec<_>>();
    struct CreateEmbedding;
    impl crate::MemoryEmbeddingEngine for CreateEmbedding {
        fn source_revision(&self) -> &str { "control-create-test" }
        fn dimensions(&self) -> lettuce_embeddings::EmbeddingDimensions {
            lettuce_embeddings::EmbeddingDimensions::D64
        }
        fn count_tokens(&self, _: &str) -> Result<u32, crate::EmbeddingGenerationError> { Ok(1) }
        fn embed_memory(
            &self,
            request: &lettuce_embeddings::EmbeddingRequest,
            _: &lettuce_jobs::handle::CancellationToken,
        ) -> Result<lettuce_embeddings::EmbeddingVector, crate::EmbeddingGenerationError> {
            let mut values = vec![0.0; request.dimensions.get()];
            values[0] = 1.0;
            Ok(lettuce_embeddings::EmbeddingVector { source_revision: "control-create-test".into(), values })
        }
    }
    let engine: std::sync::Arc<dyn crate::MemoryEmbeddingEngine> = if seeds.is_empty() {
        harness.context.embedding()
    } else {
        std::sync::Arc::new(CreateEmbedding)
    };
    crate::CompanionMemoryRoundExecutor::new(engine.as_ref(), harness.context.backend().database())
        .execute_round(
            round.run_id,
            round.attempt_id,
            0,
            &lettuce_memory::MemoryPolicy {
                max_entries: 10,
                hot_token_budget: 1_000,
                cold_threshold: lettuce_memory::Score::from_basis_points(2_000).expect("score"),
                delete_confidence_default: lettuce_memory::Score::from_basis_points(5_000)
                    .expect("score"),
                max_hard_delete_ratio_per_cycle: lettuce_memory::Score::from_basis_points(10_000)
                    .expect("score"),
                decay_rate: lettuce_memory::Score::from_basis_points(800).expect("score"),
            },
            &seeds,
            lettuce_memory::Score::from_basis_points(9_000).expect("score"),
            &round.claim,
            &round.handle,
            harness.context.now(),
        )
        .expect("settle")
        .settlement
}

async fn item(harness: &super::tests::Harness, chat: &str, text: &str) -> dto::MemoryItemView {
    status_of(harness, chat)
        .await
        .items
        .into_iter()
        .find(|item| item.text == text)
        .unwrap_or_else(|| panic!("{text} is missing"))
}

#[tokio::test]
async fn a_user_edit_during_a_run_survives_the_models_delete_and_unpin() {
    let harness = harness(Reply::Text("reply"));
    let (chat, _) = replied_chat(&harness, "stale").await;
    for (index, text) in ["Edited by the user", "Pinned by the model run", "Untouched"]
        .into_iter()
        .enumerate()
    {
        let revision = status_of(&harness, &chat).await.revision;
        memory_add(
            &harness.context,
            dto::MemoryAddRequest {
                conversation_id: chat.clone(),
                text: text.into(),
                category: None,
                observed_at: None,
                expected_revision: revision,
                client_operation_id: format!("stale-add-{index}"),
            },
        )
        .await
        .expect("add");
    }
    let edited = item(&harness, &chat, "Edited by the user").await;
    let unpinned = item(&harness, &chat, "Pinned by the model run").await;
    let untouched = item(&harness, &chat, "Untouched").await;
    let revision = status_of(&harness, &chat).await.revision;
    memory_pin(
        &harness.context,
        dto::MemoryPinRequest {
            conversation_id: chat.clone(),
            memory_id: unpinned.id.clone(),
            pinned: true,
            expected_revision: revision,
            client_operation_id: "stale-pin-before".into(),
        },
    )
    .await
    .expect("pin before the run");
    let round = open_round(
        &harness,
        &chat,
        vec![
            (
                "delete_memory",
                serde_json::json!({"text": edited.short_id, "confidence": 1.0}),
            ),
            ("unpin_memory", serde_json::json!({"id": unpinned.short_id})),
            (
                "delete_memory",
                serde_json::json!({"text": untouched.short_id, "confidence": 1.0}),
            ),
        ],
    )
    .await;
    let revision = status_of(&harness, &chat).await.revision;
    memory_update(
        &harness.context,
        dto::MemoryUpdateRequest {
            conversation_id: chat.clone(),
            memory_id: edited.id.clone(),
            text: Some("Edited by the user again".into()),
            category: dto::MemoryCategoryChange::Keep,
            observed_at: dto::MemoryObservedAtChange::Keep,
            expected_revision: revision,
            client_operation_id: "stale-edit".into(),
        },
    )
    .await
    .expect("edit during the run");
    let revision = status_of(&harness, &chat).await.revision;
    memory_pin(
        &harness.context,
        dto::MemoryPinRequest {
            conversation_id: chat.clone(),
            memory_id: unpinned.id.clone(),
            pinned: true,
            expected_revision: revision,
            client_operation_id: "stale-repin".into(),
        },
    )
    .await
    .expect("pin again during the run");
    let settlement = settle(&harness, &round);
    assert!(matches!(
        settlement.results[0].outcome,
        lettuce_memory::MemoryToolOutcome::Skipped {
            reason: lettuce_memory::MemoryToolSkipReason::UserEdited
        }
    ));
    assert!(matches!(
        settlement.results[1].outcome,
        lettuce_memory::MemoryToolOutcome::Skipped {
            reason: lettuce_memory::MemoryToolSkipReason::UserEdited
        }
    ));
    assert!(matches!(
        settlement.results[2].outcome,
        lettuce_memory::MemoryToolOutcome::Deleted { .. }
    ));
    let after = status_of(&harness, &chat).await;
    assert!(
        after
            .items
            .iter()
            .any(|item| item.text == "Edited by the user again")
    );
    assert!(
        after
            .items
            .iter()
            .any(|item| item.id == unpinned.id && item.pinned)
    );
    assert!(after.items.iter().all(|item| item.id != untouched.id));
}

#[tokio::test]
async fn the_store_admits_one_active_memory_job_per_conversation() {
    use crate::companion::companion_memory_job::{
        CompanionMemoryWindowSelection, CompanionPostTurnMemoryBatch, PostTurnMemorySource,
        job_spec,
    };
    use lettuce_conversations::{ConversationReader, MessageRole};
    use lettuce_jobs::{IdempotencyKey, JobKind, JobQuery, JobStore, StoreError};
    let harness = harness(Reply::Text("reply"));
    let (chat, _) = replied_chat(&harness, "one-active").await;
    make_dynamic(&harness, true);
    let database = harness.context.backend().database();
    let conversation_id: lettuce_types::ConversationId = chat.parse().expect("id");
    let branch_id = ConversationReader::get(database, conversation_id)
        .expect("conversation")
        .conversation
        .active_branch_id;
    let view = super::turns_tests::open(&harness, &chat).await;
    let messages = view
        .messages
        .items
        .iter()
        .rev()
        .map(|message| {
            (
                message.id.parse().expect("message"),
                if message.role == dto::MessageRole::User {
                    MessageRole::User
                } else {
                    MessageRole::Assistant
                },
            )
        })
        .collect::<Vec<_>>();
    let admit = |key: &str, selection| {
        let key = IdempotencyKey::new(key.to_owned()).expect("key");
        crate::companion::companion_memory_job::MemoryAdmissionStore::admit_memory_batch(
            database,
            job_spec(conversation_id, key.clone()).expect("spec"),
            CompanionPostTurnMemoryBatch {
                conversation_id,
                branch_id,
                idempotency_key: key,
                summary_message_interval: 2,
                window_selection: selection,
                unsummarized_message_count: 2,
                source: PostTurnMemorySource::Messages(messages.clone()),
                selected_model_profile_id: None,
                update_dynamic_memory_model_on_success: false,
            },
        )
    };
    let first =
        admit("one-active-a", CompanionMemoryWindowSelection::Automatic).expect("first admission");
    assert!(first.created);
    assert_eq!(
        admit("one-active-a", CompanionMemoryWindowSelection::Automatic)
            .expect("the same window replays")
            .job
            .id,
        first.job.id
    );
    assert_eq!(
        admit("one-active-b", CompanionMemoryWindowSelection::Recent).expect_err("a second"),
        StoreError::AlreadyActive
    );
    let jobs = database
        .list(JobQuery {
            state: None,
            kind: Some(JobKind::MemoryExtraction),
            subject: None,
            page: lettuce_types::PageRequest {
                cursor: None,
                limit: lettuce_types::PageLimit::new(50),
            },
        })
        .expect("jobs");
    assert_eq!(jobs.items.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_concurrent_trigger_and_post_turn_admission_create_one_job() {
    use lettuce_jobs::{JobKind, JobQuery, JobStore, ResourceAvailability, WorkerId};
    for round in 0..10 {
        let harness = harness(Reply::Text("reply"));
        let (chat, _) = replied_chat(&harness, &format!("race-{round}")).await;
        make_dynamic(&harness, true);
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let post_turn = {
            let context = harness.context.clone();
            let chat = chat.clone();
            let barrier = barrier.clone();
            tokio::task::spawn_blocking(move || {
                let embedding = context.embedding();
                let host = context
                    .backend()
                    .companion_memory_host(embedding.as_ref(), context.inference());
                barrier.wait();
                host.after_turn(
                    chat.parse().expect("id"),
                    lettuce_conversations::GenerationOperation::Send,
                    WorkerId::new(),
                    context.now(),
                    std::time::Duration::from_secs(30),
                    &ResourceAvailability::all(),
                )
            })
        };
        let trigger = {
            let context = harness.context.clone();
            let chat = chat.clone();
            let barrier = barrier.clone();
            tokio::task::spawn_blocking(move || {
                barrier.wait();
                tokio::runtime::Handle::current().block_on(memory_trigger(
                    &context,
                    trigger_request(&chat, "race-trigger"),
                ))
            })
        };
        let _ = post_turn.await.expect("post turn thread");
        let _ = trigger.await.expect("trigger thread");
        let jobs = harness
            .context
            .backend()
            .database()
            .list(JobQuery {
                state: None,
                kind: Some(JobKind::MemoryExtraction),
                subject: None,
                page: lettuce_types::PageRequest {
                    cursor: None,
                    limit: lettuce_types::PageLimit::new(50),
                },
            })
            .expect("jobs");
        assert_eq!(jobs.items.len(), 1, "round {round}");
    }
}

fn memory_events(harness: &super::tests::Harness, chat: &str) -> usize {
    super::tests::api_events(harness)
        .into_iter()
        .filter(|event| matches!(event, dto::ApiEvent::MemoryChanged { conversation_id } if conversation_id == chat))
        .count()
}

#[tokio::test]
async fn a_manual_edit_emits_one_memory_changed_after_it_commits_and_a_rollback_none() {
    use super::conversation_feed::ConversationFeed;
    let harness = harness(Reply::Text("reply"));
    let (chat, _) = replied_chat(&harness, "events").await;
    let mut feed = ConversationFeed::start(&harness.context)
        .await
        .expect("feed");
    feed.publish(&harness.context).await.expect("idle publish");
    let baseline = memory_events(&harness, &chat);
    let revision = status_of(&harness, &chat).await.revision;
    memory_add(
        &harness.context,
        dto::MemoryAddRequest {
            conversation_id: chat.clone(),
            text: "Event fact".into(),
            category: None,
            observed_at: None,
            expected_revision: revision,
            client_operation_id: "events-add".into(),
        },
    )
    .await
    .expect("add");
    feed.publish(&harness.context).await.expect("publish");
    assert_eq!(memory_events(&harness, &chat), baseline + 1);
    feed.publish(&harness.context).await.expect("quiet publish");
    assert_eq!(memory_events(&harness, &chat), baseline + 1);

    let database = harness.context.backend().database();
    let conversation_id: lettuce_types::ConversationId = chat.parse().expect("id");
    let branch_id = lettuce_conversations::ConversationReader::get(database, conversation_id)
        .expect("conversation")
        .conversation
        .active_branch_id;
    let space =
        lettuce_memory::MemoryRepository::get_for_branch(database, conversation_id, branch_id)
            .expect("memory")
            .expect("space");
    let conversation_revision =
        lettuce_conversations::ConversationReader::get(database, conversation_id)
            .expect("conversation")
            .conversation
            .revision;
    let failed: Result<(), lettuce_database::ApiOperationError> = database.commit_api_operation(
        "events-rollback",
        "events-rollback",
        "digest",
        harness.context.now(),
        |scope| {
            scope
                .apply_memory_manual_edit(
                    &lettuce_memory::MemoryManualEdit {
                        id: lettuce_types::OperationId::new(),
                        conversation_id,
                        branch_id,
                        conversation_revision,
                        expected_revision: space.revision,
                        space_id: space.id,
                        context_revisions: vec![],
                        mutation: lettuce_memory::MemoryManualMutation::Summary { summary: None },
                        at: harness.context.now(),
                    },
                    None,
                )
                .expect("the edit applies before the operation fails");
            Err(lettuce_database::ApiOperationError::Storage)
        },
    );
    assert!(failed.is_err());
    feed.publish(&harness.context)
        .await
        .expect("rollback publish");
    assert_eq!(memory_events(&harness, &chat), baseline + 1);
}

#[tokio::test]
async fn a_cycle_in_one_pooled_chat_emits_memory_changed_for_the_other() {
    use super::conversation_feed::ConversationFeed;
    let harness = harness(Reply::Text("reply"));
    let database = harness.context.backend().database();
    let character = super::tests::create_character(
        database,
        "Pool events",
        lettuce_characters::CharacterDefaults {
            interaction_mode: lettuce_characters::InteractionMode::Companion,
            companion_soul: Some(lettuce_companions::CompanionSoulConfig::default()),
            memory_policy: lettuce_characters::MemoryPolicy::Dynamic,
            ..lettuce_characters::CharacterDefaults::default()
        },
    );
    let a = conversation_launch_direct(
        &harness.context,
        dto::LaunchDirectRequest {
            character_id: character.to_string(),
            title: None,
            scene_id: None,
            starter_id: None,
            client_operation_id: "pool-events-a".into(),
        },
    )
    .await
    .expect("launch")
    .conversation_id;
    let message = conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: a.clone(),
            text: "Hello".into(),
            expected_revision: 1,
            client_operation_id: "pool-events-message".into(),
        },
    )
    .await
    .expect("message");
    let _ = message;
    let b = conversation_duplicate(
        &harness.context,
        dto::ConversationDuplicateRequest {
            conversation_id: a.clone(),
            title: None,
            with_messages: true,
            client_operation_id: "pool-events-b".into(),
        },
    )
    .await
    .expect("duplicate")
    .conversation_id;
    let mut feed = ConversationFeed::start(&harness.context)
        .await
        .expect("feed");
    feed.publish(&harness.context).await.expect("idle publish");
    let (before_a, before_b) = (memory_events(&harness, &a), memory_events(&harness, &b));
    recorded_cycle(&harness, &a, "Pool summary", "Pool fact", 1_000, 0).await;
    feed.publish(&harness.context).await.expect("publish");
    assert_eq!(memory_events(&harness, &a), before_a + 1);
    assert_eq!(memory_events(&harness, &b), before_b + 1);
}

#[tokio::test]
async fn a_revert_emits_memory_changed() {
    use super::conversation_feed::ConversationFeed;
    let harness = harness(Reply::Text("reply"));
    let (chat, _) = replied_chat(&harness, "events-revert").await;
    make_dynamic(&harness, true);
    let cycle = recorded_cycle(&harness, &chat, "Summary", "Fact", 1_000, 0).await;
    let mut feed = ConversationFeed::start(&harness.context)
        .await
        .expect("feed");
    feed.publish(&harness.context).await.expect("idle publish");
    let baseline = memory_events(&harness, &chat);
    let revision = status_of(&harness, &chat).await.revision;
    memory_cycle_revert(
        &harness.context,
        dto::MemoryCycleRevertRequest {
            conversation_id: chat.clone(),
            run_id: cycle.run_id,
            expected_revision: revision,
            client_operation_id: "events-revert".into(),
        },
    )
    .await
    .expect("revert");
    feed.publish(&harness.context).await.expect("publish");
    assert_eq!(memory_events(&harness, &chat), baseline + 1);
}

#[tokio::test]
async fn concurrent_memory_control_keys_replay_one_job_and_conflicting_digests_do_not_admit() {
    let harness = harness(Reply::Text("reply"));
    let (chat, _) = replied_chat(&harness, "atomic-control").await;
    make_dynamic(&harness, true);
    let request = trigger_request(&chat, "concurrent-control");
    let (first, second) = tokio::join!(
        memory_trigger(&harness.context, request.clone()),
        memory_trigger(&harness.context, request),
    );
    assert_eq!(first.expect("first admission"), second.expect("same key replay"));
    let empty = launch(&harness, "conflicting-control").await;
    let error = memory_trigger(&harness.context, trigger_request(&empty, "concurrent-control")).await.expect_err("different digest");
    assert_eq!(error.code, ApiErrorCode::Conflict);
}

#[tokio::test]
async fn a_memory_retry_receipt_replays_before_the_selected_model_is_resolved_again() {
    let harness = harness(Reply::Text("reply"));
    let (chat, _) = replied_chat(&harness, "replay-model").await;
    let request = dto::MemoryRetryRequest {
        conversation_id: chat,
        model_profile_id: Some(lettuce_types::ModelProfileId::new().to_string()),
        client_operation_id: "completed-retry".into(),
    };
    let bytes = serde_json::to_vec(&request).expect("request");
    let operation = super::messages::operation(request.client_operation_id.clone(), &[b"memory_retry", &bytes]).expect("operation");
    let accepted = dto::JobAccepted { job_id: lettuce_types::JobId::new().to_string() };
    harness.context.backend().database().commit_api_operation::<_, super::memory::EditFailure>(
        "memory_retry", &request.client_operation_id, operation.request_digest.as_str(), harness.context.now(),
        |_| Ok(accepted.clone()),
    ).unwrap_or_else(|error| panic!("receipt write failed: {}", error.0.message));
    assert_eq!(memory_retry(&harness.context, request).await.expect("receipt replay"), accepted);
}

#[tokio::test]
async fn pooled_delete_after_preserves_retained_user_edits_when_undoing_model_tools() {
    use lettuce_jobs::{JobMutation, JobOutcome, JobStore, OutcomeRef};
    use lettuce_memory::{DynamicMemoryAttemptStatus, DynamicMemoryRunRepository};
    for tool in ["pin_memory", "unpin_memory", "create_memory"] {
        let harness = super::tests::harness_in(
            Reply::Text("reply"), std::sync::Arc::new(lettuce_jobs::SystemClock), None, None,
            std::sync::Arc::new(super::inspect_tests::AllModels),
        );
        let character = super::tests::create_character(
            harness.context.backend().database(), "Retained tool edits",
            lettuce_characters::CharacterDefaults {
                interaction_mode: lettuce_characters::InteractionMode::Companion,
                companion_soul: Some(lettuce_companions::CompanionSoulConfig::default()),
                memory_policy: lettuce_characters::MemoryPolicy::Dynamic,
                ..lettuce_characters::CharacterDefaults::default()
            },
        );
        let a = conversation_launch_direct(&harness.context, dto::LaunchDirectRequest {
            character_id: character.to_string(), title: None, scene_id: None, starter_id: None,
            client_operation_id: "tool-pool-a".into(),
        }).await.expect("companion").conversation_id;
        let first = conversation_add_user_message(&harness.context, dto::ConversationAddUserMessageRequest {
            conversation_id: a.clone(), text: "Keep".into(), expected_revision: 1,
            client_operation_id: "tool-pool-first".into(),
        }).await.expect("kept anchor");
        let b = conversation_duplicate(&harness.context, dto::ConversationDuplicateRequest {
            conversation_id: a.clone(), title: None, with_messages: true,
            client_operation_id: "tool-pool-b".into(),
        }).await.expect("pooled duplicate").conversation_id;
        let target = if tool == "create_memory" {
            None
        } else {
            let added = memory_add(&harness.context, dto::MemoryAddRequest {
                conversation_id: a.clone(), text: "Shared tool target".into(), category: None,
                observed_at: None, expected_revision: status_of(&harness, &a).await.revision,
                client_operation_id: "tool-pool-add".into(),
            }).await.expect("existing target").memory_id.expect("id");
            if tool == "unpin_memory" {
                memory_pin(&harness.context, dto::MemoryPinRequest {
                    conversation_id: a.clone(), memory_id: added.clone(), pinned: true,
                    expected_revision: status_of(&harness, &a).await.revision,
                    client_operation_id: "tool-pool-pin-before".into(),
                }).await.expect("initial pin");
            }
            Some(added)
        };
        let later = conversation_add_user_message(&harness.context, dto::ConversationAddUserMessageRequest {
            conversation_id: a.clone(), text: "Remove this suffix".into(), expected_revision: first.revision,
            client_operation_id: "tool-pool-later".into(),
        }).await.expect("removed anchor");
        let arguments = match &target {
            Some(id) => serde_json::json!({"id": id}),
            None => serde_json::json!({"text": "New model fact", "category": "preference"}),
        };
        let round = open_round(&harness, &a, vec![(tool, arguments)]).await;
        harness.context.backend().database().append_and_transition(JobMutation::Start {
            claim: round.claim.claim.clone(), at: harness.context.now(),
        }).expect("started model job");
        let settlement = settle(&harness, &round);
        let target = target.unwrap_or_else(|| match &settlement.results[0].outcome {
            lettuce_memory::MemoryToolOutcome::Created { id, .. } => id.to_string(),
            other => panic!("expected create, got {other:?}"),
        });
        let database = harness.context.backend().database();
        let attempt = database.load_dynamic_memory_attempt(round.attempt_id).expect("attempt");
        database.transition_dynamic_memory_attempt(round.attempt_id, attempt.revision,
            DynamicMemoryAttemptStatus::Succeeded, None, harness.context.now()).expect("finished cycle");
        database.append_and_transition(JobMutation::Succeed {
            claim: round.claim.claim.clone(), outcome: JobOutcome::Success {
                result_ref: OutcomeRef::Conversation(a.parse().expect("conversation")),
            }, at: harness.context.now(),
        }).expect("finished job");
        let revision = status_of(&harness, &b).await.revision;
        match tool {
            "pin_memory" => { memory_pin(&harness.context, dto::MemoryPinRequest {
                conversation_id: b.clone(), memory_id: target.clone(), pinned: true,
                expected_revision: revision, client_operation_id: "tool-pool-retained-pin".into(),
            }).await.expect("retained equal pin"); }
            "unpin_memory" => { memory_set_temperature(&harness.context, dto::MemoryTemperatureRequest {
                conversation_id: b.clone(), memory_id: target.clone(), temperature: dto::MemoryTemperature::Cold,
                expected_revision: revision, client_operation_id: "tool-pool-retained-cold".into(),
            }).await.expect("retained cold setter"); }
            _ => { memory_update(&harness.context, dto::MemoryUpdateRequest {
                conversation_id: b.clone(), memory_id: target.clone(), text: Some("User-confirmed fact".into()),
                category: dto::MemoryCategoryChange::Keep, observed_at: dto::MemoryObservedAtChange::Keep,
                expected_revision: revision, client_operation_id: "tool-pool-retained-text".into(),
            }).await.expect("retained edit of model-created item"); }
        }
        messages_delete_after(&harness.context, dto::MessageDeleteRequest {
            conversation_id: a.clone(), message_id: first.message.id, expected_revision: later.revision,
            client_operation_id: "tool-pool-delete-after".into(),
        }).await.expect("delete after preserves retained pool edits");
        let memory = status_of(&harness, &b).await;
        let item = memory.items.iter().find(|item| item.id == target).expect("retained item");
        match tool {
            "pin_memory" => assert!(item.pinned, "the retained user pin owns this field"),
            "unpin_memory" => {
                assert!(!item.pinned);
                assert_eq!(item.temperature, dto::MemoryTemperature::Cold);
            }
            _ => assert_eq!(item.text, "User-confirmed fact"),
        }
    }
}
