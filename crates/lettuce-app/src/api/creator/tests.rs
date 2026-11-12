use super::*;
use crate::api::{
    JobHandlers, JobRunner, job_cancel,
    tests::{Reply, harness},
};
use lettuce_conversations::ProposedToolCall;
use lettuce_jobs::{JobState, JobStore};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn start_request(kind: dto::CreatorKind, key: &str) -> dto::CreatorStartRequest {
    dto::CreatorStartRequest {
        kind,
        target_id: None,
        draft: None,
        client_operation_id: key.into(),
    }
}

fn revision(work: &dto::CreatorView, key: &str) -> dto::CreatorRevisionRequest {
    dto::CreatorRevisionRequest {
        workflow_id: work.summary.id.clone(),
        expected_revision: work.revision,
        proposal_id: work.proposal.id.clone(),
        client_operation_id: key.into(),
    }
}

async fn edited(context: &ApiContext, work: &dto::CreatorView, key: &str) -> dto::CreatorView {
    creator_edit(
        context,
        dto::CreatorEditRequest {
            workflow_id: work.summary.id.clone(),
            expected_revision: work.revision,
            draft: dto::CreatorDraft {
                name: "Mira".into(),
                definition: "A lighthouse keeper.".into(),
                scenes: Vec::new(),
                avatar: None,
            },
            client_operation_id: key.into(),
        },
    )
    .await
    .expect("edit")
}

async fn get(context: &ApiContext, work: &dto::CreatorView) -> dto::CreatorView {
    creator_get(
        context,
        dto::CreatorGetRequest {
            workflow_id: work.summary.id.clone(),
        },
    )
    .await
    .expect("get")
}

fn send_request(work: &dto::CreatorView, key: &str) -> dto::CreatorSendRequest {
    dto::CreatorSendRequest {
        workflow_id: work.summary.id.clone(),
        expected_revision: work.revision,
        message: "Write a lighthouse keeper".into(),
        references: Vec::new(),
        images: Vec::new(),
        model_profile_id: None,
        client_operation_id: key.into(),
    }
}

async fn run(context: &ApiContext) {
    let runner = JobRunner::new(context.clone(), JobHandlers::standard());
    assert!(runner.run_once().await.expect("run"));
    tokio::time::timeout(std::time::Duration::from_secs(5), runner.wait_idle())
        .await
        .expect("settle");
}

#[tokio::test]
async fn creator_manual_edits_review_and_apply_are_durable_and_replay_safe() {
    for kind in [dto::CreatorKind::Character, dto::CreatorKind::Persona] {
        let harness = harness(Reply::Text("unused"));
        let context = &harness.context;
        let request = start_request(kind, "start");
        let work = creator_start(context, request.clone())
            .await
            .expect("start");
        assert_eq!(
            creator_start(context, request)
                .await
                .expect("replay")
                .summary
                .id,
            work.summary.id
        );
        let work = edited(context, &work, "edit").await;
        assert!(!work.reviewed);
        assert_eq!(
            creator_apply(context, revision(&work, "unreviewed"))
                .await
                .expect_err("review required")
                .code,
            ApiErrorCode::Conflict
        );
        let reviewed = creator_review(context, revision(&work, "review"))
            .await
            .expect("review without model");
        assert!(reviewed.reviewed);
        let apply = revision(&reviewed, "apply");
        let saved = creator_apply(context, apply.clone()).await.expect("apply");
        assert!(saved.summary.applied_id.is_some());
        assert_eq!(
            creator_apply(context, apply)
                .await
                .expect("apply replay")
                .summary
                .applied_id,
            saved.summary.applied_id
        );
        assert_eq!(
            creator_edit(
                context,
                dto::CreatorEditRequest {
                    workflow_id: saved.summary.id.clone(),
                    expected_revision: saved.revision,
                    draft: saved.proposal.draft.clone(),
                    client_operation_id: "closed".into()
                }
            )
            .await
            .expect_err("closed")
            .code,
            ApiErrorCode::Conflict
        );
        let page = creators_list(
            context,
            dto::CreatorsListRequest {
                cursor: None,
                limit: Some(1),
            },
        )
        .await
        .expect("list");
        assert_eq!(page.items[0].applied_id, saved.summary.applied_id);
        assert!(
            harness
                .provider
                .requests
                .lock()
                .expect("requests")
                .is_empty()
        );
    }
}

