use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{
    ApiError, HfAuthSaveRequest, HfAuthor, HfAuthorRequest, HfAvatars, HfAvatarsRequest,
    HfDownloadRequest, HfModelInfo, HfModelRequest, HfReadme, HfReadmeRequest, HfRecommendation,
    HfRecommendationRequest, HfRunnability, HfRunnabilityRequest, HfSearchRequest, HfSearchResults,
    HfTokenStatus, JobAccepted,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn hf_search(
    context: State<'_, ApiContextSlot>,
    request: HfSearchRequest,
) -> Result<HfSearchResults, ApiError> {
    let context = context.snapshot()?;
    api::hf_search(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_model_files(
    context: State<'_, ApiContextSlot>,
    request: HfModelRequest,
) -> Result<HfModelInfo, ApiError> {
    let context = context.snapshot()?;
    api::hf_model_files(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_readme(
    context: State<'_, ApiContextSlot>,
    request: HfReadmeRequest,
) -> Result<HfReadme, ApiError> {
    let context = context.snapshot()?;
    api::hf_readme(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_author(
    context: State<'_, ApiContextSlot>,
    request: HfAuthorRequest,
) -> Result<HfAuthor, ApiError> {
    let context = context.snapshot()?;
    api::hf_author(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_avatars(
    context: State<'_, ApiContextSlot>,
    request: HfAvatarsRequest,
) -> Result<HfAvatars, ApiError> {
    let context = context.snapshot()?;
    api::hf_avatars(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_runnability(
    context: State<'_, ApiContextSlot>,
    request: HfRunnabilityRequest,
) -> Result<HfRunnability, ApiError> {
    let context = context.snapshot()?;
    api::hf_runnability(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_recommendation(
    context: State<'_, ApiContextSlot>,
    request: HfRecommendationRequest,
) -> Result<HfRecommendation, ApiError> {
    let context = context.snapshot()?;
    api::hf_recommendation(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_download(
    context: State<'_, ApiContextSlot>,
    request: HfDownloadRequest,
) -> Result<JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::hf_download(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_auth_status(context: State<'_, ApiContextSlot>) -> Result<HfTokenStatus, ApiError> {
    let context = context.snapshot()?;
    api::hf_auth_status(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_auth_save(
    context: State<'_, ApiContextSlot>,
    request: HfAuthSaveRequest,
) -> Result<HfTokenStatus, ApiError> {
    let context = context.snapshot()?;
    api::hf_auth_save(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_auth_clear(context: State<'_, ApiContextSlot>) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::hf_auth_clear(&context).await
}
