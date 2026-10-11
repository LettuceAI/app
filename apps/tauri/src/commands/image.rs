use lettuce_app::api::{self, ApiContextSlot};
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
    context: State<'_, ApiContextSlot>,
    request: ImageGenerateRequest,
) -> Result<JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::image_generate(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn image_upscale(
    context: State<'_, ApiContextSlot>,
    request: ImageUpscaleRequest,
) -> Result<JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::image_upscale(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn image_capabilities(
    context: State<'_, ApiContextSlot>,
    request: ImageCapabilitiesRequest,
) -> Result<ImageCapabilities, ApiError> {
    let context = context.snapshot()?;
    api::image_capabilities(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn playground_history_list(
    context: State<'_, ApiContextSlot>,
    request: PlaygroundHistoryListRequest,
) -> Result<PlaygroundHistoryPage, ApiError> {
    let context = context.snapshot()?;
    api::playground_history_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn playground_history_delete(
    context: State<'_, ApiContextSlot>,
    request: PlaygroundHistoryDeleteRequest,
) -> Result<PlaygroundHistoryDeleted, ApiError> {
    let context = context.snapshot()?;
    api::playground_history_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_catalog(context: State<'_, ApiContextSlot>) -> Result<SdCatalog, ApiError> {
    let context = context.snapshot()?;
    api::sd_catalog(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_runtime_releases(
    context: State<'_, ApiContextSlot>,
) -> Result<SdRuntimeReleases, ApiError> {
    let context = context.snapshot()?;
    api::sd_runtime_releases(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_runtime_install(
    context: State<'_, ApiContextSlot>,
    request: SdRuntimeInstallRequest,
) -> Result<JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::sd_runtime_install(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_runtime_inventory(
    context: State<'_, ApiContextSlot>,
) -> Result<SdRuntimeInventory, ApiError> {
    let context = context.snapshot()?;
    api::sd_runtime_inventory(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_runtime_switch(
    context: State<'_, ApiContextSlot>,
    request: SdRuntimeRef,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::sd_runtime_switch(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_runtime_delete(
    context: State<'_, ApiContextSlot>,
    request: SdRuntimeRef,
) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::sd_runtime_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_model_install(
    context: State<'_, ApiContextSlot>,
    request: SdModelInstallRequest,
) -> Result<SdModelInstallStarted, ApiError> {
    let context = context.snapshot()?;
    api::sd_model_install(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_models_installed(
    context: State<'_, ApiContextSlot>,
) -> Result<SdInstalledModels, ApiError> {
    let context = context.snapshot()?;
    api::sd_models_installed(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_model_uninstall(
    context: State<'_, ApiContextSlot>,
    request: SdModelUninstallRequest,
) -> Result<SdUninstallOutcome, ApiError> {
    let context = context.snapshot()?;
    api::sd_model_uninstall(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_model_repair(
    context: State<'_, ApiContextSlot>,
    request: SdModelRepairRequest,
) -> Result<SdModelRepaired, ApiError> {
    let context = context.snapshot()?;
    api::sd_model_repair(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_runnability(
    context: State<'_, ApiContextSlot>,
    request: SdRunnabilityRequest,
) -> Result<JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::sd_runnability(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_bundle_runnability(
    context: State<'_, ApiContextSlot>,
    request: SdBundleRunnabilityRequest,
) -> Result<JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::sd_bundle_runnability(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_compute_policy_get(
    context: State<'_, ApiContextSlot>,
    request: SdRuntimeRef,
) -> Result<SdComputePolicyInfo, ApiError> {
    let context = context.snapshot()?;
    api::sd_compute_policy_get(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_compute_policy_save(
    context: State<'_, ApiContextSlot>,
    request: SdComputePolicySaveRequest,
) -> Result<SdComputePolicyInfo, ApiError> {
    let context = context.snapshot()?;
    api::sd_compute_policy_save(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_detect_model_file(
    context: State<'_, ApiContextSlot>,
    request: SdDetectModelFileRequest,
) -> Result<SdDetectedModelFile, ApiError> {
    let context = context.snapshot()?;
    api::sd_detect_model_file(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_disk_usage(context: State<'_, ApiContextSlot>) -> Result<SdDiskUsage, ApiError> {
    let context = context.snapshot()?;
    api::sd_disk_usage(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_component_library(
    context: State<'_, ApiContextSlot>,
) -> Result<SdComponentLibrary, ApiError> {
    let context = context.snapshot()?;
    api::sd_component_library(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_upscalers_list(
    context: State<'_, ApiContextSlot>,
) -> Result<SdUpscalerInventory, ApiError> {
    let context = context.snapshot()?;
    api::sd_upscalers_list(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_upscalers_install(
    context: State<'_, ApiContextSlot>,
) -> Result<JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::sd_upscalers_install(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn sd_upscalers_remove(
    context: State<'_, ApiContextSlot>,
    request: SdUpscalerRemoveRequest,
) -> Result<SdUpscalerInventory, ApiError> {
    let context = context.snapshot()?;
    api::sd_upscalers_remove(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn loras_list(
    context: State<'_, ApiContextSlot>,
    request: LorasListRequest,
) -> Result<LoraList, ApiError> {
    let context = context.snapshot()?;
    api::loras_list(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn loras_import(
    context: State<'_, ApiContextSlot>,
    request: LorasImportRequest,
) -> Result<InstalledLora, ApiError> {
    let context = context.snapshot()?;
    api::loras_import(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn loras_delete(
    context: State<'_, ApiContextSlot>,
    request: LorasDeleteRequest,
) -> Result<LoraDeleted, ApiError> {
    let context = context.snapshot()?;
    api::loras_delete(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn loras_update_keywords(
    context: State<'_, ApiContextSlot>,
    request: LorasUpdateKeywordsRequest,
) -> Result<LoraKeywordDiscovery, ApiError> {
    let context = context.snapshot()?;
    api::loras_update_keywords(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn lora_keywords_discover(
    context: State<'_, ApiContextSlot>,
    request: LoraKeywordsDiscoverRequest,
) -> Result<JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::lora_keywords_discover(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_image_bundle_profiles(
    context: State<'_, ApiContextSlot>,
) -> Result<ImageBundleProfiles, ApiError> {
    let context = context.snapshot()?;
    api::hf_image_bundle_profiles(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_image_bundle_search(
    context: State<'_, ApiContextSlot>,
    request: HfImageBundleSearchRequest,
) -> Result<HfImageBundleSearchResults, ApiError> {
    let context = context.snapshot()?;
    api::hf_image_bundle_search(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_image_bundle_files(
    context: State<'_, ApiContextSlot>,
    request: HfImageBundleFilesRequest,
) -> Result<HfImageBundleFiles, ApiError> {
    let context = context.snapshot()?;
    api::hf_image_bundle_files(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_image_bundle_install(
    context: State<'_, ApiContextSlot>,
    request: HfImageBundleInstallRequest,
) -> Result<ImageBundleAccepted, ApiError> {
    let context = context.snapshot()?;
    api::hf_image_bundle_install(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_image_bundle_retry(
    context: State<'_, ApiContextSlot>,
    request: HfImageBundleRetryRequest,
) -> Result<ImageBundleRetried, ApiError> {
    let context = context.snapshot()?;
    api::hf_image_bundle_retry(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn hf_image_bundle_retry_registration(
    context: State<'_, ApiContextSlot>,
    request: HfImageBundleRetryRequest,
) -> Result<ImageBundleRegistered, ApiError> {
    let context = context.snapshot()?;
    api::hf_image_bundle_retry_registration(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn image_models_downloaded(
    context: State<'_, ApiContextSlot>,
) -> Result<DownloadedImageModels, ApiError> {
    let context = context.snapshot()?;
    api::image_models_downloaded(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn civitai_search(
    context: State<'_, ApiContextSlot>,
    request: CivitaiSearchRequest,
) -> Result<CivitaiSearchPage, ApiError> {
    let context = context.snapshot()?;
    api::civitai_search(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn civitai_model(
    context: State<'_, ApiContextSlot>,
    request: CivitaiModelRequest,
) -> Result<CivitaiModelDetail, ApiError> {
    let context = context.snapshot()?;
    api::civitai_model(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn civitai_auth_status(
    context: State<'_, ApiContextSlot>,
) -> Result<CivitaiAuthStatus, ApiError> {
    let context = context.snapshot()?;
    api::civitai_auth_status(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn civitai_auth_save(
    context: State<'_, ApiContextSlot>,
    request: CivitaiAuthSaveRequest,
) -> Result<CivitaiAuthStatus, ApiError> {
    let context = context.snapshot()?;
    api::civitai_auth_save(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn civitai_auth_clear(context: State<'_, ApiContextSlot>) -> Result<(), ApiError> {
    let context = context.snapshot()?;
    api::civitai_auth_clear(&context).await
}

#[tauri::command]
#[specta::specta]
pub async fn civitai_lora_download(
    context: State<'_, ApiContextSlot>,
    request: CivitaiLoraDownloadRequest,
) -> Result<JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::civitai_lora_download(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn avatar_prompt(
    context: State<'_, ApiContextSlot>,
    request: AvatarPromptRequest,
) -> Result<AvatarPrompt, ApiError> {
    let context = context.snapshot()?;
    api::avatar_prompt(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn avatar_gradient(
    context: State<'_, ApiContextSlot>,
    request: AvatarGradientRequest,
) -> Result<AvatarGradient, ApiError> {
    let context = context.snapshot()?;
    api::avatar_gradient(&context, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn image_design_reference(
    context: State<'_, ApiContextSlot>,
    request: ImageDesignReferenceRequest,
) -> Result<JobAccepted, ApiError> {
    let context = context.snapshot()?;
    api::image_design_reference(&context, request).await
}
