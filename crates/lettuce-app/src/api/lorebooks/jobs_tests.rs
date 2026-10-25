use crate::api::tests::{Reply, harness};
use crate::api::*;
use lettuce_context::{LifecycleStatus, PromptEntryDraft, PromptMetadataDraft, PromptRepository};
use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_creation::LorebookKeywordRunRepository;
use lettuce_jobs::{CancellationReason, JobMutation, JobStore, WorkerId};
use lettuce_settings::GlobalSettingsStore;
use lettuce_types::TimestampMillis;
use std::sync::Arc;

async fn book(harness: &crate::api::tests::Harness) -> dto::LorebookView {
    lorebook_create(
        &harness.context,
        dto::LorebookCreateRequest {
            client_operation_id: "book".into(),
            metadata: dto::LorebookMetadataInput {
                name: "Book".into(),
                detection: dto::LorebookDetection::LatestUserMessage,
                icon_asset_id: None,
            },
            entries: vec![],
        },
    )
    .await
    .expect("book")
}

fn request(book: &dto::LorebookView) -> dto::LorebookKeywordsDraftRequest {
    dto::LorebookKeywordsDraftRequest {
        client_operation_id: "keywords".into(),
        lorebook_id: book.lorebook.id.clone(),
        entry_id: None,
        title: Some("Coast".into()),
        content: "The coastal city.".into(),
        existing_keywords: vec![],
        direction: None,
    }
}

fn custom_prompt(harness: &crate::api::tests::Harness) -> lettuce_context::PromptDocument {
    custom_prompt_for(harness, crate::BuiltInPromptId::LorebookKeywordGenerator)
}

fn custom_prompt_for(
    harness: &crate::api::tests::Harness,
    kind: crate::BuiltInPromptId,
) -> lettuce_context::PromptDocument {
    let db = harness.context.backend().database();
    let seed = PromptRepository::get(
        db,
        harness.context.backend().built_in_prompt_ids().get(kind),
    )
    .expect("seed")
    .expect("exists");
    PromptRepository::create_user_draft(
        db,
        PromptMetadataDraft {
            name: "Keyword source".into(),
            purpose: seed.purpose,
            condense: seed.condense,
            behavior_version: seed.behavior_version,
        },
        seed.entries
            .into_iter()
            .map(|entry| PromptEntryDraft {
                built_in_entry_key: None,
                name: entry.name,
                role: entry.role,
                content: entry.content,
                enabled: entry.enabled,
                injection_position: entry.injection_position,
                depth: entry.depth,
                conditional_min_messages: entry.conditional_min_messages,
                interval_turns: entry.interval_turns,
                system_prompt: entry.system_prompt,
                conditions: entry.conditions,
                payload: entry.payload,
            })
            .collect(),
        TimestampMillis::now().expect("now"),
    )
    .expect("custom prompt")
}

#[tokio::test]
async fn keyword_admission_replays_conflicts_and_keeps_deleted_prompt() {
    let harness = harness(Reply::Text(
        r#"{"tool":"write_lorebook_keywords","arguments":{"keywords":["Coast","coast","City"]}}"#,
    ));
    let book = book(&harness).await;
    let prompt = custom_prompt(&harness);
    let db = harness.context.backend().database();
    let mut stored = GlobalSettingsStore::load(db).expect("settings");
    stored.settings.lorebook_entry_generator.keyword_prompt_id = Some(prompt.id);
    GlobalSettingsStore::save(
        db,
        stored.settings,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("selection");
    let request = request(&book);
    let admitted = lorebook_keywords_draft(&harness.context, request.clone())
        .await
        .expect("admit");
    assert_eq!(
        lorebook_keywords_draft(&harness.context, request.clone())
            .await
            .expect("replay"),
        admitted
    );
    let conflict = lorebook_keywords_draft(
        &harness.context,
        dto::LorebookKeywordsDraftRequest {
            content: "Another city".into(),
            ..request.clone()
        },
    )
    .await
    .expect_err("changed request");
    assert_eq!(conflict.code, ApiErrorCode::Conflict);
    db.delete_prompt(
        prompt.id,
        prompt.revision,
        TimestampMillis::now().expect("now"),
    )
    .expect("delete prompt");
    assert_eq!(
        lorebook_keywords_draft(&harness.context, request)
            .await
            .expect("replay after deletion"),
        admitted
    );
    let job = JobStore::get(db, admitted.job_id.parse().expect("id"))
        .expect("job")
        .expect("exists");
    let handler = LorebookHandler;
    let work = handler
        .claim(&harness.context, &job, WorkerId::new())
        .await
        .expect("claim")
        .expect("claimed");
    assert!(
        handler
            .claim(&harness.context, &job, WorkerId::new())
            .await
            .expect("other worker")
            .is_none()
    );
    work.run(harness.context.clone(), Arc::new(Progress))
        .await
        .expect("run frozen prompt");
    let result = job_get(
        &harness.context,
        dto::JobGetRequest {
            job_id: admitted.job_id,
        },
    )
    .await
    .expect("result");
    assert_eq!(
        result.subject_detail,
        Some(dto::JobSubjectDetail::LorebookDraft {
            lorebook: None,
            prompt: dto::HistoricalSourceView {
                id: prompt.id.to_string(),
                name: "Keyword source".into(),
                deleted: true
            }
        })
    );
    assert_eq!(
        result.result,
        Some(dto::JobResultDto::LorebookKeywords {
            keywords: vec!["Coast".into(), "City".into()]
        })
    );
}

struct Progress;
impl JobProgressSink for Progress {
    fn text_delta(&self, _: Option<String>, _: Option<String>) {}
    fn image_progress(&self, _: dto::ImageProgress) {}
}

#[tokio::test]
async fn configured_archived_prompt_is_typed_and_cancelled_claim_cannot_write() {
    let harness = harness(Reply::Text("ignored"));
    let book = book(&harness).await;
    let prompt = custom_prompt(&harness);
    let db = harness.context.backend().database();
    let mut stored = GlobalSettingsStore::load(db).expect("settings");
    stored.settings.lorebook_entry_generator.keyword_prompt_id = Some(prompt.id);
    GlobalSettingsStore::save(
        db,
        stored.settings,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("selection");
    let archived = PromptRepository::archive(
        db,
        prompt.id,
        prompt.revision,
        TimestampMillis::now().expect("now"),
    )
    .expect("archive");
    assert_eq!(archived.document.status, LifecycleStatus::Archived);
    let error = lorebook_keywords_draft(&harness.context, request(&book))
        .await
        .expect_err("archived configured prompt");
    assert_eq!(
        error.details,
        Some(dto::ApiErrorDetails::ConfiguredPromptUnavailable {
            prompt_id: prompt.id.to_string(),
            reason: dto::ConfiguredPromptProblem::Archived
        })
    );
    PromptRepository::restore(
        db,
        prompt.id,
        archived.document.revision,
        TimestampMillis::now().expect("now"),
    )
    .expect("restore");
    let admitted = lorebook_keywords_draft(&harness.context, request(&book))
        .await
        .expect("admit");
    let id = super::jobs::request_id("lorebook_keywords_draft", "keywords", &request(&book))
        .expect("request id");
    let work = harness
        .context
        .backend()
        .lorebook_keyword_dispatcher()
        .claim(
            id,
            WorkerId::new(),
            TimestampMillis::now().expect("now"),
            std::time::Duration::from_secs(60),
            &lettuce_jobs::ResourceAvailability::all(),
        )
        .expect("claim")
        .expect("work");
    db.append_and_transition(JobMutation::RequestCancellation {
        id: admitted.job_id.parse().expect("id"),
        reason: CancellationReason::User,
        at: TimestampMillis::now().expect("now"),
    })
    .expect("cancel");
    let checkpoint = lettuce_creation::LorebookKeywordAttemptCheckpoint {
        ordinal: 0,
        attempt_kind: lettuce_creation::LorebookKeywordAttemptKind::Native,
        calls: vec![],
        decision: lettuce_creation::LorebookKeywordAttemptDecision::StructuredFallback,
        usage: None,
        provider_finish_reason: None,
        provider_request_id: None,
        completed_at: TimestampMillis::now().expect("now"),
    };
    assert!(
        db.commit_lorebook_keyword_attempt_for_job_attempt(
            id,
            checkpoint,
            Some(work.claim.claim.attempt.get())
        )
        .is_err()
    );
    assert!(
        db.load_lorebook_keyword_attempts(id)
            .expect("attempts")
            .is_empty()
    );
    let run = db.load_lorebook_keyword_run(id).expect("run");
    assert_eq!(run.prompt_name, "Keyword source");
}

#[tokio::test]
async fn project_creation_needs_no_text_model_and_planning_is_typed() {
    let harness = harness(Reply::Text("ignored"));
    let db = harness.context.backend().database();
    let stored = GlobalSettingsStore::load(db).expect("settings");
    let id = stored.default_model_profile_id.expect("model");
    let mut model = lettuce_models::ModelProfileRepository::get(db, id)
        .expect("model")
        .expect("exists");
    let revision = model.revision;
    model.config.capabilities.output_modalities.text =
        lettuce_models::CapabilityStatus::Unsupported;
    lettuce_models::ModelProfileRepository::upsert(db, model, Some(revision))
        .expect("non text model");
    let request = dto::LorebookProjectCreateRequest {
        client_operation_id: "project".into(),
        brief: "A coastal city and its districts".into(),
        lorebook_name: Some("Coast".into()),
        target_count: Some(5),
        sources: vec![],
    };
    let project = lorebook_project_create(&harness.context, request.clone())
        .await
        .expect("create without model");
    assert_eq!(project.stage, dto::LorebookProjectStage::Created);
    assert!(project.active_job_ids.is_empty());
    let restored = super::tests::backup_round_trip(db);
    assert_eq!(
        restored
            .pending_lorebook_project(project.project_id.parse().expect("project id"))
            .expect("restored project")
            .expect("exists")
            .brief,
        project.brief
    );

    assert_eq!(
        lorebook_project_create(&harness.context, request)
            .await
            .expect("replay"),
        project
    );
    let error = lorebook_project_plan(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "plan".into(),
            project_id: project.project_id.clone(),
            expected_revision: project.revision,
        },
    )
    .await
    .expect_err("non text model");
    assert_eq!(error.code, ApiErrorCode::ModelUnavailable);
    let current = lorebook_project_get(
        &harness.context,
        dto::LorebookProjectGetRequest {
            project_id: project.project_id,
        },
    )
    .await
    .expect("still exists");
    assert_eq!(current.stage, dto::LorebookProjectStage::Created);
}

async fn staged_project(
    harness: &crate::api::tests::Harness,
    key: &str,
) -> dto::LorebookProjectView {
    lorebook_project_create(
        &harness.context,
        dto::LorebookProjectCreateRequest {
            client_operation_id: key.into(),
            brief: "A coastal city and its districts".into(),
            lorebook_name: Some("Coast".into()),
            target_count: Some(5),
            sources: vec![],
        },
    )
    .await
    .expect("project")
}

async fn approved_project(
    harness: &crate::api::tests::Harness,
    key: &str,
) -> dto::LorebookProjectView {
    let project = staged_project(harness, key).await;
    let plan = lorebook_project_plan(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: format!("{key}-plan"),
            project_id: project.project_id.clone(),
            expected_revision: project.revision,
        },
    )
    .await
    .expect("plan");
    execute(harness, &plan.job_id).await;
    let planned = project_get(harness, &project.project_id).await;
    lorebook_project_outline_approve(
        &harness.context,
        dto::LorebookProjectRevisionRequest {
            client_operation_id: format!("{key}-approve"),
            project_id: project.project_id,
            expected_revision: planned.revision,
        },
    )
    .await
    .expect("approved outline")
}

