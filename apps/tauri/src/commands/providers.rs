use lettuce_app::api::{self, ApiContext};
use lettuce_contracts::{
    ApiError, ProviderAccountView, ProviderCatalogContract, ProviderOpenRouterEndpoint,
    ProviderOpenRouterEndpointsRequest, ProviderVerified, ProviderVerifyRequest,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn provider_catalog(
    context: State<'_, ApiContext>,
) -> Result<ProviderCatalogContract, ApiError> {
    api::provider_catalog(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn provider_accounts_list(
    context: State<'_, ApiContext>,
) -> Result<Vec<ProviderAccountView>, ApiError> {
    api::provider_accounts_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn provider_verify(
    context: State<'_, ApiContext>,
    request: ProviderVerifyRequest,
) -> Result<ProviderVerified, ApiError> {
    api::provider_verify(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn provider_openrouter_endpoints(
    context: State<'_, ApiContext>,
    request: ProviderOpenRouterEndpointsRequest,
) -> Result<Vec<ProviderOpenRouterEndpoint>, ApiError> {
    api::provider_openrouter_endpoints(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn certificates_list(
    context: State<'_, ApiContext>,
) -> Result<lettuce_contracts::CertificatesView, ApiError> {
    api::certificates_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn provider_account_save(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::ProviderAccountSaveRequest,
) -> Result<lettuce_contracts::ProviderAccountView, ApiError> {
    api::provider_account_save(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn provider_account_delete(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::ProviderAccountDeleteRequest,
) -> Result<(), ApiError> {
    api::provider_account_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn provider_models(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::ProviderModelsRequest,
) -> Result<Vec<lettuce_contracts::RemoteModelContract>, ApiError> {
    api::provider_models(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn provider_model_verify(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::ProviderModelVerifyRequest,
) -> Result<lettuce_contracts::ProviderModelVerified, ApiError> {
    api::provider_model_verify(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn certificates_import(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::CertificatesImportRequest,
) -> Result<lettuce_contracts::CertificatesView, ApiError> {
    api::certificates_import(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn certificates_remove(
    context: State<'_, ApiContext>,
    request: lettuce_contracts::CertificatesRemoveRequest,
) -> Result<lettuce_contracts::CertificatesView, ApiError> {
    api::certificates_remove(&context, request).await
}
