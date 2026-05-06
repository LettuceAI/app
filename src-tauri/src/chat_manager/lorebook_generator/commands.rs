use serde::Deserialize;
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

use super::intake::build_digest;
use super::pipeline::{
    apply_coherence_changes, clamp_target_count, default_target_count_from_settings, load_settings,
    run_coherence, run_planner, run_refine, run_writer,
};
use super::state::{
    CoherenceChange, DraftStatus, EntryDraft, EntryPlan, JobRegistry, JobStage, JobState,
    PromptOverrides, SourceInput, WorldDigest,
};
use crate::storage_manager::db::open_db;
use crate::storage_manager::lorebook::{
    upsert_lorebook, upsert_lorebook_entry, Lorebook, LorebookEntry, LorebookKeywordDetectionMode,
};

const PARALLEL_DRAFT_BATCH: usize = 3;

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn registry<'a>(app: &'a AppHandle) -> State<'a, JobRegistry> {
    app.state::<JobRegistry>()
}

fn snapshot(reg: &JobRegistry, job_id: &str) -> Result<JobState, String> {
    let map = reg.jobs.lock().map_err(|_| "job registry poisoned".to_string())?;
    map.get(job_id)
        .cloned()
        .ok_or_else(|| format!("Job {} not found", job_id))
}

