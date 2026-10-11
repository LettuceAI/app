use lettuce_app::api::ApiContextSlot;
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn embedding_status(
    context: State<'_, ApiContextSlot>,
) -> Result<Vec<dto::EmbeddingModelView>, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::embedding_status(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn embedding_choose(
    context: State<'_, ApiContextSlot>,
    request: dto::EmbeddingModelRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::embedding_choose(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn embedding_remove(
    context: State<'_, ApiContextSlot>,
    request: dto::EmbeddingModelRequest,
) -> Result<bool, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::embedding_remove(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn embedding_compare(
    context: State<'_, ApiContextSlot>,
    request: dto::EmbeddingCompareRequest,
) -> Result<dto::EmbeddingComparison, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::embedding_compare(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn embedding_unload(context: State<'_, ApiContextSlot>) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::embedding_unload(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_emotion_status(
    context: State<'_, ApiContextSlot>,
) -> Result<dto::ThymosStatus, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::companion_emotion_status(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_emotion_remove(
    context: State<'_, ApiContextSlot>,
) -> Result<bool, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::companion_emotion_remove(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn embedding_install(
    context: State<'_, ApiContextSlot>,
    request: dto::EmbeddingInstallRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::embedding_install(&context, request).await
}
#[tauri::command]
#[specta::specta]
pub async fn companion_emotion_install(
    context: State<'_, ApiContextSlot>,
) -> Result<dto::JobAccepted, ApiError> {
    let context = context.snapshot()?;
    lettuce_app::api::companion_emotion_install(&context).await
}