#[tokio::test]
async fn creator_job_admission_is_atomic_and_serializes_workflow_writers() {
    let harness = harness(Reply::Text("Hello"));
    let context = &harness.context;
    let work = creator_start(context, start_request(dto::CreatorKind::Persona, "start"))
        .await
        .expect("start");
    let mut invalid = send_request(&work, "invalid");
    invalid.message.clear();
    assert_eq!(
        creator_send(context, invalid)
            .await
            .expect_err("empty input")
            .code,
        ApiErrorCode::InvalidInput
    );
    let jobs = crate::api::jobs_list(
        context,
        dto::JobsListRequest {
            kinds: Some(vec![dto::JobKindDto::CreationRun]),
            states: None,
            subject: None,
            cursor: None,
            limit: None,
        },
    )
    .await
    .expect("jobs");
    assert!(jobs.items.is_empty());
    let request = send_request(&work, "send");
    let (first, second) = tokio::join!(
        creator_send(context, request.clone()),
        creator_send(context, request)
    );
    let first = first.expect("first");
    assert_eq!(first.job_id, second.expect("replay").job_id);
    assert_eq!(
        creator_send(context, send_request(&work, "another-send"))
            .await
            .expect_err("single writer")
            .code,
        ApiErrorCode::Conflict
    );
    assert_eq!(
        creator_edit(
            context,
            dto::CreatorEditRequest {
                workflow_id: work.summary.id.clone(),
                expected_revision: work.revision,
                draft: work.proposal.draft.clone(),
                client_operation_id: "edit-during-job".into()
            }
        )
        .await
        .expect_err("fenced")
        .code,
        ApiErrorCode::Conflict
    );
    job_cancel(
        context,
        dto::JobCancelRequest {
            job_id: first.job_id,
        },
    )
    .await
    .expect("cancel queued");
    let stopped = get(context, &work).await;
    assert!(stopped.active_job_id.is_none());
    assert_eq!(stopped.proposal, work.proposal);
    assert_eq!(
        edited(context, &stopped, "after-stop")
            .await
            .proposal
            .draft
            .name,
        "Mira"
    );
}