#[tokio::test(flavor = "multi_thread")]
async fn write_all_queues_every_entry_replays_and_limits_independent_claims() {
    let harness = harness(Reply::LorebookTools);
    let project = approved_project(&harness, "write-all").await;
    let request = dto::LorebookProjectJobRequest {
        client_operation_id: "write-all-action".into(),
        project_id: project.project_id.clone(),
        expected_revision: project.revision,
    };
    let all = lorebook_project_write_all(&harness.context, request.clone())
        .await
        .expect("all jobs");
    assert_eq!(all.job_ids.len(), 5);
    assert_eq!(
        lorebook_project_write_all(&harness.context, request)
            .await
            .expect("replay"),
        all
    );
    let waiting = project_get(&harness, &project.project_id).await;
    assert!(
        waiting
            .drafts
            .iter()
            .all(|draft| draft.status == dto::LorebookDraftStatus::Pending)
    );
    assert_eq!(waiting.active_job_ids.len(), 5);
    let mut claimed = Vec::new();
    let mut deferred = Vec::new();
    for id in &all.job_ids {
        let job = JobStore::get(
            harness.context.backend().database(),
            id.parse().expect("id"),
        )
        .expect("job")
        .expect("exists");
        match LorebookHandler
            .claim(&harness.context, &job, WorkerId::new())
            .await
            .expect("independent claim")
        {
            Some(work) => claimed.push((id.clone(), work)),
            None => deferred.push(id.clone()),
        }
    }
    assert_eq!(claimed.len(), 3);
    assert_eq!(deferred.len(), 2);
    assert!(
        JobStore::claim_next(
            harness.context.backend().database(),
            WorkerId::new(),
            TimestampMillis::now().expect("now"),
            std::time::Duration::from_secs(60),
            &lettuce_jobs::ResourceAvailability::all()
        )
        .expect("another worker")
        .is_none()
    );
    let writing = project_get(&harness, &project.project_id).await;
    assert_eq!(
        writing
            .drafts
            .iter()
            .filter(|draft| draft.status == dto::LorebookDraftStatus::Drafting)
            .count(),
        3
    );
    for (_, work) in claimed {
        work.run(harness.context.clone(), Arc::new(Progress))
            .await
            .expect("write");
    }
    for id in deferred {
        execute(&harness, &id).await;
    }
    let completed = project_get(&harness, &project.project_id).await;
    assert_eq!(completed.stage, dto::LorebookProjectStage::DraftsReady);
    assert!(completed.active_job_ids.is_empty());
    assert_eq!(harness.provider.requests.lock().expect("requests").len(), 6);
}

