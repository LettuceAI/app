use lettuce_app::api::{self, ApiContext};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn lorebooks_list(
    context: State<'_, ApiContext>,
    request: dto::LorebooksListRequest,
) -> Result<dto::LorebookPage, ApiError> {
    api::lorebooks_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_get(
    context: State<'_, ApiContext>,
    request: dto::LorebookGetRequest,
) -> Result<dto::LorebookView, ApiError> {
    api::lorebook_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_create(
    context: State<'_, ApiContext>,
    request: dto::LorebookCreateRequest,
) -> Result<dto::LorebookView, ApiError> {
    api::lorebook_create(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_update_metadata(
    context: State<'_, ApiContext>,
    request: dto::LorebookUpdateMetadataRequest,
) -> Result<dto::LorebookView, ApiError> {
    api::lorebook_update_metadata(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_entries_mutate(
    context: State<'_, ApiContext>,
    request: dto::LorebookEntriesMutateRequest,
) -> Result<dto::LorebookView, ApiError> {
    api::lorebook_entries_mutate(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_archive(
    context: State<'_, ApiContext>,
    request: dto::LorebookRevisionRequest,
) -> Result<dto::LorebookView, ApiError> {
    api::lorebook_archive(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_restore(
    context: State<'_, ApiContext>,
    request: dto::LorebookRevisionRequest,
) -> Result<dto::LorebookView, ApiError> {
    api::lorebook_restore(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_delete(
    context: State<'_, ApiContext>,
    request: dto::LorebookRevisionRequest,
) -> Result<dto::SourceDeleteResult, ApiError> {
    api::lorebook_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn tokens_count(
    context: State<'_, ApiContext>,
    request: dto::TokensCountRequest,
) -> Result<dto::TokensCount, ApiError> {
    api::tokens_count(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_trigger_preview(
    context: State<'_, ApiContext>,
    request: dto::LorebookTriggerPreviewRequest,
) -> Result<dto::LorebookTriggerPreview, ApiError> {
    api::lorebook_trigger_preview(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_entry_draft(
    context: State<'_, ApiContext>,
    request: dto::LorebookEntryDraftRequest,
) -> Result<dto::JobAccepted, ApiError> {
    api::lorebook_entry_draft(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_keywords_draft(
    context: State<'_, ApiContext>,
    request: dto::LorebookKeywordsDraftRequest,
) -> Result<dto::JobAccepted, ApiError> {
    api::lorebook_keywords_draft(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_create(
    context: State<'_, ApiContext>,
    request: dto::LorebookProjectCreateRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    api::lorebook_project_create(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_get(
    context: State<'_, ApiContext>,
    request: dto::LorebookProjectGetRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    api::lorebook_project_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_projects_list(
    context: State<'_, ApiContext>,
    request: dto::LorebookProjectsListRequest,
) -> Result<dto::LorebookProjectPage, ApiError> {
    api::lorebook_projects_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_plan(
    context: State<'_, ApiContext>,
    request: dto::LorebookProjectJobRequest,
) -> Result<dto::JobAccepted, ApiError> {
    api::lorebook_project_plan(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_outline_update(
    context: State<'_, ApiContext>,
    request: dto::LorebookProjectOutlineUpdateRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    api::lorebook_project_outline_update(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_outline_approve(
    context: State<'_, ApiContext>,
    request: dto::LorebookProjectRevisionRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    api::lorebook_project_outline_approve(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_draft_update(
    context: State<'_, ApiContext>,
    request: dto::LorebookProjectDraftUpdateRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    api::lorebook_project_draft_update(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_draft_set_approved(
    context: State<'_, ApiContext>,
    request: dto::LorebookProjectDraftApprovalRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    api::lorebook_project_draft_set_approved(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_coherence_apply(
    context: State<'_, ApiContext>,
    request: dto::LorebookProjectCoherenceApplyRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    api::lorebook_project_coherence_apply(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_cancel(
    context: State<'_, ApiContext>,
    request: dto::LorebookProjectRevisionRequest,
) -> Result<dto::LorebookProjectView, ApiError> {
    api::lorebook_project_cancel(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_generator_defaults(
    context: State<'_, ApiContext>,
) -> Result<dto::LorebookGeneratorDefaults, ApiError> {
    api::lorebook_generator_defaults(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_draft_next(
    context: State<'_, ApiContext>,
    request: dto::LorebookProjectJobRequest,
) -> Result<dto::LorebookProjectBatchAccepted, ApiError> {
    api::lorebook_project_draft_next(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_refine(
    context: State<'_, ApiContext>,
    request: dto::LorebookProjectRefineRequest,
) -> Result<dto::JobAccepted, ApiError> {
    api::lorebook_project_refine(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_coherence(
    context: State<'_, ApiContext>,
    request: dto::LorebookProjectJobRequest,
) -> Result<dto::JobAccepted, ApiError> {
    api::lorebook_project_coherence(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_project_commit(
    context: State<'_, ApiContext>,
    request: dto::LorebookProjectCommitRequest,
) -> Result<dto::LorebookProjectCommitView, ApiError> {
    api::lorebook_project_commit(&context, request).await
}
