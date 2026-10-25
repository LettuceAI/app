use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn lorebooks_list(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebooksListRequest,
) -> Result<dto::LorebookPage, ApiError> {
    let context = context.snapshot()?;
    api::lorebooks_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_get(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookGetRequest,
) -> Result<dto::LorebookView, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_create(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookCreateRequest,
) -> Result<dto::LorebookView, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_create(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_update_metadata(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookUpdateMetadataRequest,
) -> Result<dto::LorebookView, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_update_metadata(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_entries_mutate(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookEntriesMutateRequest,
) -> Result<dto::LorebookView, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_entries_mutate(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_archive(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookRevisionRequest,
) -> Result<dto::LorebookView, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_archive(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_restore(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookRevisionRequest,
) -> Result<dto::LorebookView, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_restore(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_delete(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookRevisionRequest,
) -> Result<dto::SourceDeleteResult, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn tokens_count(
    context: State<'_, ApiContextSlot>,
    request: dto::TokensCountRequest,
) -> Result<dto::TokensCount, ApiError> {
    let context = context.snapshot()?;
    api::tokens_count(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_trigger_preview(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookTriggerPreviewRequest,
) -> Result<dto::LorebookTriggerPreview, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_trigger_preview(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_entry_draft(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookEntryDraftRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_entry_draft(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_keywords_draft(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookKeywordsDraftRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_keywords_draft(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_create(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookProjectCreateRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_project_create(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_get(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookProjectGetRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_project_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_projects_list(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookProjectsListRequest,
) -> Result<dto::LorebookProjectPage, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_projects_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_plan(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookProjectJobRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_project_plan(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_outline_update(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookProjectOutlineUpdateRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_project_outline_update(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_outline_approve(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookProjectRevisionRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_project_outline_approve(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_draft_update(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookProjectDraftUpdateRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_project_draft_update(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_draft_set_approved(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookProjectDraftApprovalRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_project_draft_set_approved(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_coherence_apply(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookProjectCoherenceApplyRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_project_coherence_apply(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_cancel(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookProjectRevisionRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_project_cancel(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_generator_defaults(
    context: State<'_, ApiContextSlot>,
) -> Result<dto::LorebookGeneratorDefaults, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_generator_defaults(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_draft_next(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookProjectJobRequest,
) -> Result<dto::LorebookProjectBatchAccepted, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_project_draft_next(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_write_all(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookProjectJobRequest,
) -> Result<dto::LorebookProjectBatchAccepted, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_project_write_all(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_stop(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookProjectRevisionRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_project_stop(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_refine(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookProjectRefineRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_project_refine(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_coherence(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookProjectJobRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_project_coherence(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_commit(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookProjectCommitRequest,
) -> Result<dto::LorebookProjectCommitView, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_project_commit(&context, request).await
}