#[tokio::test]
async fn creator_model_confirmation_does_not_replace_user_review() {
    let harness = harness(Reply::Text("The draft is ready."));
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    *harness.provider.response_hook.lock().expect("hook") = Some(Arc::new(move |_, outcome| {
        if observed.fetch_add(1, Ordering::SeqCst) == 0 {
            outcome.candidates[0].tool_calls = vec![
                ("set_name", serde_json::json!({"name":"Mira"})),
                (
                    "write_definition",
                    serde_json::json!({"definition":"A lighthouse keeper."}),
                ),
                ("request_confirmation", serde_json::json!({})),
            ]
            .into_iter()
            .enumerate()
            .map(|(index, (name, arguments))| ProposedToolCall {
                provider_call_id: Some(format!("call-{index}")),
                name: name.into(),
                arguments,
                raw_arguments: None,
                provider_replay: None,
            })
            .collect();
        }
    }));
    let context = &harness.context;
    let work = creator_start(context, start_request(dto::CreatorKind::Persona, "start"))
        .await
        .expect("start");
    let job = creator_send(context, send_request(&work, "send"))
        .await
        .expect("send");
    run(context).await;
    let finished = get(context, &work).await;
    assert_eq!(
        JobStore::get(
            context.backend().database(),
            job.job_id.parse().expect("id")
        )
        .expect("job")
        .expect("exists")
        .state,
        JobState::Succeeded
    );
    assert_eq!(finished.proposal.draft.name, "Mira");
    assert_eq!(finished.turns[0].status, dto::CreatorTurnStatus::Succeeded);
    assert_eq!(
        finished.turns[0]
            .tools
            .iter()
            .map(|tool| (tool.name.as_str(), tool.status))
            .collect::<Vec<_>>(),
        vec![
            ("set_name", dto::CreatorToolStatus::Succeeded),
            ("write_definition", dto::CreatorToolStatus::Succeeded),
            ("request_confirmation", dto::CreatorToolStatus::Succeeded),
        ]
    );
    assert!(harness.events.events().iter().any(|event| matches!(event,dto::ApiEvent::JobUpdated { job: updated } if updated.id==job.job_id && updated.state==dto::JobStateDto::Running)));
    assert!(!finished.reviewed);
    assert_eq!(
        creator_apply(context, revision(&finished, "unsafe-apply"))
            .await
            .expect_err("human review")
            .code,
        ApiErrorCode::Conflict
    );
    let reviewed = creator_review(context, revision(&finished, "review"))
        .await
        .expect("review");
    assert!(
        creator_apply(context, revision(&reviewed, "apply"))
            .await
            .expect("apply")
            .summary
            .applied_id
            .is_some()
    );
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn creator_cancelled_inference_preserves_the_last_completed_draft() {
    let harness = harness(Reply::UntilCancelled);
    let context = &harness.context;
    let initial = creator_start(context, start_request(dto::CreatorKind::Persona, "start"))
        .await
        .expect("start");
    let work = edited(context, &initial, "edit").await;
    let job = creator_send(context, send_request(&work, "send"))
        .await
        .expect("send");
    let runner = JobRunner::new(context.clone(), JobHandlers::standard());
    assert!(runner.run_once().await.expect("run"));
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        harness.provider.entered.notified(),
    )
    .await
    .expect("provider entered");
    job_cancel(context, dto::JobCancelRequest { job_id: job.job_id })
        .await
        .expect("stop");
    tokio::time::timeout(std::time::Duration::from_secs(5), runner.wait_idle())
        .await
        .expect("stopped");
    let stopped = get(context, &work).await;
    assert_eq!(stopped.proposal, work.proposal);
    assert_eq!(stopped.turns[0].status, dto::CreatorTurnStatus::Cancelled);
    assert!(stopped.active_job_id.is_none());
    assert!(stopped.turns[0].regenerable);
    creator_regenerate(
        context,
        dto::CreatorRegenerateRequest {
            workflow_id: stopped.summary.id.clone(),
            expected_revision: stopped.revision,
            turn_id: stopped.turns[0].id.clone(),
            model_profile_id: None,
            client_operation_id: "retry".into(),
        },
    )
    .await
    .expect("retry the stopped turn");
    let retried = get(context, &work).await;
    assert!(retried.turns[0].regenerated);
    assert_eq!(retried.turns[1].message, stopped.turns[0].message);
    assert_eq!(retried.turns[1].status, dto::CreatorTurnStatus::Pending);
}

#[tokio::test]
async fn creator_review_cannot_copy_a_proposal_from_another_workflow() {
    let harness = harness(Reply::Text("unused"));
    let context = &harness.context;
    let first = creator_start(context, start_request(dto::CreatorKind::Persona, "first"))
        .await
        .expect("first");
    let second = creator_start(context, start_request(dto::CreatorKind::Persona, "second"))
        .await
        .expect("second");
    let second = edited(context, &second, "edit-second").await;
    let mut request = revision(&first, "wrong-proposal");
    request.proposal_id = second.proposal.id;
    assert_eq!(
        creator_review(context, request)
            .await
            .expect_err("cross-workflow review")
            .code,
        ApiErrorCode::Conflict
    );
    assert_eq!(get(context, &first).await.proposal, first.proposal);
}

