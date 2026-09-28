use lettuce_app::api::{self, ApiContext};
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
    context: State<'_, ApiContext>,
    request: HfSearchRequest,
) -> Result<HfSearchResults, ApiError> {
    api::hf_search(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_model_files(
    context: State<'_, ApiContext>,
    request: HfModelRequest,
) -> Result<HfModelInfo, ApiError> {
    api::hf_model_files(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_readme(
    context: State<'_, ApiContext>,
    request: HfReadmeRequest,
) -> Result<HfReadme, ApiError> {
    api::hf_readme(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_author(
    context: State<'_, ApiContext>,
    request: HfAuthorRequest,
) -> Result<HfAuthor, ApiError> {
    api::hf_author(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_avatars(
    context: State<'_, ApiContext>,
    request: HfAvatarsRequest,
) -> Result<HfAvatars, ApiError> {
    api::hf_avatars(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_runnability(
    context: State<'_, ApiContext>,
    request: HfRunnabilityRequest,
) -> Result<HfRunnability, ApiError> {
    api::hf_runnability(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_recommendation(
    context: State<'_, ApiContext>,
    request: HfRecommendationRequest,
) -> Result<HfRecommendation, ApiError> {
    api::hf_recommendation(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_download(
    context: State<'_, ApiContext>,
    request: HfDownloadRequest,
) -> Result<JobAccepted, ApiError> {
    api::hf_download(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_auth_status(context: State<'_, ApiContext>) -> Result<HfTokenStatus, ApiError> {
    api::hf_auth_status(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_auth_save(
    context: State<'_, ApiContext>,
    request: HfAuthSaveRequest,
) -> Result<HfTokenStatus, ApiError> {
    api::hf_auth_save(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_auth_clear(context: State<'_, ApiContext>) -> Result<(), ApiError> {
    api::hf_auth_clear(&context).await
}