#[tokio::test(flavor = "multi_thread")]
async fn write_all_survives_reopen_without_an_attached_frontend() {
    let directory =
        std::env::temp_dir().join(format!("lorebook-write-restart-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).expect("directory");
    let path = directory.join("jobs.sqlite");
    let backend = Arc::new(
        crate::AppBackend::open(&path, TimestampMillis::now().expect("now")).expect("backend"),
    );
    let first = crate::api::tests::harness_over(
        backend,
        Reply::LorebookTools,
        Arc::new(lettuce_jobs::SystemClock),
        None,
        None,
        Arc::new(NoModels),
        Arc::new(crate::api::tests::NoImages),
    );
    let project = approved_project(&first, "reopen").await;
    let all = lorebook_project_write_all(
        &first.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "reopen-write".into(),
            project_id: project.project_id.clone(),
            expected_revision: project.revision,
        },
    )
    .await
    .expect("all jobs");
    execute(&first, &all.job_ids[0]).await;
    drop(first);
    let backend = Arc::new(
        crate::AppBackend::open(&path, TimestampMillis::now().expect("now"))
            .expect("reopened backend"),
    );
    let restarted = crate::api::tests::harness_over(
        backend,
        Reply::LorebookTools,
        Arc::new(lettuce_jobs::SystemClock),
        None,
        None,
        Arc::new(NoModels),
        Arc::new(crate::api::tests::NoImages),
    );
    let runner = Arc::new(JobRunner::new(
        restarted.context.clone(),
        JobHandlers::standard(),
    ));
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let running = Arc::clone(&runner);
    let task = tokio::spawn(async move {
        running
            .run(async {
                let _ = stopped.await;
            })
            .await;
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if project_get(&restarted, &project.project_id).await.stage
                == dto::LorebookProjectStage::DraftsReady
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("native worker completes without UI admission");
    stop.send(()).expect("shutdown");
    task.await.expect("worker");
    assert_eq!(
        restarted.provider.requests.lock().expect("requests").len(),
        4
    );
    drop(runner);
    drop(restarted);
    std::fs::remove_dir_all(directory).expect("cleanup");
}

#[tokio::test(flavor = "multi_thread")]
async fn stop_preserves_finished_entries_fences_claims_and_retries_only_unfinished_work() {
    let harness = harness(Reply::LorebookTools);
    let project = approved_project(&harness, "stop-writing").await;
    let all = lorebook_project_write_all(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "before-stop".into(),
            project_id: project.project_id.clone(),
            expected_revision: project.revision,
        },
    )
    .await
    .expect("all jobs");
    execute(&harness, &all.job_ids[0]).await;
    let current = project_get(&harness, &project.project_id).await;
    let written = current
        .drafts
        .iter()
        .find(|draft| draft.status == dto::LorebookDraftStatus::Drafted)
        .expect("written")
        .clone();
    let job = JobStore::get(
        harness.context.backend().database(),
        all.job_ids[1].parse().expect("id"),
    )
    .expect("job")
    .expect("exists");
    let claimed = LorebookHandler
        .claim(&harness.context, &job, WorkerId::new())
        .await
        .expect("claim")
        .expect("claimed");
    let request = dto::LorebookProjectRevisionRequest {
        client_operation_id: "stop-action".into(),
        project_id: project.project_id.clone(),
        expected_revision: current.revision,
    };
    let stopped = lorebook_project_stop(&harness.context, request.clone())
        .await
        .expect("stop");
    assert_eq!(
        lorebook_project_stop(&harness.context, request.clone())
            .await
            .expect("stop replay"),
        stopped
    );
    assert_eq!(
        stopped
            .drafts
            .iter()
            .find(|draft| draft.plan_id == written.plan_id),
        Some(&written)
    );
    claimed
        .run(harness.context.clone(), Arc::new(Progress))
        .await
        .expect("cancelled settlement");
    let after = project_get(&harness, &project.project_id).await;
    assert!(after.active_job_ids.is_empty());
    assert_eq!(
        after
            .drafts
            .iter()
            .filter(|draft| draft.status == dto::LorebookDraftStatus::Failed)
            .count(),
        4
    );
    let retried = lorebook_project_write_all(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "after-stop".into(),
            project_id: project.project_id.clone(),
            expected_revision: after.revision,
        },
    )
    .await
    .expect("retry unfinished");
    assert_eq!(retried.job_ids.len(), 4);
    let resumed_id = retried.job_ids[0].parse().expect("resumed id");
    let resumed_job = JobStore::get(harness.context.backend().database(), resumed_id)
        .expect("job")
        .expect("exists");
    let resumed = LorebookHandler
        .claim(&harness.context, &resumed_job, WorkerId::new())
        .await
        .expect("claim")
        .expect("resumed");
    let cancellation = resumed.cancellation();
    harness
        .context
        .jobs()
        .start_running(resumed_id, cancellation.clone());
    lorebook_project_stop(&harness.context, request)
        .await
        .expect("old stop replay");
    assert!(!cancellation.is_cancelled());
    resumed
        .run(harness.context.clone(), Arc::new(Progress))
        .await
        .expect("resumed write");
    harness.context.jobs().finish_running(resumed_id);
    for id in retried.job_ids.into_iter().skip(1) {
        execute(&harness, &id).await;
    }
    let done = project_get(&harness, &project.project_id).await;
    assert_eq!(done.stage, dto::LorebookProjectStage::DraftsReady);
    assert_eq!(
        done.drafts
            .iter()
            .find(|draft| draft.plan_id == written.plan_id),
        Some(&written)
    );
}

#[tokio::test]
async fn stopping_a_queued_plan_is_visible_and_can_be_retried() {
    let harness = harness(Reply::LorebookTools);
    let project = staged_project(&harness, "stop-plan").await;
    let plan = lorebook_project_plan(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "queued-plan".into(),
            project_id: project.project_id.clone(),
            expected_revision: project.revision,
        },
    )
    .await
    .expect("plan");
    let planning = project_get(&harness, &project.project_id).await;
    let stopped = lorebook_project_stop(
        &harness.context,
        dto::LorebookProjectRevisionRequest {
            client_operation_id: "stop-plan-action".into(),
            project_id: project.project_id.clone(),
            expected_revision: planning.revision,
        },
    )
    .await
    .expect("stop");
    assert_eq!(stopped.stage, dto::LorebookProjectStage::PlanFailed);
    assert!(stopped.active_job_ids.is_empty());
    let retried = lorebook_project_plan(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "retry-stopped-plan".into(),
            project_id: project.project_id.clone(),
            expected_revision: stopped.revision,
        },
    )
    .await
    .expect("retry stopped plan");
    assert_ne!(retried.job_id, plan.job_id);
    execute(&harness, &retried.job_id).await;
    assert_eq!(
        project_get(&harness, &project.project_id).await.stage,
        dto::LorebookProjectStage::AwaitingOutlineApproval
    );
}

#[tokio::test]
async fn write_all_does_not_automatically_readmit_newly_failed_entries() {
    let harness = harness(Reply::LorebookTools);
    let project = approved_project(&harness, "write-failures").await;
    *harness.provider.response_hook.lock().expect("hook") = Some(Arc::new(|_, outcome| {
        for candidate in &mut outcome.candidates {
            candidate.tool_calls.clear();
        }
    }));
    let all = lorebook_project_write_all(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "failed-entries-action".into(),
            project_id: project.project_id.clone(),
            expected_revision: project.revision,
        },
    )
    .await
    .expect("write jobs");
    for id in &all.job_ids {
        execute(&harness, id).await;
    }
    let failed = project_get(&harness, &project.project_id).await;
    assert_eq!(failed.stage, dto::LorebookProjectStage::DraftsReady);
    assert_eq!(
        failed
            .drafts
            .iter()
            .filter(|draft| draft.status == dto::LorebookDraftStatus::Failed)
            .count(),
        5
    );
    assert!(failed.active_job_ids.is_empty());
    assert_eq!(harness.provider.requests.lock().expect("requests").len(), 6);
    let runner = JobRunner::new(harness.context.clone(), JobHandlers::standard());
    assert!(!runner.run_once().await.expect("no automatic retry"));
}

#[tokio::test(flavor = "multi_thread")]
async fn finished_drafts_can_be_edited_and_approved_while_other_writers_are_queued() {
    let harness = harness(Reply::LorebookTools);
    let project = approved_project(&harness, "edit-queued").await;
    let all = lorebook_project_write_all(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "edit-queued-write".into(),
            project_id: project.project_id.clone(),
            expected_revision: project.revision,
        },
    )
    .await
    .expect("write jobs");
    execute(&harness, &all.job_ids[0]).await;
    let current = project_get(&harness, &project.project_id).await;
    let written = current
        .drafts
        .iter()
        .find(|draft| draft.status == dto::LorebookDraftStatus::Drafted)
        .expect("written");
    let edited = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        lorebook_project_draft_update(
            &harness.context,
            dto::LorebookProjectDraftUpdateRequest {
                client_operation_id: "edit-queued-content".into(),
                project_id: project.project_id.clone(),
                expected_revision: current.revision,
                plan_id: written.plan_id.clone(),
                title: "Edited while waiting".into(),
                keywords: written.keywords.clone(),
                content: written.content.clone(),
                always_active: written.always_active,
            },
        ),
    )
    .await
    .expect("transaction-scoped reads must not deadlock")
    .expect("edited");
    assert_eq!(
        edited
            .drafts
            .iter()
            .filter(|draft| draft.status == dto::LorebookDraftStatus::Pending)
            .count(),
        4
    );
    let approved = lorebook_project_draft_set_approved(
        &harness.context,
        dto::LorebookProjectDraftApprovalRequest {
            client_operation_id: "approve-queued-content".into(),
            project_id: project.project_id,
            expected_revision: edited.revision,
            plan_id: written.plan_id.clone(),
            approved: true,
        },
    )
    .await
    .expect("approved while waiting");
    let entry = approved
        .drafts
        .iter()
        .find(|draft| draft.plan_id == written.plan_id)
        .expect("entry");
    assert_eq!(entry.title, "Edited while waiting");
    assert_eq!(entry.status, dto::LorebookDraftStatus::Approved);
}

async fn execute(harness: &crate::api::tests::Harness, id: &str) {
    let db = harness.context.backend().database();
    let job = JobStore::get(db, id.parse().expect("job id"))
        .expect("job")
        .expect("exists");
    let work = LorebookHandler
        .claim(&harness.context, &job, WorkerId::new())
        .await
        .expect("claim")
        .expect("work");
    assert!(
        LorebookHandler
            .claim(&harness.context, &job, WorkerId::new())
            .await
            .expect("another worker")
            .is_none()
    );
    work.run(harness.context.clone(), Arc::new(Progress))
        .await
        .expect("execute");
}

async fn project_get(harness: &crate::api::tests::Harness, id: &str) -> dto::LorebookProjectView {
    lorebook_project_get(
        &harness.context,
        dto::LorebookProjectGetRequest {
            project_id: id.into(),
        },
    )
    .await
    .expect("project")
}

