use lettuce_app::api::{self, ApiContextSlot};
use lettuce_contracts::{
    ApiError, ProviderAccountView, ProviderCatalogContract, ProviderOpenRouterEndpoint,
    ProviderOpenRouterEndpointsRequest, ProviderVerified, ProviderVerifyRequest,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn provider_catalog(
    context: State<'_, ApiContextSlot>,
) -> Result<ProviderCatalogContract, ApiError> {
    let context = context.snapshot()?;
    api::provider_catalog(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn provider_accounts_list(
    context: State<'_, ApiContextSlot>,
) -> Result<Vec<ProviderAccountView>, ApiError> {
    let context = context.snapshot()?;
    api::provider_accounts_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn provider_verify(
    context: State<'_, ApiContextSlot>,
    request: ProviderVerifyRequest,
) -> Result<ProviderVerified, ApiError> {
    let context = context.snapshot()?;
    api::provider_verify(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn provider_openrouter_endpoints(
    context: State<'_, ApiContextSlot>,
    request: ProviderOpenRouterEndpointsRequest,
) -> Result<Vec<ProviderOpenRouterEndpoint>, ApiError> {
    let context = context.snapshot()?;
    api::provider_openrouter_endpoints(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn certificates_list(
    context: State<'_, ApiContextSlot>,
) -> Result<lettuce_contracts::CertificatesView, ApiError> {
    let context = context.snapshot()?;
    api::certificates_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn provider_account_save(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::ProviderAccountSaveRequest,
) -> Result<lettuce_contracts::ProviderAccountView, ApiError> {
    let context = context.snapshot()?;
    api::provider_account_save(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn provider_account_delete(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::ProviderAccountDeleteRequest,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::provider_account_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn provider_models(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::ProviderModelsRequest,
) -> Result<Vec<lettuce_contracts::RemoteModelContract>, ApiError> {
    let context = context.snapshot()?;
    api::provider_models(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn provider_model_verify(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::ProviderModelVerifyRequest,
) -> Result<lettuce_contracts::ProviderModelVerified, ApiError> {
    let context = context.snapshot()?;
    api::provider_model_verify(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn certificates_import(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::CertificatesImportRequest,
) -> Result<lettuce_contracts::CertificatesView, ApiError> {
    let context = context.snapshot()?;
    api::certificates_import(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn certificates_remove(
    context: State<'_, ApiContextSlot>,
    request: lettuce_contracts::CertificatesRemoveRequest,
) -> Result<lettuce_contracts::CertificatesView, ApiError> {
    let context = context.snapshot()?;
    api::certificates_remove(&context, request).await
}
