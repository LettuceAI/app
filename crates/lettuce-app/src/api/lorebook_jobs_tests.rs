use super::tests::{Reply, harness};
use super::*;
use lettuce_context::{
    LifecycleStatus, PromptEntryDraft, PromptMetadataDraft, PromptPurpose, PromptRepository,
};
use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_creation::LorebookKeywordRunRepository;
use lettuce_jobs::{CancellationReason, JobMutation, JobStore, WorkerId};
use lettuce_settings::GlobalSettingsStore;
use lettuce_types::TimestampMillis;
use std::sync::Arc;

async fn book(harness: &super::tests::Harness) -> dto::LorebookView {
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

fn custom_prompt(harness: &super::tests::Harness) -> lettuce_context::PromptDocument {
    let db = harness.context.backend().database();
    let seed = PromptRepository::get(
        db,
        harness
            .context
            .backend()
            .built_in_prompt_ids()
            .lorebook_keyword_generator,
    )
    .expect("seed")
    .expect("exists");
    PromptRepository::create_user_draft(
        db,
        PromptMetadataDraft {
            name: "Keyword source".into(),
            purpose: PromptPurpose::LorebookKeywordGenerator,
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
    let id =
        super::lorebook_jobs::request_id("lorebook_keywords_draft", "keywords", &request(&book))
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
    let restored = super::lorebooks_tests::backup_round_trip(db);
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