#[tokio::test]
async fn creator_existing_target_cas_preserves_unrelated_fields() {
    let harness = harness(Reply::Text("unused"));
    let context = &harness.context;
    let db = context.backend().database();
    let work = creator_start(
        context,
        dto::CreatorStartRequest {
            kind: dto::CreatorKind::Character,
            target_id: Some(harness.character_id.to_string()),
            draft: None,
            client_operation_id: "start".into(),
        },
    )
    .await
    .expect("start");
    let draft = edited(context, &work, "edit").await;
    assert_eq!(
        CharacterRepository::get(db, harness.character_id)
            .expect("character")
            .expect("exists")
            .character
            .profile
            .name,
        "Ada"
    );
    let reviewed = creator_review(context, revision(&draft, "review"))
        .await
        .expect("review");
    creator_apply(context, revision(&reviewed, "apply"))
        .await
        .expect("save");
    let character = CharacterRepository::get(db, harness.character_id)
        .expect("character")
        .expect("exists");
    assert_eq!(character.character.profile.name, "Mira");
    assert_eq!(
        character.character.profile.description.as_deref(),
        Some("A meticulous engineer")
    );
    let existing = |key: &str, draft: Option<dto::CreatorDraft>| dto::CreatorStartRequest {
        kind: dto::CreatorKind::Character,
        target_id: Some(harness.character_id.to_string()),
        draft,
        client_operation_id: key.into(),
    };
    let stale = creator_start(context, existing("another-start", None))
        .await
        .expect("start");
    assert_ne!(stale.summary.id, work.summary.id);
    let stale = creator_review(context, revision(&stale, "stale-review"))
        .await
        .expect("review");
    assert_eq!(stale.proposal.source, dto::CreatorProposalSource::Review);
    let resumed = creator_start(context, existing("resume-start", None))
        .await
        .expect("resume");
    assert_eq!(resumed.summary.id, stale.summary.id);
    let mut other = stale.proposal.draft.clone();
    other.name = "Mira Finch".into();
    let fresh = creator_start(context, existing("fresh-start", Some(other)))
        .await
        .expect("start");
    assert_ne!(fresh.summary.id, stale.summary.id);
    let fresh = creator_review(context, revision(&fresh, "fresh-review"))
        .await
        .expect("review");
    creator_apply(context, revision(&fresh, "fresh-apply"))
        .await
        .expect("save");
    assert!(get(context, &stale).await.target_changed);
    assert_eq!(
        creator_apply(context, revision(&stale, "stale-apply"))
            .await
            .expect_err("target changed")
            .code,
        ApiErrorCode::Conflict
    );
    let carried = creator_start(
        context,
        existing("carry-start", Some(stale.proposal.draft.clone())),
    )
    .await
    .expect("carry the draft over");
    assert!(!carried.target_changed);
    assert_eq!(carried.history[0].draft.name, "Mira Finch");
    assert_eq!(carried.proposal.draft, stale.proposal.draft);
    assert_eq!(carried.proposal.source, dto::CreatorProposalSource::Edit);
}

#[tokio::test]
async fn creator_regeneration_keeps_history_and_refuses_to_discard_manual_edits() {
    let harness = harness(Reply::Text("Ready."));
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    *harness.provider.response_hook.lock().expect("hook") = Some(Arc::new(move |_, outcome| {
        let index = observed.fetch_add(1, Ordering::SeqCst);
        if index == 0 || index == 2 {
            outcome.candidates[0].tool_calls = vec![
                (
                    "set_name",
                    serde_json::json!({"name":if index==0 {"Mira"} else {"Nora"}}),
                ),
                (
                    "write_definition",
                    serde_json::json!({"definition":"A lighthouse keeper."}),
                ),
            ]
            .into_iter()
            .enumerate()
            .map(|(ordinal, (name, arguments))| ProposedToolCall {
                provider_call_id: Some(format!("call-{index}-{ordinal}")),
                name: name.into(),
                arguments,
                raw_arguments: None,
                provider_replay: None,
            })
            .collect();
        }
    }));
    let context = &harness.context;
    let initial = creator_start(context, start_request(dto::CreatorKind::Persona, "start"))
        .await
        .expect("start");
    creator_send(context, send_request(&initial, "send"))
        .await
        .expect("send");
    run(context).await;
    let first = get(context, &initial).await;
    assert!(first.turns[0].regenerable);
    creator_regenerate(
        context,
        dto::CreatorRegenerateRequest {
            workflow_id: first.summary.id.clone(),
            expected_revision: first.revision,
            turn_id: first.turns[0].id.clone(),
            model_profile_id: None,
            client_operation_id: "regenerate".into(),
        },
    )
    .await
    .expect("regenerate");
    run(context).await;
    let regenerated = get(context, &initial).await;
    assert_eq!(regenerated.proposal.draft.name, "Nora");
    assert!(
        regenerated
            .history
            .iter()
            .any(|proposal| proposal.draft.name == "Mira")
    );
    assert!(regenerated.turns[0].regenerated);
    let manual = edited(context, &regenerated, "manual-edit").await;
    assert!(!manual.turns.last().expect("turn").regenerable);
    assert_eq!(manual.proposal.source, dto::CreatorProposalSource::Edit);
    assert_eq!(
        creator_regenerate(
            context,
            dto::CreatorRegenerateRequest {
                workflow_id: manual.summary.id.clone(),
                expected_revision: manual.revision,
                turn_id: regenerated.turns.last().expect("turn").id.clone(),
                model_profile_id: None,
                client_operation_id: "discard-manual".into()
            }
        )
        .await
        .expect_err("keep manual revision")
        .code,
        ApiErrorCode::Conflict
    );
    let kept = get(context, &manual).await;
    assert_eq!(kept.proposal, manual.proposal);
    assert!(kept.active_job_id.is_none());
}