#[tokio::test(flavor = "multi_thread")]
async fn staged_api_batches_replay_refuse_overlap_and_commit_unapproved_drafts() {
    let harness = harness(Reply::LorebookTools);
    let created = staged_project(&harness, "staged-api").await;
    let plan_request = dto::LorebookProjectJobRequest {
        client_operation_id: "plan-staged".into(),
        project_id: created.project_id.clone(),
        expected_revision: created.revision,
    };
    let plan = lorebook_project_plan(&harness.context, plan_request.clone())
        .await
        .expect("plan");
    assert_eq!(
        lorebook_project_plan(&harness.context, plan_request)
            .await
            .expect("plan replay"),
        plan
    );
    execute(&harness, &plan.job_id).await;
    let planned = project_get(&harness, &created.project_id).await;
    assert_eq!(
        planned.stage,
        dto::LorebookProjectStage::AwaitingOutlineApproval
    );
    let approved = lorebook_project_outline_approve(
        &harness.context,
        dto::LorebookProjectRevisionRequest {
            client_operation_id: "outline-approve".into(),
            project_id: created.project_id.clone(),
            expected_revision: planned.revision,
        },
    )
    .await
    .expect("approve");
    let batch_request = dto::LorebookProjectJobRequest {
        client_operation_id: "first-batch".into(),
        project_id: created.project_id.clone(),
        expected_revision: approved.revision,
    };
    let first = lorebook_project_draft_next(&harness.context, batch_request.clone())
        .await
        .expect("batch");
    assert_eq!(first.job_ids.len(), 3);
    assert_eq!(
        lorebook_project_draft_next(&harness.context, batch_request.clone())
            .await
            .expect("batch replay"),
        first
    );
    let overlap = lorebook_project_draft_next(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "overlap".into(),
            ..batch_request.clone()
        },
    )
    .await
    .expect_err("overlap");
    assert_eq!(overlap.code, ApiErrorCode::Busy);
    assert!(matches!(
        overlap.details,
        Some(dto::ApiErrorDetails::LorebookBatchRunning { .. })
    ));
    assert_eq!(
        lorebook_project_draft_next(
            &harness.context,
            dto::LorebookProjectJobRequest {
                expected_revision: approved.revision + 1,
                ..batch_request
            }
        )
        .await
        .expect_err("changed digest")
        .code,
        ApiErrorCode::Conflict
    );
    for id in &first.job_ids {
        execute(&harness, id).await;
    }
    let partial = project_get(&harness, &created.project_id).await;
    let second = lorebook_project_draft_next(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "second-batch".into(),
            project_id: created.project_id.clone(),
            expected_revision: partial.revision,
        },
    )
    .await
    .expect("second batch");
    assert_eq!(second.job_ids.len(), 2);
    for id in &second.job_ids {
        execute(&harness, id).await;
    }
    let ready = project_get(&harness, &created.project_id).await;
    assert_eq!(ready.stage, dto::LorebookProjectStage::DraftsReady);
    let coherence_request = dto::LorebookProjectJobRequest {
        client_operation_id: "coherence".into(),
        project_id: created.project_id.clone(),
        expected_revision: ready.revision,
    };
    let coherence = lorebook_project_coherence(&harness.context, coherence_request.clone())
        .await
        .expect("coherence");
    execute(&harness, &coherence.job_id).await;
    assert_eq!(
        lorebook_project_coherence(&harness.context, coherence_request)
            .await
            .expect("coherence replay"),
        coherence
    );
    let review = project_get(&harness, &created.project_id).await;
    assert_eq!(review.stage, dto::LorebookProjectStage::CoherenceReview);
    let draft = &review.drafts[0];
    let edited = lorebook_project_draft_update(
        &harness.context,
        dto::LorebookProjectDraftUpdateRequest {
            client_operation_id: "review-edit".into(),
            project_id: created.project_id.clone(),
            expected_revision: review.revision,
            plan_id: draft.plan_id.clone(),
            title: "Edited".into(),
            keywords: draft.keywords.clone(),
            content: "Edited content".into(),
            always_active: false,
        },
    )
    .await
    .expect("edit review");
    let refined = lorebook_project_refine(
        &harness.context,
        dto::LorebookProjectRefineRequest {
            client_operation_id: "refine".into(),
            project_id: created.project_id.clone(),
            expected_revision: edited.revision,
            plan_id: draft.plan_id.clone(),
            feedback: "More detail".into(),
        },
    )
    .await
    .expect("refine review");
    assert!(
        project_get(&harness, &created.project_id)
            .await
            .active_job_ids
            .contains(&refined.job_id)
    );
    execute(&harness, &refined.job_id).await;
    let review = project_get(&harness, &created.project_id).await;
    assert_eq!(review.stage, dto::LorebookProjectStage::CoherenceReview);
    let approved = lorebook_project_draft_set_approved(
        &harness.context,
        dto::LorebookProjectDraftApprovalRequest {
            client_operation_id: "review-approve".into(),
            project_id: created.project_id.clone(),
            expected_revision: review.revision,
            plan_id: draft.plan_id.clone(),
            approved: true,
        },
    )
    .await
    .expect("approval review");
    let commit_request = dto::LorebookProjectCommitRequest {
        client_operation_id: "commit".into(),
        project_id: created.project_id.clone(),
        expected_revision: approved.revision,
        target: dto::LorebookProjectCommitTarget::NewLorebook {
            name: Some("Coast".into()),
        },
    };
    let committed = lorebook_project_commit(&harness.context, commit_request.clone())
        .await
        .expect("commit");
    assert_eq!(committed.entry_ids.len(), 5);
    let committed_project = project_get(&harness, &created.project_id).await;
    assert_eq!(
        committed_project.stage,
        dto::LorebookProjectStage::Committed
    );
    assert_eq!(committed_project.coherence_changes.len(), 1);
    let committed_book = lorebook_get(
        &harness.context,
        dto::LorebookGetRequest {
            lorebook_id: committed.lorebook_id.clone(),
        },
    )
    .await
    .expect("committed book");
    lorebook_delete(
        &harness.context,
        dto::LorebookRevisionRequest {
            client_operation_id: "delete-committed".into(),
            lorebook_id: committed.lorebook_id.clone(),
            expected_revision: committed_book.lorebook.revision,
        },
    )
    .await
    .expect("delete source");
    assert_eq!(
        lorebook_project_commit(&harness.context, commit_request)
            .await
            .expect("original commit replay"),
        committed
    );
    assert!(
        project_get(&harness, &created.project_id)
            .await
            .commit
            .expect("history")
            .lorebook_deleted
    );
    let requests = harness.provider.requests.lock().expect("requests");
    let operations = requests
        .iter()
        .filter_map(|request| {
            request
                .context
                .attributions
                .usage_snapshot
                .as_ref()?
                .operation_kind
                .as_deref()
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        operations,
        std::collections::BTreeSet::from([
            "lorebook_plan",
            "lorebook_entry_write",
            "lorebook_entry_refine",
            "lorebook_coherence",
        ])
    );
}

async fn prepare_stage(harness: &crate::api::tests::Harness, stage: &str) -> (String, String) {
    let project = staged_project(harness, stage).await;
    let plan = lorebook_project_plan(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "plan-stage".into(),
            project_id: project.project_id.clone(),
            expected_revision: project.revision,
        },
    )
    .await
    .expect("plan");
    if stage == "planner" {
        return (project.project_id, plan.job_id);
    }
    execute(harness, &plan.job_id).await;
    let project = project_get(harness, &project.project_id).await;
    let approved = lorebook_project_outline_approve(
        &harness.context,
        dto::LorebookProjectRevisionRequest {
            client_operation_id: "approve-stage".into(),
            project_id: project.project_id.clone(),
            expected_revision: project.revision,
        },
    )
    .await
    .expect("approve");
    let first = lorebook_project_draft_next(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "batch-stage".into(),
            project_id: project.project_id.clone(),
            expected_revision: approved.revision,
        },
    )
    .await
    .expect("batch");
    if stage == "writer" {
        return (project.project_id, first.job_ids[0].clone());
    }
    for id in &first.job_ids {
        execute(harness, id).await;
    }
    let project = project_get(harness, &project.project_id).await;
    let next = lorebook_project_draft_next(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "batch-stage-next".into(),
            project_id: project.project_id.clone(),
            expected_revision: project.revision,
        },
    )
    .await
    .expect("next");
    for id in &next.job_ids {
        execute(harness, id).await;
    }
    let project = project_get(harness, &project.project_id).await;
    let coherence = lorebook_project_coherence(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "coherence-stage".into(),
            project_id: project.project_id.clone(),
            expected_revision: project.revision,
        },
    )
    .await
    .expect("coherence");
    (project.project_id, coherence.job_id)
}

