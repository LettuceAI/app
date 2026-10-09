use super::tests::{RecordingStream, Reply, harness, launch, send};
use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_types::RequestId;
use lettuce_usage::JobUsageLedger;
use std::sync::Arc;

#[tokio::test]
async fn openrouter_chat_admits_a_durable_automatic_cost_job_after_the_reply() {
    use lettuce_jobs::{JobKind, JobQuery, JobState, JobStore};
    use lettuce_types::PageRequest;
    let h = harness(Reply::Text("Priced reply."));
    super::usage_billing_tests::ready_openrouter(&h);
    *h.provider.response_hook.lock().expect("response hook") = Some(Arc::new(|_, response| {
        response.usage = Some(lettuce_conversations::InferenceUsage {
            input_tokens: 120,
            output_tokens: 30,
            image_tokens: None,
            audio_tokens: None,
            total_tokens: Some(150),
            cached_input_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
            web_search_requests: None,
            provider_reported_cost: None,
        });
    }));
    let conversation = launch(&h, "automatic-cost-launch").await;
    let stream = Arc::new(RecordingStream::default());
    send(
        &h,
        &conversation,
        "automatic-cost-send",
        "Hello",
        stream.clone(),
    )
    .await
    .expect("send");
    super::ConversationGenerationWorker::new(h.context.clone())
        .run_once()
        .await
        .expect("reply");
    assert!(matches!(
        stream.events().last(),
        Some(dto::GenerationEvent::Completed { .. })
    ));
    assert_eq!(h.provider.requests.lock().expect("requests").len(), 1);
    let queued = h
        .context
        .backend()
        .database()
        .list(JobQuery {
            state: Some(JobState::Queued),
            kind: Some(JobKind::Maintenance),
            subject: None,
            page: PageRequest::default(),
        })
        .expect("automatic jobs");
    assert_eq!(queued.items.len(), 1);
    assert_eq!(queued.items[0].subject.id.as_str(), "usage-cost-capture");
    let dispatch = h
        .context
        .backend()
        .database()
        .job_usage(
            h.provider.requests.lock().expect("requests")[0]
                .cancellation
                .expect("chat job"),
        )
        .expect("dispatch");
    assert_eq!(dispatch.len(), 1);
    assert_eq!(
        dispatch[0]
            .snapshot
            .as_ref()
            .expect("snapshot")
            .provider_kind
            .as_deref(),
        Some("openrouter")
    );
}

