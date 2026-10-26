use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts as dto;
use lettuce_contracts::{
    ApiError, AssetRef, AssetsIngestRequest, FileInspection, FileSavePicked, FilesInspectRequest,
    FilesPickOpenRequest, FilesPickSaveRequest, FilesPicked,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn files_inspect(
    context: State<'_, ApiContextSlot>,
    request: FilesInspectRequest,
) -> Result<FileInspection, ApiError> {
    let context = context.snapshot()?;
    api::files_inspect(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn assets_ingest(
    context: State<'_, ApiContextSlot>,
    request: AssetsIngestRequest,
) -> Result<AssetRef, ApiError> {
    let context = context.snapshot()?;
    api::assets_ingest(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn files_pick_open(
    context: State<'_, ApiContextSlot>,
    request: FilesPickOpenRequest,
) -> Result<FilesPicked, ApiError> {
    let context = context.snapshot()?;
    api::files_pick_open(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn files_pick_directory(
    context: State<'_, ApiContextSlot>,
) -> Result<dto::DirectoryPicked, ApiError> {
    let context = context.snapshot()?;
    api::files_pick_directory(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn files_pick_save(
    context: State<'_, ApiContextSlot>,
    request: FilesPickSaveRequest,
) -> Result<FileSavePicked, ApiError> {
    let context = context.snapshot()?;
    api::files_pick_save(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn backup_inspect(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::BackupInspectRequest,
) -> Result<lettuce_contracts::BackupHeaderView, ApiError> {
    let context = context.snapshot()?;
    api::backup_inspect(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn backups_list(
    context: State<'_, ApiContextSlot>,
) -> Result<Vec<dto::BackupIndexView>, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::backups_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn backup_forget(
    context: State<'_, ApiContextSlot>,
    request: dto::BackupMutationRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::backup_forget(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn backup_delete(
    context: State<'_, ApiContextSlot>,
    request: dto::BackupMutationRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::backup_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn backup_restore(
    context: State<'_, ApiContextSlot>,
    request: dto::BackupRestoreRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::backup_restore(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn legacy_backup_restore(
    context: State<'_, ApiContextSlot>,
    request: dto::BackupRestoreRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::legacy_backup_restore(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn legacy_database_import(
    context: State<'_, ApiContextSlot>,
    request: dto::LegacyDatabaseImportRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::legacy_database_import(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn backup_export(
    context: State<'_, ApiContextSlot>,
    request: dto::BackupExportRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::backup_export(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn import_preview(
    context: State<'_, ApiContextSlot>,
    request: dto::ImportPreviewRequest,
) -> Result<dto::ImportPreview, ApiError> {
    let context = context.snapshot()?;
    api::import_preview(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn import_commit(
    context: State<'_, ApiContextSlot>,
    request: dto::ImportCommitRequest,
) -> Result<dto::ImportCommitResult, ApiError> {
    let context = context.snapshot()?;
    api::import_commit(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn character_export(
    context: State<'_, ApiContextSlot>,
    request: dto::CharacterExportRequest,
) -> Result<dto::EntityExportResult, ApiError> {
    let context = context.snapshot()?;
    api::character_export(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn persona_export(
    context: State<'_, ApiContextSlot>,
    request: dto::EntityExportRequest,
) -> Result<dto::EntityExportResult, ApiError> {
    let context = context.snapshot()?;
    api::persona_export(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lorebook_export(
    context: State<'_, ApiContextSlot>,
    request: dto::LorebookExportRequest,
) -> Result<dto::EntityExportResult, ApiError> {
    let context = context.snapshot()?;
    api::lorebook_export(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn prompt_export(
    context: State<'_, ApiContextSlot>,
    request: dto::EntityExportRequest,
) -> Result<dto::EntityExportResult, ApiError> {
    let context = context.snapshot()?;
    api::prompt_export(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn chat_template_export(
    context: State<'_, ApiContextSlot>,
    request: dto::ChatTemplateExportRequest,
) -> Result<dto::EntityExportResult, ApiError> {
    let context = context.snapshot()?;
    api::chat_template_export(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn model_export(
    context: State<'_, ApiContextSlot>,
    request: dto::ModelExportRequest,
) -> Result<dto::EntityExportResult, ApiError> {
    let context = context.snapshot()?;
    api::model_export(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn chat_export(
    context: State<'_, ApiContextSlot>,
    request: dto::ChatExportRequest,
) -> Result<dto::EntityExportResult, ApiError> {
    let context = context.snapshot()?;
    api::chat_export(&context, request).await
}