async fn shutdown_restart_stage(stage: &str, tool: &'static str) {
    let directory = std::env::temp_dir().join(format!("slice6-jobs-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).expect("directory");
    let path = directory.join("jobs.sqlite");
    let backend = Arc::new(
        crate::AppBackend::open(&path, TimestampMillis::now().expect("now")).expect("backend"),
    );
    let harness = crate::api::tests::harness_over(
        backend,
        Reply::LorebookToolsUntil(tool),
        Arc::new(lettuce_jobs::SystemClock),
        None,
        None,
        Arc::new(NoModels),
        Arc::new(crate::api::tests::NoImages),
    );
    let (project_id, job_id) = prepare_stage(&harness, stage).await;
    let db = harness.context.backend().database();
    let job = JobStore::get(db, job_id.parse().expect("job id"))
        .expect("job")
        .expect("exists");
    let work = LorebookHandler
        .claim(&harness.context, &job, WorkerId::new())
        .await
        .expect("claim")
        .expect("work");
    let link =
        crate::api::worker::link_to_shutdown(harness.context.shutdown_token(), work.cancellation());
    let context = harness.context.clone();
    let running = tokio::spawn(async move { work.run(context, Arc::new(Progress)).await });
    loop {
        let notified = harness.provider.entered.notified();
        let entered = harness
            .provider
            .requests
            .lock()
            .expect("requests")
            .last()
            .is_some_and(|request| {
                request.tools.as_ref().is_some_and(|tools| {
                    tools
                        .definitions
                        .iter()
                        .any(|definition| definition.name == tool)
                })
            });
        if entered {
            break;
        }
        notified.await;
    }
    harness.context.begin_shutdown();
    tokio::time::timeout(std::time::Duration::from_secs(5), running)
        .await
        .expect("shutdown completes")
        .expect("worker task")
        .expect("retry scheduled");
    assert_eq!(
        JobStore::get(db, job.id)
            .expect("job")
            .expect("exists")
            .state,
        lettuce_jobs::JobState::Queued
    );
    drop(link);
    drop(harness);
    let backend = Arc::new(
        crate::AppBackend::open(&path, TimestampMillis::now().expect("now")).expect("reopen"),
    );
    let restarted = crate::api::tests::harness_over(
        backend,
        Reply::LorebookTools,
        Arc::new(lettuce_jobs::SystemClock),
        None,
        None,
        Arc::new(NoModels),
        Arc::new(crate::api::tests::NoImages),
    );
    execute(&restarted, &job_id).await;
    assert_eq!(
        JobStore::get(restarted.context.backend().database(), job.id)
            .expect("job")
            .expect("exists")
            .state,
        lettuce_jobs::JobState::Succeeded
    );
    let project = project_get(&restarted, &project_id).await;
    assert_eq!(
        project.stage,
        match stage {
            "planner" => dto::LorebookProjectStage::AwaitingOutlineApproval,
            "writer" => dto::LorebookProjectStage::Drafting,
            _ => dto::LorebookProjectStage::CoherenceReview,
        }
    );
    assert_eq!(
        restarted.provider.requests.lock().expect("requests").len(),
        1
    );
    drop(restarted);
    std::fs::remove_dir_all(directory).expect("cleanup");
}

#[tokio::test(flavor = "multi_thread")]
async fn shutdown_mid_planner_then_restart() {
    shutdown_restart_stage("planner", "propose_lorebook_outline").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn shutdown_mid_writer_then_restart() {
    shutdown_restart_stage("writer", "write_lorebook_entry").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn shutdown_mid_coherence_then_restart() {
    shutdown_restart_stage("coherence", "propose_coherence_changes").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_prompt_during_keyword_inference_preserves_restart_and_history() {
    let directory = std::env::temp_dir().join(format!("slice6-draft-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).expect("directory");
    let path = directory.join("jobs.sqlite");
    let backend = Arc::new(
        crate::AppBackend::open(&path, TimestampMillis::now().expect("now")).expect("backend"),
    );
    let harness = crate::api::tests::harness_over(
        backend,
        Reply::UntilCancelled,
        Arc::new(lettuce_jobs::SystemClock),
        None,
        None,
        Arc::new(NoModels),
        Arc::new(crate::api::tests::NoImages),
    );
    let book = book(&harness).await;
    let prompt = custom_prompt(&harness);
    let db = harness.context.backend().database();
    let mut stored = GlobalSettingsStore::load(db).expect("settings");
    stored.settings.lorebook_entry_generator.keyword_prompt_id = Some(prompt.id);
    GlobalSettingsStore::save(
        db,
        stored.settings,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("selection");
    let request = request(&book);
    let admitted = lorebook_keywords_draft(&harness.context, request.clone())
        .await
        .expect("admit");
    let job = JobStore::get(db, admitted.job_id.parse().expect("id"))
        .expect("job")
        .expect("exists");
    let work = LorebookHandler
        .claim(&harness.context, &job, WorkerId::new())
        .await
        .expect("claim")
        .expect("work");
    let link =
        crate::api::worker::link_to_shutdown(harness.context.shutdown_token(), work.cancellation());
    let context = harness.context.clone();
    let running = tokio::spawn(async move { work.run(context, Arc::new(Progress)).await });
    harness.provider.entered.notified().await;
    db.delete_prompt(
        prompt.id,
        prompt.revision,
        TimestampMillis::now().expect("now"),
    )
    .expect("delete during inference");
    let result = job_get(
        &harness.context,
        dto::JobGetRequest {
            job_id: admitted.job_id.clone(),
        },
    )
    .await
    .expect("running history");
    assert_eq!(
        result.subject_detail,
        Some(dto::JobSubjectDetail::LorebookDraft {
            lorebook: None,
            prompt: dto::HistoricalSourceView {
                id: prompt.id.to_string(),
                name: "Keyword source".into(),
                deleted: true
            }
        })
    );
    harness.context.begin_shutdown();
    tokio::time::timeout(std::time::Duration::from_secs(5), running)
        .await
        .expect("shutdown")
        .expect("task")
        .expect("retry");
    drop(link);
    drop(harness);
    let backend = Arc::new(
        crate::AppBackend::open(&path, TimestampMillis::now().expect("now")).expect("reopen"),
    );
    let restarted = crate::api::tests::harness_over(
        backend,
        Reply::Text(r#"{"tool":"write_lorebook_keywords","arguments":{"keywords":["Coast"]}}"#),
        Arc::new(lettuce_jobs::SystemClock),
        None,
        None,
        Arc::new(NoModels),
        Arc::new(crate::api::tests::NoImages),
    );
    assert_eq!(
        lorebook_keywords_draft(&restarted.context, request)
            .await
            .expect("replay after deletion"),
        admitted
    );
    execute(&restarted, &admitted.job_id).await;
    assert_eq!(
        job_get(
            &restarted.context,
            dto::JobGetRequest {
                job_id: admitted.job_id
            }
        )
        .await
        .expect("result")
        .result,
        Some(dto::JobResultDto::LorebookKeywords {
            keywords: vec!["Coast".into()]
        })
    );
    drop(restarted);
    std::fs::remove_dir_all(directory).expect("cleanup");
}

#[tokio::test]
async fn entry_force_uses_catalog_and_save_is_atomic_and_replayable() {
    let harness = harness(Reply::LorebookTools);
    let book = book(&harness).await;
    let conversation = crate::api::tests::launch(&harness, "draft-source").await;
    let open = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: conversation.clone(),
        },
    )
    .await
    .expect("open");
    let message = conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: conversation.clone(),
            text: "A new coastal district has opened.".into(),
            expected_revision: open.revision,
            client_operation_id: "source-message".into(),
        },
    )
    .await
    .expect("message");
    let request = dto::LorebookEntryDraftRequest {
        client_operation_id: "entry-force".into(),
        conversation_id: conversation,
        lorebook_id: book.lorebook.id.clone(),
        source: dto::LorebookEntryDraftSource::Messages,
        message_ids: vec![message.message.id],
        memory_ids: vec![],
        use_summary: false,
        direction: Some("Describe the district".into()),
        force: true,
    };
    let admitted = lorebook_entry_draft(&harness.context, request.clone())
        .await
        .expect("draft");
    assert_eq!(
        lorebook_entry_draft(&harness.context, request)
            .await
            .expect("replay"),
        admitted
    );
    execute(&harness, &admitted.job_id).await;
    {
        let requests = harness.provider.requests.lock().expect("requests");
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0]
                .context
                .attributions
                .usage_snapshot
                .as_ref()
                .expect("usage attribution")
                .operation_kind
                .as_deref(),
            Some("lorebook_entry_generator")
        );
        assert!(
            !requests[0]
                .tools
                .as_ref()
                .expect("tools")
                .definitions
                .iter()
                .any(|tool| tool.name == "no_entry")
        );
        assert!(format!("{:?}", requests[0].context.messages).contains("[FORCE MODE]"));
    }

    let result = job_get(
        &harness.context,
        dto::JobGetRequest {
            job_id: admitted.job_id,
        },
    )
    .await
    .expect("result");
    let Some(dto::JobResultDto::LorebookEntryDraft { draft }) = result.result else {
        panic!("draft missing");
    };
    let save = dto::LorebookEntriesMutateRequest {
        client_operation_id: "save-draft".into(),
        lorebook_id: book.lorebook.id.clone(),
        expected_revision: book.lorebook.revision,
        mutations: vec![dto::LorebookEntryMutationInput::Add {
            entry: super::tests::entry(&draft.title, &draft.content, Some(&draft.keywords[0])),
            index: None,
        }],
    };
    let saved = lorebook_entries_mutate(&harness.context, save.clone())
        .await
        .expect("save");
    assert_eq!(saved.entries.len(), 1);
    assert_eq!(
        lorebook_entries_mutate(&harness.context, save)
            .await
            .expect("save replay"),
        saved
    );
}

#[tokio::test]
async fn planner_failure_is_typed_and_a_new_key_retries() {
    let harness = harness(Reply::Text("invalid planner response"));
    let created = staged_project(&harness, "failed-planner").await;
    let request = dto::LorebookProjectJobRequest {
        client_operation_id: "failed-plan".into(),
        project_id: created.project_id.clone(),
        expected_revision: created.revision,
    };
    let plan = lorebook_project_plan(&harness.context, request.clone())
        .await
        .expect("plan");
    execute(&harness, &plan.job_id).await;
    let failed = project_get(&harness, &created.project_id).await;
    assert_eq!(failed.stage, dto::LorebookProjectStage::PlanFailed);
    assert!(failed.plan_failure.is_some());
    assert_eq!(
        lorebook_project_plan(&harness.context, request)
            .await
            .expect("replay"),
        plan
    );
    let retry = lorebook_project_plan(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "retry-plan".into(),
            project_id: failed.project_id,
            expected_revision: failed.revision,
        },
    )
    .await
    .expect("retry");
    assert_ne!(retry.job_id, plan.job_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_lorebook_during_entry_inference_keeps_frozen_source_history() {
    let harness = harness(Reply::UntilCancelled);
    let book = book(&harness).await;
    let conversation = crate::api::tests::launch(&harness, "entry-delete-source").await;
    let open = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: conversation.clone(),
        },
    )
    .await
    .expect("open");
    let message = conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: conversation.clone(),
            text: "The district is on the coast.".into(),
            expected_revision: open.revision,
            client_operation_id: "entry-delete-message".into(),
        },
    )
    .await
    .expect("message");
    let request = dto::LorebookEntryDraftRequest {
        client_operation_id: "entry-delete-draft".into(),
        conversation_id: conversation,
        lorebook_id: book.lorebook.id.clone(),
        source: dto::LorebookEntryDraftSource::Messages,
        message_ids: vec![message.message.id],
        memory_ids: vec![],
        use_summary: false,
        direction: None,
        force: false,
    };
    let admitted = lorebook_entry_draft(&harness.context, request.clone())
        .await
        .expect("draft");
    let db = harness.context.backend().database();
    let job = JobStore::get(db, admitted.job_id.parse().expect("id"))
        .expect("job")
        .expect("exists");
    let work = LorebookHandler
        .claim(&harness.context, &job, WorkerId::new())
        .await
        .expect("claim")
        .expect("work");
    let link =
        crate::api::worker::link_to_shutdown(harness.context.shutdown_token(), work.cancellation());
    let context = harness.context.clone();
    let running = tokio::spawn(async move { work.run(context, Arc::new(Progress)).await });
    harness.provider.entered.notified().await;
    lorebook_delete(
        &harness.context,
        dto::LorebookRevisionRequest {
            client_operation_id: "delete-running-source".into(),
            lorebook_id: book.lorebook.id.clone(),
            expected_revision: book.lorebook.revision,
        },
    )
    .await
    .expect("delete during inference");
    assert_eq!(
        lorebook_entry_draft(&harness.context, request)
            .await
            .expect("draft replay"),
        admitted
    );
    let result = job_get(
        &harness.context,
        dto::JobGetRequest {
            job_id: admitted.job_id,
        },
    )
    .await
    .expect("history");
    let Some(dto::JobSubjectDetail::LorebookDraft {
        lorebook: Some(source),
        ..
    }) = result.subject_detail
    else {
        panic!("missing history source");
    };
    assert_eq!(
        source,
        dto::HistoricalSourceView {
            id: book.lorebook.id,
            name: "Book".into(),
            deleted: true
        }
    );
    harness.context.begin_shutdown();
    tokio::time::timeout(std::time::Duration::from_secs(5), running)
        .await
        .expect("shutdown")
        .expect("task")
        .expect("retry");
    assert_eq!(
        JobStore::get(db, job.id)
            .expect("job")
            .expect("exists")
            .state,
        lettuce_jobs::JobState::Queued
    );
    drop(link);
}

