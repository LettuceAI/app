use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn creator_start(
    context: State<'_, ApiContextSlot>,
    request: dto::CreatorStartRequest,
) -> Result<dto::CreatorView, ApiError> {
    let context = context.snapshot()?;
    api::creator_start(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn creator_get(
    context: State<'_, ApiContextSlot>,
    request: dto::CreatorGetRequest,
) -> Result<dto::CreatorView, ApiError> {
    let context = context.snapshot()?;
    api::creator_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn creators_list(
    context: State<'_, ApiContextSlot>,
    request: dto::CreatorsListRequest,
) -> Result<dto::CreatorPage, ApiError> {
    let context = context.snapshot()?;
    api::creators_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn creator_edit(
    context: State<'_, ApiContextSlot>,
    request: dto::CreatorEditRequest,
) -> Result<dto::CreatorView, ApiError> {
    let context = context.snapshot()?;
    api::creator_edit(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn creator_review(
    context: State<'_, ApiContextSlot>,
    request: dto::CreatorRevisionRequest,
) -> Result<dto::CreatorView, ApiError> {
    let context = context.snapshot()?;
    api::creator_review(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn creator_apply(
    context: State<'_, ApiContextSlot>,
    request: dto::CreatorRevisionRequest,
) -> Result<dto::CreatorView, ApiError> {
    let context = context.snapshot()?;
    api::creator_apply(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn creator_send(
    context: State<'_, ApiContextSlot>,
    request: dto::CreatorSendRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::creator_send(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn creator_regenerate(
    context: State<'_, ApiContextSlot>,
    request: dto::CreatorRegenerateRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::creator_regenerate(&context, request).await
}