#[tokio::test]
async fn creator_fallbacks_pin_settings_and_hide_protocol_text() {
    for (fallback, envelope) in [
        (
            lettuce_settings::CreationHelperToolFallback::Json,
            r#"{"calls":[{"name":"set_name","arguments":{"name":"Mira"}},{"name":"write_definition","arguments":{"definition":"A keeper."}},{"name":"reply","arguments":{"message":"Drafted Mira."}}]}"#,
        ),
        (
            lettuce_settings::CreationHelperToolFallback::Xml,
            r#"<calls><call name="set_name"><arg name="name">Mira</arg></call><call name="write_definition"><arg name="definition">A keeper.</arg></call><call name="reply"><arg name="message">Drafted Mira.</arg></call></calls>"#,
        ),
    ] {
        let harness = harness(Reply::Text("Done."));
        let context = &harness.context;
        let db = context.backend().database();
        let stored = GlobalSettingsStore::load(db).expect("settings");
        let mut settings = stored.settings;
        settings.creation_helper.tool_fallback = fallback;
        GlobalSettingsStore::save(
            db,
            settings,
            stored.default_model_profile_id,
            stored.revision,
        )
        .expect("fallback settings");
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        *harness.provider.response_hook.lock().expect("hook") =
            Some(Arc::new(move |_, outcome| {
                if observed.fetch_add(1, Ordering::SeqCst) == 0 {
                    outcome.candidates[0].parts = vec![MessagePart::Text {
                        text: envelope.into(),
                    }];
                }
            }));
        let initial = creator_start(context, start_request(dto::CreatorKind::Persona, "start"))
            .await
            .expect("start");
        creator_send(context, send_request(&initial, "send"))
            .await
            .expect("send");
        let stored = GlobalSettingsStore::load(db).expect("settings");
        let mut settings = stored.settings;
        settings.creation_helper.tool_fallback =
            lettuce_settings::CreationHelperToolFallback::Native;
        GlobalSettingsStore::save(db, settings, None, stored.revision)
            .expect("change after admission");
        run(context).await;
        let finished = get(context, &initial).await;
        assert_eq!(finished.proposal.draft.name, "Mira");
        assert_eq!(finished.proposal.draft.definition, "A keeper.");
        assert_eq!(finished.turns[0].reply, "Drafted Mira.\n\nDone.");
        assert_eq!(finished.turns[0].tools.len(), 2);
        let requests = harness.provider.requests.lock().expect("requests");
        assert_eq!(requests.len(), 2);
        assert!(requests.iter().all(|request| request.tools.is_none()
            && request.stream_sink.is_none()
            && request.profile.tool_policy == lettuce_conversations::ToolPolicy::Disabled));
    }
}