#[tokio::test]
async fn configured_archived_entry_prompt_is_refused_before_job_admission() {
    let harness = harness(Reply::LorebookTools);
    let book = book(&harness).await;
    let chat = crate::api::tests::launch(&harness, "archived-entry").await;
    let prompt = custom_prompt_for(&harness, crate::BuiltInPromptId::LorebookEntryWriter);
    let db = harness.context.backend().database();
    let mut stored = GlobalSettingsStore::load(db).expect("settings");
    stored.settings.lorebook_entry_generator.entry_prompt_id = Some(prompt.id);
    GlobalSettingsStore::save(
        db,
        stored.settings,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("selection");
    PromptRepository::archive(
        db,
        prompt.id,
        prompt.revision,
        TimestampMillis::now().expect("now"),
    )
    .expect("archive");
    let error = lorebook_entry_draft(
        &harness.context,
        dto::LorebookEntryDraftRequest {
            client_operation_id: "archived-entry-job".into(),
            conversation_id: chat,
            lorebook_id: book.lorebook.id,
            source: dto::LorebookEntryDraftSource::Messages,
            message_ids: vec![],
            memory_ids: vec![],
            use_summary: false,
            direction: None,
            force: false,
        },
    )
    .await
    .expect_err("configured archived prompt");
    assert_eq!(error.code, ApiErrorCode::Unavailable);
    assert_eq!(
        error.details,
        Some(dto::ApiErrorDetails::ConfiguredPromptUnavailable {
            prompt_id: prompt.id.to_string(),
            reason: dto::ConfiguredPromptProblem::Archived
        })
    );
    assert!(
        harness
            .provider
            .requests
            .lock()
            .expect("requests")
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_keyword_admissions_replay_one_frozen_job() {
    let harness = harness(Reply::LorebookTools);
    let book = book(&harness).await;
    let request = request(&book);
    let mut callers = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let context = harness.context.clone();
        let request = request.clone();
        callers.spawn(async move { lorebook_keywords_draft(&context, request).await });
    }
    let expected = callers
        .join_next()
        .await
        .expect("caller")
        .expect("task")
        .expect("admission");
    while let Some(result) = callers.join_next().await {
        assert_eq!(result.expect("task").expect("replay"), expected);
    }
}

async fn run_standard_runner(harness: &crate::api::tests::Harness) {
    let runner = JobRunner::new(harness.context.clone(), JobHandlers::standard());
    while runner.run_once().await.expect("runner") {}
    runner.wait_idle().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn standard_runner_executes_entry_and_staged_planner_jobs() {
    let harness = harness(Reply::LorebookTools);
    let book = book(&harness).await;
    let conversation = crate::api::tests::launch(&harness, "runner-source").await;
    let open = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: conversation.clone(),
        },
    )
    .await
    .expect("open");
    let message = conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: conversation.clone(),
            text: "A new coastal district has opened.".into(),
            expected_revision: open.revision,
            client_operation_id: "runner-message".into(),
        },
    )
    .await
    .expect("message");
    let entry = lorebook_entry_draft(
        &harness.context,
        dto::LorebookEntryDraftRequest {
            client_operation_id: "runner-entry".into(),
            conversation_id: conversation,
            lorebook_id: book.lorebook.id.clone(),
            source: dto::LorebookEntryDraftSource::Messages,
            message_ids: vec![message.message.id],
            memory_ids: vec![],
            use_summary: false,
            direction: None,
            force: true,
        },
    )
    .await
    .expect("entry draft");
    let project = staged_project(&harness, "runner-project").await;
    let plan = lorebook_project_plan(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "runner-plan".into(),
            project_id: project.project_id.clone(),
            expected_revision: project.revision,
        },
    )
    .await
    .expect("plan");
    run_standard_runner(&harness).await;
    let db = harness.context.backend().database();
    for id in [&entry.job_id, &plan.job_id] {
        assert_eq!(
            JobStore::get(db, id.parse().expect("job id"))
                .expect("job")
                .expect("exists")
                .state,
            lettuce_jobs::JobState::Succeeded
        );
    }
    assert_eq!(
        project_get(&harness, &project.project_id).await.stage,
        dto::LorebookProjectStage::AwaitingOutlineApproval
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn planner_retry_job_runs_and_the_project_leaves_planning() {
    let directory = std::env::temp_dir().join(format!("slice6-retry-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).expect("directory");
    let path = directory.join("retry.sqlite");
    let backend = Arc::new(
        crate::AppBackend::open(&path, TimestampMillis::now().expect("now")).expect("backend"),
    );
    let failing = crate::api::tests::harness_over(
        Arc::clone(&backend),
        Reply::Text("invalid planner response"),
        Arc::new(lettuce_jobs::SystemClock),
        None,
        None,
        Arc::new(NoModels),
        Arc::new(crate::api::tests::NoImages),
    );
    let created = staged_project(&failing, "retry-project").await;
    let plan = lorebook_project_plan(
        &failing.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "first-plan".into(),
            project_id: created.project_id.clone(),
            expected_revision: created.revision,
        },
    )
    .await
    .expect("plan");
    run_standard_runner(&failing).await;
    let failed = project_get(&failing, &created.project_id).await;
    assert_eq!(failed.stage, dto::LorebookProjectStage::PlanFailed);
    drop(failing);
    let working = crate::api::tests::harness_over(
        backend,
        Reply::LorebookTools,
        Arc::new(lettuce_jobs::SystemClock),
        None,
        None,
        Arc::new(NoModels),
        Arc::new(crate::api::tests::NoImages),
    );
    let retry = lorebook_project_plan(
        &working.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "retry-plan".into(),
            project_id: failed.project_id,
            expected_revision: failed.revision,
        },
    )
    .await
    .expect("retry");
    assert_ne!(retry.job_id, plan.job_id);
    run_standard_runner(&working).await;
    let db = working.context.backend().database();
    assert_eq!(
        JobStore::get(db, retry.job_id.parse().expect("job id"))
            .expect("job")
            .expect("exists")
            .state,
        lettuce_jobs::JobState::Succeeded
    );
    assert_eq!(
        project_get(&working, &created.project_id).await.stage,
        dto::LorebookProjectStage::AwaitingOutlineApproval
    );
    let view = job_get(
        &working.context,
        dto::JobGetRequest {
            job_id: retry.job_id,
        },
    )
    .await
    .expect("job view");
    assert!(matches!(
        view.subject_detail,
        Some(dto::JobSubjectDetail::LorebookProject { .. })
    ));
    assert!(matches!(
        view.result,
        Some(dto::JobResultDto::LorebookProject { .. })
    ));
    drop(working);
    std::fs::remove_dir_all(directory).expect("cleanup");
}

#[tokio::test]
async fn keyword_draft_uses_the_first_text_model_not_the_default() {
    let harness = harness(Reply::Text("ignored"));
    let book = book(&harness).await;
    let db = harness.context.backend().database();
    crate::launch::tests::seed_model(db, lettuce_models::ProviderProtocol::Ollama, "ollama");
    let models = lettuce_models::ModelCatalog::model_profiles(db).expect("models");
    assert_eq!(models.len(), 2);
    crate::launch::tests::set_application_default_model(db, models[1].id);
    let admitted = lorebook_keywords_draft(&harness.context, request(&book))
        .await
        .expect("admit");
    let job = JobStore::get(db, admitted.job_id.parse().expect("id"))
        .expect("job")
        .expect("exists");
    let work = LorebookHandler
        .claim(&harness.context, &job, WorkerId::new())
        .await
        .expect("claim")
        .expect("claimed");
    work.run(harness.context.clone(), Arc::new(Progress))
        .await
        .expect("run");
    let requests = harness.provider.requests.lock().expect("requests");
    assert!(!requests.is_empty());
    for request in requests.iter() {
        assert_eq!(request.profile.chat_profile.model_profile_id, models[0].id);
        assert_eq!(
            request
                .context
                .attributions
                .usage_snapshot
                .as_ref()
                .expect("usage attribution")
                .operation_kind
                .as_deref(),
            Some("lorebook_keywords")
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn commit_cancels_the_projects_running_coherence_job() {
    let harness = harness(Reply::LorebookTools);
    let (project_id, job_id) = prepare_stage(&harness, "coherence").await;
    let db = harness.context.backend().database();
    let job = JobStore::get(db, job_id.parse().expect("job id"))
        .expect("job")
        .expect("exists");
    let work = LorebookHandler
        .claim(&harness.context, &job, WorkerId::new())
        .await
        .expect("claim")
        .expect("work");
    let project = project_get(&harness, &project_id).await;
    lorebook_project_commit(
        &harness.context,
        dto::LorebookProjectCommitRequest {
            client_operation_id: "commit-running".into(),
            project_id: project_id.clone(),
            expected_revision: project.revision,
            target: dto::LorebookProjectCommitTarget::NewLorebook { name: None },
        },
    )
    .await
    .expect("commit");
    let after = JobStore::get(db, job.id).expect("job").expect("exists");
    assert!(after.cancellation.requested);
    assert!(
        harness
            .events
            .events()
            .contains(&dto::ApiEvent::LorebooksChanged)
    );
    drop(work);
}

#[tokio::test]
async fn staged_plan_and_entry_draft_share_the_first_text_model_fallback() {
    let harness = harness(Reply::LorebookTools);
    let db = harness.context.backend().database();
    let text_id = lettuce_models::ModelCatalog::model_profiles(db).expect("models")[0].id;
    let image =
        crate::launch::tests::seed_model(db, lettuce_models::ProviderProtocol::Ollama, "ollama");
    let mut model = lettuce_models::ModelProfileRepository::get(db, image)
        .expect("model")
        .expect("exists");
    let revision = model.revision;
    model.config.capabilities.output_modalities.text =
        lettuce_models::CapabilityStatus::Unsupported;
    lettuce_models::ModelProfileRepository::upsert(db, model, Some(revision)).expect("image");
    crate::launch::tests::set_application_default_model(db, image);
    let project = staged_project(&harness, "shared-fallback").await;
    let plan = lorebook_project_plan(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "shared-plan".into(),
            project_id: project.project_id.clone(),
            expected_revision: project.revision,
        },
    )
    .await
    .expect("plan with an unusable default");
    execute(&harness, &plan.job_id).await;
    let requests = harness.provider.requests.lock().expect("requests");
    assert_eq!(requests[0].profile.chat_profile.model_profile_id, text_id);
}

#[tokio::test]
async fn staged_plan_with_a_configured_non_text_model_is_typed() {
    let harness = harness(Reply::LorebookTools);
    let db = harness.context.backend().database();
    let image =
        crate::launch::tests::seed_model(db, lettuce_models::ProviderProtocol::Ollama, "ollama");
    let mut model = lettuce_models::ModelProfileRepository::get(db, image)
        .expect("model")
        .expect("exists");
    let revision = model.revision;
    model.config.capabilities.output_modalities.text =
        lettuce_models::CapabilityStatus::Unsupported;
    lettuce_models::ModelProfileRepository::upsert(db, model, Some(revision)).expect("image");
    let mut stored = GlobalSettingsStore::load(db).expect("settings");
    stored
        .settings
        .lorebook_generator
        .selection
        .model_profile_id = Some(image);
    GlobalSettingsStore::save(
        db,
        stored.settings,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("selection");
    let project = staged_project(&harness, "configured-non-text").await;
    let error = lorebook_project_plan(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "non-text-plan".into(),
            project_id: project.project_id.clone(),
            expected_revision: project.revision,
        },
    )
    .await
    .expect_err("typed");
    assert_eq!(error.code, ApiErrorCode::ModelUnavailable);
    assert_eq!(
        error.details,
        Some(dto::ApiErrorDetails::LorebookModelUnavailable {
            reason: dto::LorebookModelProblem::ConfiguredModelNotText
        })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn lost_races_report_conflict_and_the_running_batch() {
    let harness = harness(Reply::LorebookTools);
    let db = harness.context.backend().database();
    let created = staged_project(&harness, "race-project").await;
    let project_id: lettuce_types::CreationWorkflowId = created.project_id.parse().expect("id");
    let plan = lorebook_project_plan(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "race-plan".into(),
            project_id: created.project_id.clone(),
            expected_revision: created.revision,
        },
    )
    .await
    .expect("plan");
    let lost_cancel = db
        .commit_api_operation(
            "lorebook_project_cancel",
            "lost-cancel",
            "digest",
            harness.context.now(),
            |scope| {
                scope
                    .cancel_pending_lorebook_project(
                        project_id,
                        lettuce_types::Revision::new(created.revision),
                        harness.context.now(),
                    )
                    .map_err(|error| super::Failure(super::jobs::failure(error)))
            },
        )
        .map(|_| ())
        .expect_err("pending row consumed by the plan");
    assert_eq!(lost_cancel.0.code, ApiErrorCode::Conflict);
    let request_id = db
        .staged_lorebook_request_for_project(&created.project_id)
        .expect("request");
    let run = lettuce_creation::StagedLorebookRepository::load_staged_lorebook(db, request_id)
        .expect("run");
    let spec = lettuce_jobs::JobSpec::new(
        lettuce_jobs::JobKind::CreationRun,
        lettuce_jobs::JobSubject::new(
            lettuce_jobs::SubjectKind::CreationProject,
            created.project_id.clone(),
        )
        .expect("subject"),
        lettuce_jobs::OutcomeRef::Request(request_id),
    );
    let lost_plan = db
        .commit_api_operation(
            "lorebook_project_plan",
            "lost-plan",
            "digest",
            harness.context.now(),
            |scope| {
                scope
                    .plan_pending_lorebook_project(
                        lettuce_types::Revision::new(created.revision),
                        spec,
                        run,
                    )
                    .map_err(|error| super::Failure(super::jobs::failure(error)))
            },
        )
        .map(|_| ())
        .expect_err("pending row consumed by the plan");
    assert_eq!(lost_plan.0.code, ApiErrorCode::Conflict);

    execute(&harness, &plan.job_id).await;
    let project = project_get(&harness, &created.project_id).await;
    let approved = lorebook_project_outline_approve(
        &harness.context,
        dto::LorebookProjectRevisionRequest {
            client_operation_id: "race-approve".into(),
            project_id: project.project_id.clone(),
            expected_revision: project.revision,
        },
    )
    .await
    .expect("approve");
    let stale = lettuce_creation::StagedLorebookRepository::load_staged_lorebook(db, request_id)
        .expect("stale run");
    let selected = super::projects::overrides(&harness.context).expect("overrides");
    let (inputs, writers) = harness
        .context
        .backend()
        .staged_lorebook_writer_coordinator()
        .prepare_atomic_batch(
            &stale,
            &selected,
            harness.context.backend().built_in_prompt_ids(),
            harness.context.now(),
        )
        .expect("stale batch");
    let first = lorebook_project_draft_next(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "race-first".into(),
            project_id: project.project_id.clone(),
            expected_revision: approved.revision,
        },
    )
    .await
    .expect("first batch");
    let lost = super::projects::admit_batch(
        &harness.context,
        "race-second",
        "digest",
        &stale,
        lettuce_types::Revision::new(approved.revision),
        inputs,
        writers,
    )
    .expect_err("lost the admission");
    assert_eq!(lost.code, ApiErrorCode::Busy);
    let Some(dto::ApiErrorDetails::LorebookBatchRunning { job_ids }) = lost.details else {
        panic!("batch details missing");
    };
    let mut expected = first.job_ids.clone();
    expected.sort();
    let mut job_ids = job_ids;
    job_ids.sort();
    assert_eq!(job_ids, expected);
}

#[tokio::test]
async fn project_list_is_newest_first_across_pages() {
    let harness = harness(Reply::LorebookTools);
    let mut created = Vec::new();
    for key in ["first", "second", "third"] {
        created.push(staged_project(&harness, key).await);
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    let plan = lorebook_project_plan(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "touch-first".into(),
            project_id: created[0].project_id.clone(),
            expected_revision: created[0].revision,
        },
    )
    .await
    .expect("plan");
    execute(&harness, &plan.job_id).await;
    let mut listed = Vec::new();
    let mut cursor = None;
    loop {
        let page = lorebook_projects_list(
            &harness.context,
            dto::LorebookProjectsListRequest {
                cursor: cursor.clone(),
                limit: Some(2),
            },
        )
        .await
        .expect("list");
        listed.extend(page.items);
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(
        listed
            .iter()
            .map(|item| item.project_id.clone())
            .collect::<Vec<_>>(),
        vec![
            created[0].project_id.clone(),
            created[2].project_id.clone(),
            created[1].project_id.clone()
        ]
    );
}

async fn committed_midway_settles_cancelled(stage: &str) {
    let harness = harness(Reply::LorebookTools);
    let (project_id, job_id) = prepare_stage(&harness, stage).await;
    let db = harness.context.backend().database();
    let job = JobStore::get(db, job_id.parse().expect("job id"))
        .expect("job")
        .expect("exists");
    let work = LorebookHandler
        .claim(&harness.context, &job, WorkerId::new())
        .await
        .expect("claim")
        .expect("work");
    let project = project_get(&harness, &project_id).await;
    lorebook_project_commit(
        &harness.context,
        dto::LorebookProjectCommitRequest {
            client_operation_id: format!("commit-midway-{stage}"),
            project_id,
            expected_revision: project.revision,
            target: dto::LorebookProjectCommitTarget::NewLorebook { name: None },
        },
    )
    .await
    .expect("commit");
    work.run(harness.context.clone(), Arc::new(Progress))
        .await
        .expect("settle");
    assert_eq!(
        JobStore::get(db, job.id)
            .expect("job")
            .expect("exists")
            .state,
        lettuce_jobs::JobState::Cancelled
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn coherence_job_in_flight_at_commit_settles_cancelled() {
    committed_midway_settles_cancelled("coherence").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn refine_job_in_flight_at_commit_settles_cancelled() {
    let harness = harness(Reply::LorebookTools);
    let (project_id, coherence_job) = prepare_stage(&harness, "coherence").await;
    execute(&harness, &coherence_job).await;
    let project = project_get(&harness, &project_id).await;
    let refine = lorebook_project_refine(
        &harness.context,
        dto::LorebookProjectRefineRequest {
            client_operation_id: "refine-midway".into(),
            expected_revision: project.revision,
            project_id: project_id.clone(),
            plan_id: project.drafts[0].plan_id.clone(),
            feedback: "Shorter".into(),
        },
    )
    .await
    .expect("refine");
    let db = harness.context.backend().database();
    let job = JobStore::get(db, refine.job_id.parse().expect("job id"))
        .expect("job")
        .expect("exists");
    let work = LorebookHandler
        .claim(&harness.context, &job, WorkerId::new())
        .await
        .expect("claim")
        .expect("work");
    let project = project_get(&harness, &project_id).await;
    lorebook_project_commit(
        &harness.context,
        dto::LorebookProjectCommitRequest {
            client_operation_id: "commit-midway-refine".into(),
            project_id,
            expected_revision: project.revision,
            target: dto::LorebookProjectCommitTarget::NewLorebook { name: None },
        },
    )
    .await
    .expect("commit");
    work.run(harness.context.clone(), Arc::new(Progress))
        .await
        .expect("settle");
    assert_eq!(
        JobStore::get(db, job.id)
            .expect("job")
            .expect("exists")
            .state,
        lettuce_jobs::JobState::Cancelled
    );
}

#[tokio::test]
async fn staged_plan_uses_the_first_text_model_even_when_the_default_is_a_later_text_model() {
    let harness = harness(Reply::LorebookTools);
    let db = harness.context.backend().database();
    crate::launch::tests::seed_model(db, lettuce_models::ProviderProtocol::Ollama, "ollama");
    let models = lettuce_models::ModelCatalog::model_profiles(db).expect("models");
    assert_eq!(models.len(), 2);
    crate::launch::tests::set_application_default_model(db, models[1].id);
    let project = staged_project(&harness, "first-text-plan").await;
    let plan = lorebook_project_plan(
        &harness.context,
        dto::LorebookProjectJobRequest {
            client_operation_id: "first-text-plan-job".into(),
            project_id: project.project_id.clone(),
            expected_revision: project.revision,
        },
    )
    .await
    .expect("plan");
    execute(&harness, &plan.job_id).await;
    let requests = harness.provider.requests.lock().expect("requests");
    assert!(!requests.is_empty());
    for request in requests.iter() {
        assert_eq!(request.profile.chat_profile.model_profile_id, models[0].id);
    }
}
