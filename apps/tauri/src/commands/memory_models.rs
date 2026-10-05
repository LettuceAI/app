use lettuce_app::api::ApiContext;
use lettuce_contracts::{self as dto, ApiError};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn embedding_status(
    context: State<'_, ApiContext>,
) -> Result<Vec<dto::EmbeddingModelView>, ApiError> {
    lettuce_app::api::embedding_status(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn embedding_choose(
    context: State<'_, ApiContext>,
    request: dto::EmbeddingModelRequest,
) -> Result<(), ApiError> {
    lettuce_app::api::embedding_choose(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn embedding_remove(
    context: State<'_, ApiContext>,
    request: dto::EmbeddingModelRequest,
) -> Result<bool, ApiError> {
    lettuce_app::api::embedding_remove(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn embedding_compare(
    context: State<'_, ApiContext>,
    request: dto::EmbeddingCompareRequest,
) -> Result<dto::EmbeddingComparison, ApiError> {
    lettuce_app::api::embedding_compare(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn embedding_unload(context: State<'_, ApiContext>) -> Result<(), ApiError> {
    lettuce_app::api::embedding_unload(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_emotion_status(
    context: State<'_, ApiContext>,
) -> Result<dto::ThymosStatus, ApiError> {
    lettuce_app::api::companion_emotion_status(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn companion_emotion_remove(context: State<'_, ApiContext>) -> Result<bool, ApiError> {
    lettuce_app::api::companion_emotion_remove(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn embedding_install(
    context: State<'_, ApiContext>,
    request: dto::EmbeddingInstallRequest,
) -> Result<dto::JobAccepted, ApiError> {
    lettuce_app::api::embedding_install(&context, request).await
}
#[tauri::command]
#[specta::specta]
pub async fn companion_emotion_install(
    context: State<'_, ApiContext>,
) -> Result<dto::JobAccepted, ApiError> {
    lettuce_app::api::companion_emotion_install(&context).await
}
