use lettuce_app::api::{self, ApiContext};
use lettuce_contracts::{
    ApiError, AvatarGradient, AvatarGradientRequest, AvatarPrompt, AvatarPromptRequest,
    CivitaiAuthSaveRequest, CivitaiAuthStatus, CivitaiLoraDownloadRequest, CivitaiModelDetail,
    CivitaiModelRequest, CivitaiSearchPage, CivitaiSearchRequest, DownloadedImageModels,
    HfImageBundleFiles, HfImageBundleFilesRequest, HfImageBundleInstallRequest,
    HfImageBundleRetryRequest, HfImageBundleSearchRequest, HfImageBundleSearchResults,
    ImageBundleAccepted, ImageBundleProfiles, ImageBundleRegistered, ImageBundleRetried,
    ImageCapabilities, ImageCapabilitiesRequest, ImageDesignReferenceRequest, ImageGenerateRequest,
    ImageUpscaleRequest, InstalledLora, JobAccepted, LoraDeleted, LoraKeywordDiscovery,
    LoraKeywordsDiscoverRequest, LoraList, LorasDeleteRequest, LorasImportRequest,
    LorasListRequest, LorasUpdateKeywordsRequest, PlaygroundHistoryDeleteRequest,
    PlaygroundHistoryDeleted, PlaygroundHistoryListRequest, PlaygroundHistoryPage,
    SdBundleRunnabilityRequest, SdCatalog, SdComponentLibrary, SdComputePolicyInfo,
    SdComputePolicySaveRequest, SdDetectModelFileRequest, SdDetectedModelFile, SdDiskUsage,
    SdInstalledModels, SdModelInstallRequest, SdModelInstallStarted, SdModelRepairRequest,
    SdModelRepaired, SdModelUninstallRequest, SdRunnabilityRequest, SdRuntimeInstallRequest,
    SdRuntimeInventory, SdRuntimeRef, SdRuntimeReleases, SdUninstallOutcome, SdUpscalerInventory,
    SdUpscalerRemoveRequest,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn image_generate(
    context: State<'_, ApiContext>,
    request: ImageGenerateRequest,
) -> Result<JobAccepted, ApiError> {
    api::image_generate(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn image_upscale(
    context: State<'_, ApiContext>,
    request: ImageUpscaleRequest,
) -> Result<JobAccepted, ApiError> {
    api::image_upscale(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn image_capabilities(
    context: State<'_, ApiContext>,
    request: ImageCapabilitiesRequest,
) -> Result<ImageCapabilities, ApiError> {
    api::image_capabilities(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn playground_history_list(
    context: State<'_, ApiContext>,
    request: PlaygroundHistoryListRequest,
) -> Result<PlaygroundHistoryPage, ApiError> {
    api::playground_history_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn playground_history_delete(
    context: State<'_, ApiContext>,
    request: PlaygroundHistoryDeleteRequest,
) -> Result<PlaygroundHistoryDeleted, ApiError> {
    api::playground_history_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_catalog(context: State<'_, ApiContext>) -> Result<SdCatalog, ApiError> {
    api::sd_catalog(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_runtime_releases(
    context: State<'_, ApiContext>,
) -> Result<SdRuntimeReleases, ApiError> {
    api::sd_runtime_releases(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_runtime_install(
    context: State<'_, ApiContext>,
    request: SdRuntimeInstallRequest,
) -> Result<JobAccepted, ApiError> {
    api::sd_runtime_install(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_runtime_inventory(
    context: State<'_, ApiContext>,
) -> Result<SdRuntimeInventory, ApiError> {
    api::sd_runtime_inventory(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_runtime_switch(
    context: State<'_, ApiContext>,
    request: SdRuntimeRef,
) -> Result<(), ApiError> {
    api::sd_runtime_switch(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_runtime_delete(
    context: State<'_, ApiContext>,
    request: SdRuntimeRef,
) -> Result<(), ApiError> {
    api::sd_runtime_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_model_install(
    context: State<'_, ApiContext>,
    request: SdModelInstallRequest,
) -> Result<SdModelInstallStarted, ApiError> {
    api::sd_model_install(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_models_installed(
    context: State<'_, ApiContext>,
) -> Result<SdInstalledModels, ApiError> {
    api::sd_models_installed(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_model_uninstall(
    context: State<'_, ApiContext>,
    request: SdModelUninstallRequest,
) -> Result<SdUninstallOutcome, ApiError> {
    api::sd_model_uninstall(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_model_repair(
    context: State<'_, ApiContext>,
    request: SdModelRepairRequest,
) -> Result<SdModelRepaired, ApiError> {
    api::sd_model_repair(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_runnability(
    context: State<'_, ApiContext>,
    request: SdRunnabilityRequest,
) -> Result<JobAccepted, ApiError> {
    api::sd_runnability(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_bundle_runnability(
    context: State<'_, ApiContext>,
    request: SdBundleRunnabilityRequest,
) -> Result<JobAccepted, ApiError> {
    api::sd_bundle_runnability(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_compute_policy_get(
    context: State<'_, ApiContext>,
    request: SdRuntimeRef,
) -> Result<SdComputePolicyInfo, ApiError> {
    api::sd_compute_policy_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_compute_policy_save(
    context: State<'_, ApiContext>,
    request: SdComputePolicySaveRequest,
) -> Result<SdComputePolicyInfo, ApiError> {
    api::sd_compute_policy_save(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_detect_model_file(
    context: State<'_, ApiContext>,
    request: SdDetectModelFileRequest,
) -> Result<SdDetectedModelFile, ApiError> {
    api::sd_detect_model_file(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_disk_usage(context: State<'_, ApiContext>) -> Result<SdDiskUsage, ApiError> {
    api::sd_disk_usage(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_component_library(
    context: State<'_, ApiContext>,
) -> Result<SdComponentLibrary, ApiError> {
    api::sd_component_library(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_upscalers_list(
    context: State<'_, ApiContext>,
) -> Result<SdUpscalerInventory, ApiError> {
    api::sd_upscalers_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_upscalers_install(context: State<'_, ApiContext>) -> Result<JobAccepted, ApiError> {
    api::sd_upscalers_install(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_upscalers_remove(
    context: State<'_, ApiContext>,
    request: SdUpscalerRemoveRequest,
) -> Result<SdUpscalerInventory, ApiError> {
    api::sd_upscalers_remove(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn loras_list(
    context: State<'_, ApiContext>,
    request: LorasListRequest,
) -> Result<LoraList, ApiError> {
    api::loras_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn loras_import(
    context: State<'_, ApiContext>,
    request: LorasImportRequest,
) -> Result<InstalledLora, ApiError> {
    api::loras_import(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn loras_delete(
    context: State<'_, ApiContext>,
    request: LorasDeleteRequest,
) -> Result<LoraDeleted, ApiError> {
    api::loras_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn loras_update_keywords(
    context: State<'_, ApiContext>,
    request: LorasUpdateKeywordsRequest,
) -> Result<LoraKeywordDiscovery, ApiError> {
    api::loras_update_keywords(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lora_keywords_discover(
    context: State<'_, ApiContext>,
    request: LoraKeywordsDiscoverRequest,
) -> Result<JobAccepted, ApiError> {
    api::lora_keywords_discover(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_image_bundle_profiles(
    context: State<'_, ApiContext>,
) -> Result<ImageBundleProfiles, ApiError> {
    api::hf_image_bundle_profiles(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_image_bundle_search(
    context: State<'_, ApiContext>,
    request: HfImageBundleSearchRequest,
) -> Result<HfImageBundleSearchResults, ApiError> {
    api::hf_image_bundle_search(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_image_bundle_files(
    context: State<'_, ApiContext>,
    request: HfImageBundleFilesRequest,
) -> Result<HfImageBundleFiles, ApiError> {
    api::hf_image_bundle_files(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_image_bundle_install(
    context: State<'_, ApiContext>,
    request: HfImageBundleInstallRequest,
) -> Result<ImageBundleAccepted, ApiError> {
    api::hf_image_bundle_install(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_image_bundle_retry(
    context: State<'_, ApiContext>,
    request: HfImageBundleRetryRequest,
) -> Result<ImageBundleRetried, ApiError> {
    api::hf_image_bundle_retry(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_image_bundle_retry_registration(
    context: State<'_, ApiContext>,
    request: HfImageBundleRetryRequest,
) -> Result<ImageBundleRegistered, ApiError> {
    api::hf_image_bundle_retry_registration(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn image_models_downloaded(
    context: State<'_, ApiContext>,
) -> Result<DownloadedImageModels, ApiError> {
    api::image_models_downloaded(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn civitai_search(
    context: State<'_, ApiContext>,
    request: CivitaiSearchRequest,
) -> Result<CivitaiSearchPage, ApiError> {
    api::civitai_search(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn civitai_model(
    context: State<'_, ApiContext>,
    request: CivitaiModelRequest,
) -> Result<CivitaiModelDetail, ApiError> {
    api::civitai_model(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn civitai_auth_status(
    context: State<'_, ApiContext>,
) -> Result<CivitaiAuthStatus, ApiError> {
    api::civitai_auth_status(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn civitai_auth_save(
    context: State<'_, ApiContext>,
    request: CivitaiAuthSaveRequest,
) -> Result<CivitaiAuthStatus, ApiError> {
    api::civitai_auth_save(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn civitai_auth_clear(context: State<'_, ApiContext>) -> Result<(), ApiError> {
    api::civitai_auth_clear(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn civitai_lora_download(
    context: State<'_, ApiContext>,
    request: CivitaiLoraDownloadRequest,
) -> Result<JobAccepted, ApiError> {
    api::civitai_lora_download(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn avatar_prompt(
    context: State<'_, ApiContext>,
    request: AvatarPromptRequest,
) -> Result<AvatarPrompt, ApiError> {
    api::avatar_prompt(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn avatar_gradient(
    context: State<'_, ApiContext>,
    request: AvatarGradientRequest,
) -> Result<AvatarGradient, ApiError> {
    api::avatar_gradient(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn image_design_reference(
    context: State<'_, ApiContext>,
    request: ImageDesignReferenceRequest,
) -> Result<JobAccepted, ApiError> {
    api::image_design_reference(&context, request).await
}