#[tokio::test]
async fn creator_invalid_tool_is_reported_without_mutating_the_draft() {
    let harness = harness(Reply::Text("Done."));
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    *harness.provider.response_hook.lock().expect("hook") = Some(Arc::new(move |_, outcome| {
        if observed.fetch_add(1, Ordering::SeqCst) == 0 {
            outcome.candidates[0].tool_calls = vec![ProposedToolCall {
                provider_call_id: Some("bad-tool".into()),
                name: "delete_everything".into(),
                arguments: serde_json::json!({}),
                raw_arguments: None,
                provider_replay: None,
            }];
        }
    }));
    let context = &harness.context;
    let initial = creator_start(context, start_request(dto::CreatorKind::Persona, "start"))
        .await
        .expect("start");
    creator_send(context, send_request(&initial, "send"))
        .await
        .expect("send");
    run(context).await;
    let finished = get(context, &initial).await;
    assert_eq!(finished.proposal.draft, initial.proposal.draft);
    assert_eq!(finished.turns[0].status, dto::CreatorTurnStatus::Succeeded);
    assert_eq!(
        finished.turns[0].tools,
        vec![dto::CreatorToolView {
            name: "delete_everything".into(),
            status: dto::CreatorToolStatus::Failed,
            image: None,
        }]
    );
}

#[tokio::test]
async fn creator_restart_interrupts_without_redispatching_or_losing_the_draft() {
    use lettuce_creation::{CreationAttemptRepository, CreationAttemptStatus};
    let harness = harness(Reply::Text("Done."));
    let context = &harness.context;
    let initial = creator_start(context, start_request(dto::CreatorKind::Persona, "start"))
        .await
        .expect("start");
    let work = edited(context, &initial, "edit").await;
    let request = send_request(&work, "send");
    let accepted = creator_send(context, request.clone()).await.expect("send");
    let db = context.backend().database();
    let job_id = accepted.job_id.parse().expect("job id");
    let claim = db
        .claim(
            job_id,
            lettuce_jobs::WorkerId::new(),
            context.now(),
            std::time::Duration::from_secs(3600),
            &lettuce_jobs::ResourceAvailability::all(),
        )
        .expect("claim")
        .expect("eligible");
    db.append_and_transition(lettuce_jobs::JobMutation::Start {
        claim: claim.claim,
        at: context.now(),
    })
    .expect("start job");
    let workspace = db
        .creator_workspace(work.summary.id.parse().expect("id"))
        .expect("workspace");
    let attempt = &workspace.dialogue[0].attempt;
    db.transition_creation_attempt(
        attempt.id,
        attempt.revision,
        CreationAttemptStatus::Running,
        None,
        context.now(),
    )
    .expect("start attempt");
    let restarted = context.restarted();
    restarted.recover_after_restart().expect("recover");
    let recovered = get(&restarted, &work).await;
    assert_eq!(
        recovered.turns[0].status,
        dto::CreatorTurnStatus::Interrupted
    );
    assert_eq!(recovered.proposal, work.proposal);
    assert!(recovered.active_job_id.is_none());
    assert_eq!(
        creator_send(&restarted, request)
            .await
            .expect("replay interrupted job"),
        accepted
    );
    let runner = JobRunner::new(restarted.clone(), JobHandlers::standard());
    assert!(!runner.run_once().await.expect("no automatic dispatch"));
    assert!(
        harness
            .provider
            .requests
            .lock()
            .expect("requests")
            .is_empty()
    );
    assert_eq!(
        edited(&restarted, &recovered, "after-restart")
            .await
            .proposal
            .draft
            .name,
        "Mira"
    );
}