fn with_job<F, R>(reg: &JobRegistry, job_id: &str, f: F) -> Result<R, String>
where
    F: FnOnce(&mut JobState) -> Result<R, String>,
{
    let mut map = reg.jobs.lock().map_err(|_| "job registry poisoned".to_string())?;
    let job = map
        .get_mut(job_id)
        .ok_or_else(|| format!("Job {} not found", job_id))?;
    let result = f(job)?;
    job.updated_at_ms = now_ms();
    Ok(result)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateJobArgs {
    pub brief: String,
    pub sources: Vec<SourceInput>,
    pub target_count: u32,
    #[serde(default)]
    pub initial_lorebook_name: Option<String>,
    #[serde(default)]
    pub overrides: Option<PromptOverrides>,
}

#[tauri::command]
pub async fn lorebook_gen_create(
    app: AppHandle,
    args: CreateJobArgs,
) -> Result<JobState, String> {
    let CreateJobArgs {
        brief,
        sources,
        target_count,
        initial_lorebook_name,
        overrides,
    } = args;
    if brief.trim().is_empty() {
        return Err("Brief cannot be empty".to_string());
    }
    let target_count = clamp_target_count(target_count);
    let digest: WorldDigest = build_digest(&sources)?;

    let id = format!("lbgen_{}", Uuid::new_v4());
    let now = now_ms();
    let job = JobState {
        id: id.clone(),
        created_at_ms: now,
        updated_at_ms: now,
        stage: JobStage::Created,
        brief,
        initial_lorebook_name,
        target_count,
        digest,
        overrides: overrides.unwrap_or_default(),
        outline: Vec::new(),
        drafts: Vec::new(),
        coherence_proposals: Vec::new(),
        last_error: None,
    };

    let reg = registry(&app);
    {
        let mut map = reg.jobs.lock().map_err(|_| "job registry poisoned".to_string())?;
        map.insert(id, job.clone());
    }
    Ok(job)
}

#[tauri::command]
pub async fn lorebook_gen_get(app: AppHandle, job_id: String) -> Result<JobState, String> {
    let reg = registry(&app);
    snapshot(&reg, &job_id)
}

#[tauri::command]
pub async fn lorebook_gen_default_target_count(app: AppHandle) -> Result<u32, String> {
    let settings = load_settings(&app)?;
    Ok(default_target_count_from_settings(&settings))
}

#[tauri::command]
pub async fn lorebook_gen_run_planner(
    app: AppHandle,
    job_id: String,
) -> Result<JobState, String> {
    let snap = {
        let reg = registry(&app);
        snapshot(&reg, &job_id)?
    };

    {
        let reg = registry(&app);
        with_job(&reg, &job_id, |job| {
            job.stage = JobStage::Planning;
            job.last_error = None;
            Ok(())
        })?;
    }

    let result = run_planner(&app, &snap).await;
    let reg = registry(&app);
    match result {
        Ok(plan) => with_job(&reg, &job_id, |job| {
            job.outline = plan;
            job.stage = JobStage::AwaitingOutlineApproval;
            Ok(job.clone())
        }),
        Err(err) => with_job(&reg, &job_id, |job| {
            job.stage = JobStage::Failed;
            job.last_error = Some(err.clone());
            Err(err)
        }),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOutlineArgs {
    pub job_id: String,
    pub outline: Vec<EntryPlan>,
}

#[tauri::command]
pub async fn lorebook_gen_update_outline(
    app: AppHandle,
    args: UpdateOutlineArgs,
) -> Result<JobState, String> {
    let reg = registry(&app);
    with_job(&reg, &args.job_id, |job| {
        let mut next = args.outline.clone();
        for (i, p) in next.iter_mut().enumerate() {
            p.idx = i;
        }
        job.outline = next;
        Ok(job.clone())
    })
}

#[tauri::command]
pub async fn lorebook_gen_approve_outline(
    app: AppHandle,
    job_id: String,
) -> Result<JobState, String> {
    let reg = registry(&app);
    with_job(&reg, &job_id, |job| {
        if job.outline.is_empty() {
            return Err("Outline is empty; cannot approve".to_string());
        }
        job.drafts = job
            .outline
            .iter()
            .map(|p| EntryDraft {
                plan_idx: p.idx,
                title: p.title.clone(),
                keywords: p.proposed_keys.clone(),
                content: String::new(),
                always_active: false,
                status: DraftStatus::Pending,
                revisions: Vec::new(),
            })
            .collect();
        job.stage = JobStage::Drafting;
        Ok(job.clone())
    })
}

#[tauri::command]
pub async fn lorebook_gen_draft_next(
    app: AppHandle,
    job_id: String,
) -> Result<JobState, String> {
    use futures::stream::{FuturesUnordered, StreamExt};

    let snap = {
        let reg = registry(&app);
        snapshot(&reg, &job_id)?
    };

    let pending: Vec<EntryPlan> = snap
        .drafts
        .iter()
        .enumerate()
        .filter(|(_, d)| matches!(d.status, DraftStatus::Pending | DraftStatus::Failed))
        .filter_map(|(i, _)| snap.outline.iter().find(|p| p.idx == i).cloned())
        .take(PARALLEL_DRAFT_BATCH)
        .collect();

    if pending.is_empty() {
        let reg = registry(&app);
        return with_job(&reg, &job_id, |job| {
            if job
                .drafts
                .iter()
                .all(|d| !matches!(d.status, DraftStatus::Pending | DraftStatus::Drafting))
            {
                job.stage = JobStage::DraftsReady;
            }
            Ok(job.clone())
        });
    }

    {
        let reg = registry(&app);
        with_job(&reg, &job_id, |job| {
            for plan in &pending {
                if let Some(d) = job.drafts.iter_mut().find(|d| d.plan_idx == plan.idx) {
                    d.status = DraftStatus::Drafting;
                }
            }
            Ok(())
        })?;
    }

    let mut futures = FuturesUnordered::new();
    for plan in pending.into_iter() {
        let app_clone = app.clone();
        let snap_clone = snap.clone();
        futures.push(async move {
            let res = run_writer(&app_clone, &snap_clone, &plan).await;
            (plan.idx, res)
        });
    }

    let mut last_error: Option<String> = None;
    while let Some((plan_idx, res)) = futures.next().await {
        let reg = registry(&app);
        let _ = with_job(&reg, &job_id, |job| {
            match res {
                Ok(draft) => {
                    if let Some(d) = job.drafts.iter_mut().find(|d| d.plan_idx == plan_idx) {
                        *d = draft;
                    }
                }
                Err(err) => {
                    if let Some(d) = job.drafts.iter_mut().find(|d| d.plan_idx == plan_idx) {
                        d.status = DraftStatus::Failed;
                    }
                    last_error = Some(err);
                }
            }
            Ok(())
        });
    }

    let reg = registry(&app);
    with_job(&reg, &job_id, |job| {
        if job
            .drafts
            .iter()
            .all(|d| !matches!(d.status, DraftStatus::Pending | DraftStatus::Drafting))
        {
            if job.drafts.iter().any(|d| d.status == DraftStatus::Failed) {
                job.last_error = last_error.clone();
            }
            job.stage = JobStage::DraftsReady;
        }
        Ok(job.clone())
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefineEntryArgs {
    pub job_id: String,
    pub entry_idx: usize,
    pub feedback: String,
}

#[tauri::command]
pub async fn lorebook_gen_refine_entry(
    app: AppHandle,
    args: RefineEntryArgs,
) -> Result<JobState, String> {
    if args.feedback.trim().is_empty() {
        return Err("Feedback cannot be empty".to_string());
    }
    let snap = {
        let reg = registry(&app);
        snapshot(&reg, &args.job_id)?
    };
    let draft = snap
        .drafts
        .iter()
        .find(|d| d.plan_idx == args.entry_idx)
        .cloned()
        .ok_or_else(|| format!("Entry {} not found", args.entry_idx))?;

    let revised = run_refine(&app, &snap, &draft, args.feedback.trim()).await?;

    let reg = registry(&app);
    with_job(&reg, &args.job_id, |job| {
        if let Some(d) = job.drafts.iter_mut().find(|d| d.plan_idx == args.entry_idx) {
            *d = revised;
        }
        Ok(job.clone())
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveDraftArgs {
    pub job_id: String,
    pub entry_idx: usize,
    #[serde(default)]
    pub approved: bool,
}

#[tauri::command]
pub async fn lorebook_gen_set_draft_approved(
    app: AppHandle,
    args: ApproveDraftArgs,
) -> Result<JobState, String> {
    let reg = registry(&app);
    with_job(&reg, &args.job_id, |job| {
        if let Some(d) = job.drafts.iter_mut().find(|d| d.plan_idx == args.entry_idx) {
            d.status = if args.approved {
                DraftStatus::Approved
            } else {
                DraftStatus::Drafted
            };
        }
        Ok(job.clone())
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditDraftArgs {
    pub job_id: String,
    pub entry_idx: usize,
    pub title: String,
    pub keywords: Vec<String>,
    pub content: String,
    pub always_active: bool,
}

#[tauri::command]
pub async fn lorebook_gen_edit_draft(
    app: AppHandle,
    args: EditDraftArgs,
) -> Result<JobState, String> {
    let reg = registry(&app);
    with_job(&reg, &args.job_id, |job| {
        if let Some(d) = job.drafts.iter_mut().find(|d| d.plan_idx == args.entry_idx) {
            d.title = args.title.clone();
            d.keywords = args
                .keywords
                .iter()
                .map(|k| k.trim().to_string())
                .filter(|k| !k.is_empty())
                .collect();
            d.content = args.content.clone();
            d.always_active = args.always_active;
            if d.status == DraftStatus::Pending || d.status == DraftStatus::Failed {
                d.status = DraftStatus::Drafted;
            }
        }
        Ok(job.clone())
    })
}

#[tauri::command]
pub async fn lorebook_gen_run_coherence(
    app: AppHandle,
    job_id: String,
) -> Result<JobState, String> {
    let snap = {
        let reg = registry(&app);
        snapshot(&reg, &job_id)?
    };
    let proposals = run_coherence(&app, &snap).await?;
    let reg = registry(&app);
    with_job(&reg, &job_id, |job| {
        job.coherence_proposals = proposals;
        job.stage = JobStage::CoherenceReview;
        Ok(job.clone())
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyCoherenceArgs {
    pub job_id: String,
    pub accepted_change_ids: Vec<String>,
}

#[tauri::command]
pub async fn lorebook_gen_apply_coherence(
    app: AppHandle,
    args: ApplyCoherenceArgs,
) -> Result<JobState, String> {
    let reg = registry(&app);
    with_job(&reg, &args.job_id, |job| {
        let accepted: Vec<CoherenceChange> = job
            .coherence_proposals
            .iter()
            .filter(|c| args.accepted_change_ids.contains(change_id(c)))
            .cloned()
            .collect();
        apply_coherence_changes(&mut job.drafts, &accepted);
        job.coherence_proposals.clear();
        job.stage = JobStage::DraftsReady;
        Ok(job.clone())
    })
}

fn change_id(c: &CoherenceChange) -> &String {
    match c {
        CoherenceChange::MergeKeys { id, .. } => id,
        CoherenceChange::RenameTerm { id, .. } => id,
        CoherenceChange::FlagContradiction { id, .. } => id,
        CoherenceChange::ToggleAlwaysActive { id, .. } => id,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitArgs {
    pub job_id: String,
    #[serde(default)]
    pub target_lorebook_id: Option<String>,
    #[serde(default)]
    pub new_name: Option<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitResult {
    pub lorebook_id: String,
    pub created_entry_ids: Vec<String>,
}

#[tauri::command]
pub async fn lorebook_gen_commit(
    app: AppHandle,
    args: CommitArgs,
) -> Result<CommitResult, String> {
    let snap = {
        let reg = registry(&app);
        snapshot(&reg, &args.job_id)?
    };

    let conn = open_db(&app)?;
    let now = now_ms() as i64;

    let lorebook_id = if let Some(id) = args
        .target_lorebook_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        id.to_string()
    } else {
        let name = args
            .new_name
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .or(snap
                .initial_lorebook_name
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty()))
            .ok_or_else(|| "A lorebook name is required when creating a new lorebook".to_string())?;
        let new_lb = Lorebook {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            avatar_path: None,
            keyword_detection_mode: LorebookKeywordDetectionMode::default(),
            created_at: now,
            updated_at: now,
        };
        let saved = upsert_lorebook(&conn, &new_lb)?;
        saved.id
    };

    let mut created = Vec::new();
    for (display_order, draft) in snap.drafts.iter().enumerate() {
        if draft.title.trim().is_empty() && draft.content.trim().is_empty() {
            continue;
        }
        let entry = LorebookEntry {
            id: Uuid::new_v4().to_string(),
            lorebook_id: lorebook_id.clone(),
            title: draft.title.clone(),
            enabled: true,
            always_active: draft.always_active,
            keywords: draft.keywords.clone(),
            case_sensitive: false,
            content: draft.content.clone(),
            priority: 0,
            display_order: display_order as i32,
            created_at: now,
            updated_at: now,
        };
        let saved = upsert_lorebook_entry(&conn, &entry)?;
        created.push(saved.id);
    }

    let reg = registry(&app);
    let _ = with_job(&reg, &args.job_id, |job| {
        job.stage = JobStage::Committed;
        Ok(())
    });

    Ok(CommitResult {
        lorebook_id,
        created_entry_ids: created,
    })
}

#[tauri::command]
pub async fn lorebook_gen_cancel(app: AppHandle, job_id: String) -> Result<(), String> {
    let reg = registry(&app);
    let mut map = reg.jobs.lock().map_err(|_| "job registry poisoned".to_string())?;
    map.remove(&job_id);
    Ok(())
}