#[tokio::test]
async fn chat_usage_keeps_dispatch_names_after_in_flight_catalog_edits() {
    use lettuce_models::{ModelProfileRepository, ProviderAccountRepository};
    use lettuce_usage::UsageLedger;
    let h = harness(Reply::Text("Hello."));
    let conversation = launch(&h, "snapshot-launch").await;
    let context = h.context.clone();
    *h.provider.response_hook.lock().expect("response hook") = Some(Arc::new(move |request, _| {
        let database = context.backend().database();
        let mut account = ProviderAccountRepository::get(
            database,
            request.profile.chat_profile.provider_account_id,
        )
        .expect("account")
        .expect("account exists");
        let revision = account.revision;
        account.label = "Changed account".into();
        ProviderAccountRepository::upsert(database, account, Some(revision))
            .expect("rename account while inference runs");
        let mut model =
            ModelProfileRepository::get(database, request.profile.chat_profile.model_profile_id)
                .expect("model")
                .expect("model exists");
        let revision = model.revision;
        model.display_name = "Changed model".into();
        ModelProfileRepository::upsert(database, model, Some(revision))
            .expect("rename model while inference runs");
    }));
    let accepted = send(
        &h,
        &conversation,
        "snapshot-send",
        "Hello",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    assert!(
        super::ConversationGenerationWorker::new(h.context.clone())
            .run_once()
            .await
            .expect("run chat")
    );
    let request = h.provider.requests.lock().expect("requests")[0].clone();
    let database = h.context.backend().database();
    let event = database
        .get_for_attempt(accepted.turn_id.parse().expect("turn"), request.attempt_id)
        .expect("terminal usage")
        .expect("event");
    let document = serde_json::to_value(event).expect("usage document");
    assert_eq!(
        document["record"]["snapshot"]["character_id"],
        h.character_id.to_string()
    );
    assert_eq!(document["record"]["snapshot"]["character_name"], "Ada");
    assert_eq!(
        document["record"]["snapshot"]["model_name"],
        request.profile.chat_profile.model_display_name
    );
    assert_eq!(document["record"]["snapshot"]["provider_kind"], "ollama");
    assert_eq!(
        document["record"]["snapshot"]["provider_label"],
        request
            .profile
            .chat_profile
            .provider_label
            .expect("frozen label")
    );
    assert_eq!(document["record"]["snapshot"]["finish_reason"], "stop");
    assert_eq!(
        document["record"]["snapshot"]["provider_response_id"],
        "fake-response"
    );
    let dispatches = database
        .job_usage(request.cancellation.expect("owning job"))
        .expect("dispatches");
    assert_eq!(dispatches.len(), 1);
    let document = serde_json::to_value(&dispatches[0]).expect("dispatch document");
    assert_eq!(document["snapshot"]["character_name"], "Ada");
    assert_eq!(
        document["snapshot"]["model_name"],
        request.profile.chat_profile.model_display_name
    );
    assert_eq!(
        document["result"]["Response"]["snapshot"]["provider_response_id"],
        "fake-response"
    );
}

#[tokio::test]
async fn usage_clear_preserves_a_live_chat_then_clears_once_after_completion() {
    let h = harness(Reply::Text("Hello."));
    let conversation = launch(&h, "clear-launch").await;
    let accepted = send(
        &h,
        &conversation,
        "clear-send",
        "Hello",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("test operation succeeds");
    let request = dto::UsageClearBeforeRequest {
        before: i64::MAX,
        client_operation_id: RequestId::new().to_string(),
    };
    assert_eq!(
        super::usage_clear_before(&h.context, request.clone())
            .await
            .expect("test operation succeeds")
            .removed,
        0
    );
    assert!(
        super::ConversationGenerationWorker::new(h.context.clone())
            .run_once()
            .await
            .expect("test operation succeeds")
    );
    let turn = lettuce_conversations::ConversationReader::get_turn(
        h.context.backend().database(),
        accepted.turn_id.parse().expect("test operation succeeds"),
    )
    .expect("test operation succeeds");
    let job = turn
        .attempts
        .last()
        .expect("test operation succeeds")
        .job_id
        .expect("test operation succeeds");
    let dispatches = h
        .context
        .backend()
        .database()
        .job_usage(job)
        .expect("test operation succeeds");
    assert_eq!(dispatches.len(), 1);
    assert!(dispatches[0].result.is_some());
    let next = dto::UsageClearBeforeRequest {
        client_operation_id: RequestId::new().to_string(),
        ..request.clone()
    };
    assert_eq!(
        super::usage_clear_before(&h.context, next.clone())
            .await
            .expect("test operation succeeds")
            .removed,
        2
    );
    assert!(
        h.context
            .backend()
            .database()
            .job_usage(job)
            .expect("test operation succeeds")
            .is_empty()
    );
    assert_eq!(
        super::usage_clear_before(&h.context, next.clone())
            .await
            .expect("test operation succeeds")
            .removed,
        2
    );
    let changed = dto::UsageClearBeforeRequest {
        before: i64::MAX - 1,
        ..next
    };
    assert_eq!(
        super::usage_clear_before(&h.context, changed)
            .await
            .expect_err("test operation fails")
            .code,
        ApiErrorCode::Conflict
    );
    super::conversation_open(
        &h.context,
        dto::ConversationOpenRequest {
            conversation_id: conversation,
        },
    )
    .await
    .expect("test operation succeeds");
}

#[tokio::test(flavor = "multi_thread")]
async fn usage_clear_keeps_a_settled_dispatch_while_its_chat_attempt_is_running() {
    use lettuce_usage::{JobInferenceUsageResult, UsageLedger};
    let h = harness(Reply::Text("Hello."));
    let conversation = launch(&h, "running-clear-launch").await;
    let accepted = send(
        &h,
        &conversation,
        "running-clear-send",
        "Hello",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("test operation succeeds");
    let turn_id = accepted.turn_id.parse().expect("test operation succeeds");
    let turn = lettuce_conversations::ConversationReader::get_turn(
        h.context.backend().database(),
        turn_id,
    )
    .expect("test operation succeeds");
    let attempt_id = turn.attempts[0].id;
    let job_id = turn.attempts[0].job_id.expect("test operation succeeds");
    let settled = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    *h.provider
        .response_release
        .lock()
        .expect("test operation succeeds") = Some(release.clone());
    let context = h.context.clone();
    let notify = settled.clone();
    *h.provider
        .response_hook
        .lock()
        .expect("test operation succeeds") = Some(Arc::new(move |request, outcome| {
        outcome.usage = Some(lettuce_conversations::InferenceUsage {
            input_tokens: 10,
            output_tokens: 3,
            cached_input_tokens: None,
            reasoning_tokens: None,
            cache_write_tokens: None,
            web_search_requests: None,
            image_tokens: None,
            audio_tokens: None,
            total_tokens: Some(13),
            provider_reported_cost: None,
        });
        let database = context.backend().database();
        let dispatch = database
            .job_usage(job_id)
            .expect("test operation succeeds")
            .into_iter()
            .find(|record| record.logical_attempt_id == attempt_id)
            .expect("test operation succeeds");
        database
            .settle_job_usage(
                dispatch.id,
                JobInferenceUsageResult::Response {
                    snapshot: Some(Box::new(
                        crate::jobs::job_inference_usage::settle_usage_snapshot(
                            crate::jobs::job_inference_usage::inference_usage_snapshot(
                                &request.profile,
                                &request.context,
                            ),
                            outcome,
                        ),
                    )),
                    usage: outcome.usage.clone(),
                    provider_response_id: outcome.provider_response_id.clone(),
                },
            )
            .expect("test operation succeeds");
        notify.notify_one();
    }));
    let worker = super::ConversationGenerationWorker::new(h.context.clone());
    let running = tokio::spawn(async move { worker.run_once().await });
    tokio::time::timeout(std::time::Duration::from_secs(10), settled.notified())
        .await
        .expect("test operation succeeds");
    let turn = lettuce_conversations::ConversationReader::get_turn(
        h.context.backend().database(),
        turn_id,
    )
    .expect("test operation succeeds");
    assert_eq!(
        turn.attempts[0].status,
        lettuce_conversations::GenerationAttemptStatus::Running
    );
    let cut = dto::UsageClearBeforeRequest {
        before: i64::MAX,
        client_operation_id: RequestId::new().to_string(),
    };
    assert_eq!(
        super::usage_clear_before(&h.context, cut)
            .await
            .expect("test operation succeeds")
            .removed,
        0
    );
    assert_eq!(
        h.context
            .backend()
            .database()
            .job_usage(job_id)
            .expect("test operation succeeds")
            .len(),
        1
    );
    release.notify_one();
    assert!(
        running
            .await
            .expect("test operation succeeds")
            .expect("test operation succeeds")
    );
    let event = h
        .context
        .backend()
        .database()
        .get_for_attempt(turn_id, attempt_id)
        .expect("test operation succeeds")
        .expect("test operation succeeds");
    let lettuce_conversations::UsageCounters::Known(usage) = event.record.usage else {
        panic!("terminal usage is missing");
    };
    assert_eq!(usage.input_tokens, 10);
    assert_eq!(usage.output_tokens, 3);
    assert_eq!(usage.total_tokens, Some(13));
}

#[tokio::test]
async fn cleared_chat_usage_exports_and_restores_with_exact_tombstone_proofs() {
    use lettuce_transfer::{ProviderBackupRestoreWriter, ProviderBackupSource};
    let h = harness(Reply::Text("Hello."));
    let conversation = launch(&h, "proof-launch").await;
    send(
        &h,
        &conversation,
        "proof-send",
        "Hello",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("test operation succeeds");
    super::ConversationGenerationWorker::new(h.context.clone())
        .run_once()
        .await
        .expect("test operation succeeds");
    let original = h
        .context
        .backend()
        .database()
        .read_provider_backup_graph()
        .expect("test operation succeeds");
    assert_eq!(original.conversation_usage.events.len(), 1);
    let cut = dto::UsageClearBeforeRequest {
        before: i64::MAX,
        client_operation_id: RequestId::new().to_string(),
    };
    assert_eq!(
        super::usage_clear_before(&h.context, cut)
            .await
            .expect("test operation succeeds")
            .removed,
        2
    );
    let mut graph = h
        .context
        .backend()
        .database()
        .read_provider_backup_graph()
        .expect("test operation succeeds");
    lettuce_transfer::canonicalize_and_validate(&mut graph).expect("test operation succeeds");
    assert_eq!(graph.conversation_usage.version, 3);
    assert_eq!(graph.conversation_usage.tombstones.len(), 2);
    let mut missing = graph.clone();
    missing
        .conversation_usage
        .tombstones
        .retain(|proof| !matches!(proof, lettuce_usage::UsageTombstone::Conversation { .. }));
    assert!(lettuce_transfer::canonicalize_and_validate(&mut missing).is_err());
    let mut forged = graph.clone();
    for proof in &mut forged.conversation_usage.tombstones {
        if let lettuce_usage::UsageTombstone::Conversation { event_id, .. } = proof {
            *event_id = lettuce_types::UsageEventId::new();
        }
    }
    assert!(lettuce_transfer::canonicalize_and_validate(&mut forged).is_err());
    let mut wrong_owner = graph.clone();
    for proof in &mut wrong_owner.conversation_usage.tombstones {
        if let lettuce_usage::UsageTombstone::Conversation { attempt_id, .. } = proof {
            *attempt_id = lettuce_types::GenerationAttemptId::new();
        }
    }
    assert!(lettuce_transfer::canonicalize_and_validate(&mut wrong_owner).is_err());
    let mut wrong_dispatch = graph.clone();
    for proof in &mut wrong_dispatch.conversation_usage.tombstones {
        if let lettuce_usage::UsageTombstone::Dispatch { attempt_id, .. } = proof {
            *attempt_id = lettuce_types::GenerationAttemptId::new();
        }
    }
    assert!(lettuce_transfer::canonicalize_and_validate(&mut wrong_dispatch).is_err());
    let mut wrong_job = graph.clone();
    for proof in &mut wrong_job.conversation_usage.tombstones {
        if let lettuce_usage::UsageTombstone::Dispatch { job_id, .. } = proof {
            *job_id = lettuce_types::JobId::new();
        }
    }
    assert!(lettuce_transfer::canonicalize_and_validate(&mut wrong_job).is_err());
    let mut duplicate = graph.clone();
    duplicate
        .conversation_usage
        .tombstones
        .push(duplicate.conversation_usage.tombstones[0].clone());
    assert!(lettuce_transfer::canonicalize_and_validate(&mut duplicate).is_err());
    let mut old = original;
    old.conversation_usage.version = 1;
    lettuce_transfer::canonicalize_and_validate(&mut old).expect("test operation succeeds");
    let mut mislabeled = graph.clone();
    mislabeled.conversation_usage.version = 1;
    assert!(lettuce_transfer::canonicalize_and_validate(&mut mislabeled).is_err());
    let target = lettuce_database::Database::open_in_memory().expect("test operation succeeds");
    use lettuce_conversations::{ConversationArtifactTransferPort, TrustedArtifactDescriptor};
    let mut artifacts = Vec::new();
    for descriptor in lettuce_transfer::provider_backup_artifact_requirements(&graph)
        .expect("test operation succeeds")
    {
        let mut sink = ArtifactBytes::default();
        match &descriptor {
            TrustedArtifactDescriptor::Snapshot(reference) => h
                .context
                .backend()
                .database()
                .export_snapshot(reference.artifact_id, &mut sink)
                .expect("test operation succeeds"),
            TrustedArtifactDescriptor::Replay(reference) => h
                .context
                .backend()
                .database()
                .export_replay(reference.artifact_id, &mut sink)
                .expect("test operation succeeds"),
        }
        artifacts.push(lettuce_transfer::BackupConversationArtifact {
            descriptor,
            bytes: sink.0.into(),
        });
    }
    target
        .restore_provider_backup_graph(&graph, &artifacts)
        .expect("test operation succeeds");
    let mut restored = target
        .read_provider_backup_graph()
        .expect("test operation succeeds");
    lettuce_transfer::canonicalize_and_validate(&mut restored).expect("test operation succeeds");
    assert!(restored.conversation_usage.events.is_empty());
    assert_eq!(
        restored.conversation_usage.tombstones,
        graph.conversation_usage.tombstones
    );
}

#[derive(Default)]
struct ArtifactBytes(Vec<u8>);

impl lettuce_conversations::TrustedArtifactSink for ArtifactBytes {
    fn begin(
        &mut self,
        _: &lettuce_conversations::TrustedArtifactDescriptor,
    ) -> Result<(), lettuce_conversations::ArtifactTransferError> {
        Ok(())
    }
    fn chunk(&mut self, bytes: &[u8]) -> Result<(), lettuce_conversations::ArtifactTransferError> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
    fn finish(&mut self) -> Result<(), lettuce_conversations::ArtifactTransferError> {
        Ok(())
    }
}