#[tokio::test]
async fn creator_invalid_response_preserves_the_draft_and_releases_the_writer() {
    let harness = harness(Reply::Text("Invalid response."));
    *harness.provider.response_hook.lock().expect("hook") =
        Some(Arc::new(|_, outcome| outcome.candidates.clear()));
    let context = &harness.context;
    let initial = creator_start(context, start_request(dto::CreatorKind::Persona, "start"))
        .await
        .expect("start");
    let work = edited(context, &initial, "edit").await;
    creator_send(context, send_request(&work, "send"))
        .await
        .expect("send");
    run(context).await;
    let failed = get(context, &work).await;
    assert_eq!(failed.proposal, work.proposal);
    assert_eq!(failed.turns[0].status, dto::CreatorTurnStatus::Failed);
    assert!(failed.active_job_id.is_none());
    assert_eq!(harness.provider.requests.lock().expect("requests").len(), 1);
    assert_eq!(
        edited(context, &failed, "after-failure")
            .await
            .proposal
            .draft
            .name,
        "Mira"
    );
}

#[tokio::test]
async fn creator_attachments_reach_the_prompt_and_the_avatar_is_saved() {
    let root = std::env::temp_dir().join(format!("lettuce-creator-media-{}", RequestId::new()));
    let harness = crate::api::tests::media_harness(Reply::Text("Done."), &root);
    let context = &harness.context;
    let image = context
        .media()
        .expect("media")
        .ingest(
            crate::api::tests::png_bytes().as_slice(),
            lettuce_media::IngestRequest::new(
                lettuce_media::AssetKind::OtherImage,
                lettuce_media::AssetOrigin::Upload,
                lettuce_media::RetentionClass::Persistent,
                lettuce_media::AssetProvenanceV1::default(),
            ),
        )
        .expect("image")
        .asset
        .id;
    let work = creator_start(context, start_request(dto::CreatorKind::Persona, "start"))
        .await
        .expect("start");
    let mut request = send_request(&work, "send");
    request.references = vec![dto::CreatorReference {
        kind: dto::CreatorKind::Character,
        id: harness.character_id.to_string(),
    }];
    request.images = vec![image.to_string()];
    creator_send(context, request).await.expect("send");
    run(context).await;
    let answered = get(context, &work).await;
    assert_eq!(answered.turns[0].references[0].name.as_deref(), Some("Ada"));
    assert_eq!(answered.turns[0].images[0].asset_id, image.to_string());
    assert_eq!(answered.images[0].origin, dto::CreatorImageOrigin::Upload);
    {
        let requests = harness.provider.requests.lock().expect("requests");
        let user = requests[0]
            .context
            .messages
            .iter()
            .rev()
            .find(|message| message.role == lettuce_conversations::MessageRole::User)
            .expect("user message");
        let lettuce_conversations::ProviderContextPart::Text { text } = &user.parts[0] else {
            panic!("text part");
        };
        assert!(
            text.starts_with("Write a lighthouse keeper\n\n---\n[Referenced Character: \"Ada\"")
        );
        assert!(text.ends_with(&format!(
            "[Uploaded Image (id:{})]",
            lettuce_creation::creation_image_label(image)
        )));
    }
    let edited = creator_edit(
        context,
        dto::CreatorEditRequest {
            workflow_id: answered.summary.id.clone(),
            expected_revision: answered.revision,
            draft: dto::CreatorDraft {
                name: "Mira".into(),
                definition: "A lighthouse keeper.".into(),
                scenes: Vec::new(),
                avatar: Some(context.image_asset_ref(image)),
            },
            client_operation_id: "avatar".into(),
        },
    )
    .await
    .expect("edit");
    assert_eq!(
        edited
            .proposal
            .draft
            .avatar
            .as_ref()
            .map(|asset| asset.asset_id.clone()),
        Some(image.to_string())
    );
    let reviewed = creator_review(context, revision(&edited, "review"))
        .await
        .expect("review");
    let saved = creator_apply(context, revision(&reviewed, "apply"))
        .await
        .expect("apply");
    let persona: lettuce_types::PersonaId = saved
        .summary
        .applied_id
        .expect("applied")
        .parse()
        .expect("persona id");
    let persona = lettuce_characters::PersonaRepository::get(context.backend().database(), persona)
        .expect("persona")
        .expect("exists");
    assert!(
        persona.media.links.iter().any(|link| link.asset_id == image
            && link.slot == lettuce_characters::PersonaMediaSlot::Avatar)
    );
    drop(harness);
    let _ = std::fs::remove_dir_all(root);
}
