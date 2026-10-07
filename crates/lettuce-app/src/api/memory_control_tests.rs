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
