//! Local models: the llama.cpp runtime (devices, fit estimates, embedded
//! templates, unloading), the GGUF models folder (listing, deleting,
//! adopting a file, switching or moving the folder) and the runnability of a
//! model file.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode, ApiErrorDetails};
use lettuce_models::{
    LlamaCppSettings, LlamaGpuDistributionMode, LlamaKvPlacement, LlamaKvType, LlamaMtpPlacement,
    ModelProfileRepository, ProviderAccountRepository, ProviderProtocol,
};
use lettuce_settings::{DeviceSettingsStore, GlobalSettingsStore};
use lettuce_types::ModelProfileId;

use super::ApiContext;
use super::error::{api_error, invalid_field, parse_id};
use crate::{
    HuggingFaceBrowser, LocalModelSidecars, ModelFileReference, ModelFileReferences, ModelPathField,
};

type ResidentFiles = Arc<dyn Fn() -> Vec<String> + Send + Sync>;

/// The API's local model state: the Hugging Face browser (its avatar cache
/// lives as long as the process) and where the llama.cpp worker's open
/// files are read from.
pub(crate) struct LocalModelsState {
    hugging_face_endpoint: Mutex<String>,
    browser: Mutex<Option<Arc<HuggingFaceBrowser>>>,
    resident_files: Mutex<Option<ResidentFiles>>,
    folder_access: Mutex<()>,
}

impl Default for LocalModelsState {
    fn default() -> Self {
        Self {
            hugging_face_endpoint: Mutex::new(lettuce_model_hub::HUGGING_FACE_ENDPOINT.to_owned()),
            browser: Mutex::new(None),
            resident_files: Mutex::new(None),
            folder_access: Mutex::new(()),
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl LocalModelsState {
    pub(crate) fn folder_access(&self) -> MutexGuard<'_, ()> {
        lock(&self.folder_access)
    }

    /// The browser, built on first use with the device's trusted
    /// certificates.
    pub(crate) fn browser(
        &self,
        context: &ApiContext,
    ) -> Result<Arc<HuggingFaceBrowser>, ApiError> {
        let mut browser = lock(&self.browser);
        if let Some(browser) = browser.as_ref() {
            return Ok(Arc::clone(browser));
        }
        let tls = context
            .backend()
            .tls_policy()
            .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
        let client = lettuce_network::JsonClient::with_tls(&tls)
            .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?;
        let built = Arc::new(HuggingFaceBrowser::with_endpoint(
            client,
            lock(&self.hugging_face_endpoint).clone(),
        ));
        *browser = Some(Arc::clone(&built));
        Ok(built)
    }

    #[cfg(test)]
    pub(crate) fn use_hugging_face_endpoint(&self, endpoint: String) {
        *lock(&self.hugging_face_endpoint) = endpoint;
        *lock(&self.browser) = None;
    }

    #[cfg(test)]
    pub(crate) fn use_resident_files(
        &self,
        files: impl Fn() -> Vec<String> + Send + Sync + 'static,
    ) {
        *lock(&self.resident_files) = Some(Arc::new(files));
    }

    /// The model files the llama.cpp worker holds open.
    pub(crate) fn resident_files(&self, context: &ApiContext) -> Vec<String> {
        let resident = lock(&self.resident_files).clone();
        match resident {
            Some(files) => files(),
            None => backend_resident_files(context),
        }
    }
}

fn backend_resident_files(context: &ApiContext) -> Vec<String> {
    context.backend().local_llama_resident_files()
}

pub(crate) fn unsupported_on_mobile(what: &str) -> ApiError {
    api_error(
        ApiErrorCode::Unsupported,
        format!("{what} is only available on desktop"),
    )
}

/// A filesystem path the user picked; local model files are opened by path,
/// so a platform URI is refused.
pub(crate) fn local_path(source: &dto::FileSource, field: &str) -> Result<String, ApiError> {
    let path = source.uri.trim();
    if path.is_empty() {
        return Err(invalid_field(field, format!("{field} is empty")));
    }
    if path.contains("://") {
        return Err(invalid_field(
            field,
            format!("{field} must be a file on this device"),
        ));
    }
    Ok(path.to_owned())
}

fn optional_path(
    source: Option<&dto::FileSource>,
    field: &str,
) -> Result<Option<String>, ApiError> {
    source.map(|source| local_path(source, field)).transpose()
}

pub(crate) fn app_folder(context: &ApiContext) -> Result<&Path, ApiError> {
    context
        .app_folder()
        .ok_or_else(|| api_error(ApiErrorCode::Unavailable, "no app data folder is open"))
}

pub(crate) fn device_settings(
    context: &ApiContext,
) -> Result<lettuce_settings::DeviceSettings, ApiError> {
    context
        .backend()
        .database()
        .load_device_settings()
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))
}

/// The GGUF models folder in use.
pub(crate) fn models_root(context: &ApiContext) -> Result<PathBuf, ApiError> {
    Ok(crate::llm_models_root(
        &device_settings(context)?,
        app_folder(context)?,
    ))
}

/// The local runnability defaults the settings hold.
pub(crate) fn runnability_defaults(
    context: &ApiContext,
) -> Result<lettuce_model_hub::RunnabilityDefaults, ApiError> {
    let settings = GlobalSettingsStore::load(context.backend().database())
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
        .settings;
    Ok(lettuce_model_hub::RunnabilityDefaults::new(
        settings.llama_default_context_length.map(u64::from),
        settings
            .llama_default_kv_cache_type
            .map(lettuce_settings::LlamaDefaultKvCacheType::as_str)
            .filter(|kv| *kv != "auto"),
    ))
}

fn kv_type(value: Option<&String>, field: &str) -> Result<Option<String>, ApiError> {
    value
        .map(|name| {
            serde_json::from_value::<LlamaKvType>(serde_json::Value::String(name.trim().to_owned()))
                .map(|_| name.trim().to_owned())
                .map_err(|_| invalid_field(field, format!("{field} is not a llama.cpp KV type")))
        })
        .transpose()
}

fn kv_type_name(kv_type: LlamaKvType) -> Option<String> {
    serde_json::to_value(kv_type)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
}

const fn distribution_name(mode: LlamaGpuDistributionMode) -> &'static str {
    match mode {
        LlamaGpuDistributionMode::Balanced => "balanced",
        LlamaGpuDistributionMode::Proportional => "proportional",
        LlamaGpuDistributionMode::Priority => "priority",
        LlamaGpuDistributionMode::Manual => "manual",
    }
}

const fn kv_placement_name(placement: LlamaKvPlacement) -> &'static str {
    match placement {
        LlamaKvPlacement::Auto => "auto",
        LlamaKvPlacement::Split => "split",
        LlamaKvPlacement::SystemRam => "systemRam",
        LlamaKvPlacement::Pin => "pin",
    }
}

const fn draft_placement_name(placement: LlamaMtpPlacement) -> &'static str {
    match placement {
        LlamaMtpPlacement::Auto => "auto",
        LlamaMtpPlacement::Gpu => "gpu",
        LlamaMtpPlacement::Cpu => "cpu",
    }
}

const fn distribution(mode: dto::LlamaGpuDistribution) -> LlamaGpuDistributionMode {
    match mode {
        dto::LlamaGpuDistribution::Balanced => LlamaGpuDistributionMode::Balanced,
        dto::LlamaGpuDistribution::Proportional => LlamaGpuDistributionMode::Proportional,
        dto::LlamaGpuDistribution::Priority => LlamaGpuDistributionMode::Priority,
        dto::LlamaGpuDistribution::Manual => LlamaGpuDistributionMode::Manual,
    }
}

const fn kv_placement(placement: dto::LlamaKvPlacement) -> LlamaKvPlacement {
    match placement {
        dto::LlamaKvPlacement::Auto => LlamaKvPlacement::Auto,
        dto::LlamaKvPlacement::Split => LlamaKvPlacement::Split,
        dto::LlamaKvPlacement::SystemRam => LlamaKvPlacement::SystemRam,
        dto::LlamaKvPlacement::Pin => LlamaKvPlacement::Pin,
    }
}

const fn draft_placement(placement: dto::LlamaDraftPlacement) -> LlamaMtpPlacement {
    match placement {
        dto::LlamaDraftPlacement::Auto => LlamaMtpPlacement::Auto,
        dto::LlamaDraftPlacement::Gpu => LlamaMtpPlacement::Gpu,
        dto::LlamaDraftPlacement::Cpu => LlamaMtpPlacement::Cpu,
    }
}

/// A model file and the llama.cpp settings its estimate uses, as the
/// runtime reads them.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct LlamaFileSettings {
    pub model_path: String,
    pub settings: LlamaCppSettings,
}

/// The editor's draft as the settings a saved model would hold.
fn draft_settings(draft: &dto::LlamaSettingsDraft) -> Result<LlamaCppSettings, ApiError> {
    let parse = |value: Option<&String>, field: &str| -> Result<Option<LlamaKvType>, ApiError> {
        kv_type(value, field)?
            .map(|name| {
                serde_json::from_value(serde_json::Value::String(name))
                    .map_err(|_| invalid_field(field, "not a llama.cpp KV type"))
            })
            .transpose()
    };
    Ok(LlamaCppSettings {
        offload_kqv: draft.offload_kqv,
        kv_type: parse(draft.kv_type.as_ref(), "kv_type")?,
        kv_type_k: parse(draft.kv_type_k.as_ref(), "kv_type_k")?,
        kv_type_v: parse(draft.kv_type_v.as_ref(), "kv_type_v")?,
        gpu_layers: draft.gpu_layers,
        multi_gpu_enabled: draft.multi_gpu_enabled,
        gpu_device_ids: draft.gpu_device_ids.clone(),
        gpu_distribution_mode: draft.gpu_distribution.map(distribution),
        gpu_manual_layers: draft.gpu_manual_layers.as_ref().map(|layers| {
            layers
                .iter()
                .map(|assignment| lettuce_models::LlamaGpuLayerAssignment {
                    device_id: assignment.device_id,
                    layers: assignment.layers,
                })
                .collect()
        }),
        single_gpu_device_id: draft.single_gpu_device_id,
        kv_placement: draft.kv_placement.map(kv_placement),
        priority_vram_limit_bytes: draft.priority_vram_limit_bytes,
        mmproj_path: optional_path(draft.mmproj.as_ref(), "mmproj")?,
        mtp_enabled: draft.mtp_enabled,
        mtp_placement: draft.mtp_placement.map(draft_placement),
        mtp_model_path: optional_path(draft.mtp_model.as_ref(), "mtp_model")?,
        dflash_enabled: draft.dflash_enabled,
        dflash_model_path: optional_path(draft.dflash_model.as_ref(), "dflash_model")?,
        ..LlamaCppSettings::default()
    })
}

/// A saved llama.cpp model's file and settings.
pub(crate) fn saved_llama_model(
    context: &ApiContext,
    model_profile_id: &str,
) -> Result<LlamaFileSettings, ApiError> {
    let id: ModelProfileId = parse_id(model_profile_id, "model_profile_id")?;
    let database = context.backend().database();
    let profile = ModelProfileRepository::get(database, id)
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
        .ok_or_else(|| api_error(ApiErrorCode::NotFound, "the model was not found"))?;
    let account = ProviderAccountRepository::get(database, profile.provider_account_id)
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
    if account.is_none_or(|account| account.protocol != ProviderProtocol::LlamaCpp) {
        return Err(invalid_field(
            "model_profile_id",
            "the model is not a llama.cpp model",
        ));
    }
    Ok(LlamaFileSettings {
        model_path: profile.external_model_id,
        settings: profile.config.llama_cpp,
    })
}

async fn llama_target(
    context: &ApiContext,
    target: dto::LlamaModelTarget,
) -> Result<LlamaFileSettings, ApiError> {
    match target {
        dto::LlamaModelTarget::Draft { model, settings } => Ok(LlamaFileSettings {
            model_path: local_path(&model, "model")?,
            settings: draft_settings(&settings)?,
        }),
        dto::LlamaModelTarget::Saved { model_profile_id } => {
            context
                .blocking(move |context| saved_llama_model(context, &model_profile_id))
                .await
        }
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn context_info_request(
    file: LlamaFileSettings,
) -> lettuce_local_llm::context_info::ContextInfoRequest {
    let settings = file.settings;
    let device = |id: u32| usize::try_from(id).unwrap_or(usize::MAX);
    lettuce_local_llm::context_info::ContextInfoRequest {
        model_path: file.model_path,
        llama_offload_kqv: settings.offload_kqv,
        llama_kv_type: settings.kv_type.and_then(kv_type_name),
        llama_kv_type_k: settings.kv_type_k.and_then(kv_type_name),
        llama_kv_type_v: settings.kv_type_v.and_then(kv_type_name),
        llama_gpu_layers: settings.gpu_layers,
        llama_multi_gpu_enabled: settings.multi_gpu_enabled,
        llama_gpu_device_ids: settings
            .gpu_device_ids
            .map(|ids| ids.into_iter().map(device).collect()),
        llama_gpu_distribution_mode: settings
            .gpu_distribution_mode
            .map(|mode| distribution_name(mode).to_owned()),
        llama_gpu_manual_layers: settings.gpu_manual_layers.map(|layers| {
            layers
                .into_iter()
                .map(
                    |assignment| lettuce_local_llm::context_info::GpuLayerAssignment {
                        device_id: device(assignment.device_id),
                        layers: assignment.layers,
                    },
                )
                .collect()
        }),
        llama_single_gpu_device_id: settings.single_gpu_device_id.map(device),
        llama_kv_placement: settings
            .kv_placement
            .map(|placement| kv_placement_name(placement).to_owned()),
        llama_priority_vram_limit_bytes: settings.priority_vram_limit_bytes,
        llama_mmproj_path: settings.mmproj_path,
        llama_mtp_enabled: settings.mtp_enabled,
        llama_mtp_placement: settings
            .mtp_placement
            .map(|placement| draft_placement_name(placement).to_owned()),
        llama_mtp_model_path: settings.mtp_model_path,
        llama_dflash_enabled: settings.dflash_enabled,
        llama_dflash_model_path: settings.dflash_model_path,
    }
}

fn to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// The GPUs llama.cpp can offload to; none on mobile.
pub async fn llama_devices(context: &ApiContext) -> Result<dto::LlamaDeviceList, ApiError> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        context
            .blocking(|context| {
                Ok(dto::LlamaDeviceList {
                    devices: context
                        .backend()
                        .llama_backend_devices()
                        .into_iter()
                        .map(|device| dto::LlamaDevice {
                            index: to_u32(device.index),
                            name: device.name,
                            description: device.description,
                            backend: device.backend,
                            memory_total: device.memory_total,
                            memory_free: device.memory_free,
                            device_type: device.device_type,
                        })
                        .collect(),
                })
            })
            .await
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = context;
        Ok(dto::LlamaDeviceList {
            devices: Vec::new(),
        })
    }
}

/// How a model fits this machine with the editor's (unsaved) or a saved
/// model's settings.
pub async fn llama_context_info(
    context: &ApiContext,
    request: dto::LlamaContextInfoRequest,
) -> Result<dto::LlamaContextInfo, ApiError> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        use lettuce_local_llm::context_info::ContextInfoError;
        let file = llama_target(context, request.target).await?;
        let info = context
            .backend()
            .llama_context_info(context_info_request(file))
            .await
            .map_err(|error| match error {
                crate::LocalLlamaCommandError::ContextInfo(ContextInfoError::EmptyPath) => {
                    invalid_field("model", error.to_string())
                }
                crate::LocalLlamaCommandError::ContextInfo(ContextInfoError::NotFound(_)) => {
                    api_error(ApiErrorCode::NotFound, error.to_string())
                }
                error => api_error(ApiErrorCode::Unavailable, error.to_string()),
            })?;
        Ok(dto::LlamaContextInfo {
            max_context_length: info.max_context_length,
            recommended_context_length: info.recommended_context_length,
            available_memory_bytes: info.available_memory_bytes,
            available_vram_bytes: info.available_vram_bytes,
            model_size_bytes: info.model_size_bytes,
            layer_count: info.layer_count,
            max_gpu_layers: info.max_gpu_layers,
            supports_gpu_offload: info.supports_gpu_offload,
            selected_gpu_device_ids: info
                .selected_gpu_device_ids
                .map(|ids| ids.into_iter().map(to_u32).collect()),
            per_device_vram: info.per_device_vram.map(|devices| {
                devices
                    .into_iter()
                    .map(|device| dto::LlamaDeviceMemory {
                        index: to_u32(device.index),
                        memory_free: device.memory_free,
                        memory_total: device.memory_total,
                    })
                    .collect()
            }),
            estimated_placement: info
                .estimated_placement
                .map(|placement| dto::LlamaPlacement {
                    total_gpu_layers: placement.total_gpu_layers,
                    per_device_layers: placement.per_device_layers,
                }),
        })
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = (context, request);
        Err(unsupported_on_mobile("llama.cpp"))
    }
}

/// The chat template a model file carries.
pub async fn llama_chat_template(
    context: &ApiContext,
    request: dto::LlamaChatTemplateRequest,
) -> Result<dto::LlamaChatTemplate, ApiError> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        use lettuce_local_llm::llama::EmbeddedTemplateError;
        let path = match request.target {
            dto::LlamaModelFile::File { model } => local_path(&model, "model")?,
            dto::LlamaModelFile::Saved { model_profile_id } => {
                context
                    .blocking(move |context| saved_llama_model(context, &model_profile_id))
                    .await?
                    .model_path
            }
        };
        let template = context
            .backend()
            .llama_embedded_chat_template(path)
            .await
            .map_err(|error| match error {
                crate::LocalLlamaCommandError::EmbeddedTemplate(
                    EmbeddedTemplateError::EmptyPath,
                ) => invalid_field("model", error.to_string()),
                crate::LocalLlamaCommandError::EmbeddedTemplate(
                    EmbeddedTemplateError::NotFound(_) | EmbeddedTemplateError::Missing(_),
                ) => api_error(ApiErrorCode::NotFound, error.to_string()),
                error => api_error(ApiErrorCode::Unavailable, error.to_string()),
            })?;
        Ok(dto::LlamaChatTemplate { template })
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = (context, request);
        Err(unsupported_on_mobile("llama.cpp"))
    }
}

/// Frees the loaded model and its cached contexts, after any running
/// generation.
pub async fn llama_unload(context: &ApiContext) -> Result<(), ApiError> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        context
            .backend()
            .unload_local_llama()
            .await
            .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = context;
        Err(unsupported_on_mobile("llama.cpp"))
    }
}

/// Unloads llama.cpp when it holds `path` open; returns whether it did.
pub(crate) async fn unload_if_resident(context: &ApiContext, path: &str) -> Result<bool, ApiError> {
    let resident = context.local_models().resident_files(context);
    let target = Path::new(path);
    if !resident
        .iter()
        .any(|file| crate::models::gguf_library::paths_equal(Path::new(file), target))
    {
        return Ok(false);
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        context
            .backend()
            .unload_local_llama()
            .await
            .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
    }
    Ok(true)
}

const fn path_field(field: ModelPathField) -> dto::LocalModelPathField {
    match field {
        ModelPathField::Model => dto::LocalModelPathField::Model,
        ModelPathField::Mmproj => dto::LocalModelPathField::Mmproj,
        ModelPathField::Mtp => dto::LocalModelPathField::Mtp,
        ModelPathField::Dflash => dto::LocalModelPathField::Dflash,
    }
}

fn reference(reference: ModelFileReference) -> dto::LocalModelReference {
    dto::LocalModelReference {
        model_profile_id: reference.model_profile_id.map(|id| id.to_string()),
        display_name: reference.display_name,
        fields: reference.fields.into_iter().map(path_field).collect(),
    }
}

/// The GGUF files in the models folder (nested folders included), with the
/// models that use each one.
pub async fn local_models_list(context: &ApiContext) -> Result<dto::LocalModelList, ApiError> {
    context
        .blocking(|context| {
            let device = device_settings(context)?;
            let folder = app_folder(context)?;
            let root = crate::llm_models_root(&device, folder);
            let skipped = crate::image_model_roots(&device, folder);
            let files = crate::downloaded_ggufs(&root, &skipped)
                .map_err(|error| api_error(ApiErrorCode::Unavailable, error))?;
            let references = ModelFileReferences::load(context.backend().database())
                .map_err(|error| api_error(ApiErrorCode::Internal, error))?;
            Ok(dto::LocalModelList {
                files: files
                    .into_iter()
                    .map(|file| dto::LocalModelFile {
                        used_by: references
                            .of(&file.path)
                            .into_iter()
                            .map(reference)
                            .collect(),
                        repo: file.model_id,
                        filename: file.filename,
                        path: file.path,
                        size: file.size,
                        quantization: file.quantization,
                        is_mmproj: file.is_mmproj,
                        is_mtp: file.is_mtp,
                        is_dflash: file.is_dflash,
                        architecture: file.architecture,
                        context_length: file.context_length,
                    })
                    .collect(),
            })
        })
        .await
}

fn folders(context: &ApiContext) -> Result<(PathBuf, Vec<PathBuf>), ApiError> {
    let device = device_settings(context)?;
    let folder = app_folder(context)?;
    Ok((
        crate::llm_models_root(&device, folder),
        crate::image_model_roots(&device, folder),
    ))
}

/// Deletes a model file from the models folder, unloading llama.cpp first
/// when it holds the file; the models still pointing at it are returned
/// for the UI to fix.
pub async fn local_model_delete(
    context: &ApiContext,
    request: dto::LocalModelDeleteRequest,
) -> Result<dto::LocalModelDeleted, ApiError> {
    let path = request.path.trim().to_owned();
    if path.is_empty() {
        return Err(invalid_field("path", "path is empty"));
    }
    let checked = path.clone();
    let exists = context
        .blocking(move |context| {
            let (root, image_roots) = folders(context)?;
            crate::deletable_model(&root, &image_roots, &checked)
                .map_err(|error| delete_error(&error))
        })
        .await?;
    let unloaded = exists && unload_if_resident(context, &path).await?;
    context
        .blocking(move |context| {
            let (root, image_roots) = folders(context)?;
            crate::delete_downloaded_model(&root, &image_roots, &path)
                .map_err(|error| delete_error(&error))?;
            let references = crate::model_file_references(context.backend().database(), &path)
                .map_err(|error| api_error(ApiErrorCode::Internal, error))?;
            Ok(dto::LocalModelDeleted {
                unloaded,
                referencing_profiles: references.into_iter().map(reference).collect(),
            })
        })
        .await
}

fn delete_error(error: &str) -> ApiError {
    if error == crate::OUTSIDE_MODELS_FOLDER {
        invalid_field("path", error)
    } else {
        api_error(ApiErrorCode::Unavailable, error)
    }
}

/// Moves a model file into the models folder (unloading llama.cpp first
/// when it holds the file) and returns where it now is.
pub async fn local_model_adopt(
    context: &ApiContext,
    request: dto::LocalModelAdoptRequest,
) -> Result<dto::LocalModelAdopted, ApiError> {
    let source = local_path(&request.source, "source")?;
    unload_if_resident(context, &source).await?;
    let model_name = request.model_name;
    context
        .blocking(move |context| {
            let root = models_root(context)?;
            crate::move_model_into_library(&root, &source, model_name.as_deref())
                .map(|path| dto::LocalModelAdopted { path })
                .map_err(|error| {
                    if error.starts_with("Source file does not exist") {
                        api_error(ApiErrorCode::NotFound, error)
                    } else if error.starts_with("Source path is not a file") {
                        invalid_field("source", error)
                    } else if error.starts_with("The model name cannot be used") {
                        invalid_field("model_name", error)
                    } else if error.starts_with("A different file") {
                        api_error(ApiErrorCode::Conflict, error)
                    } else {
                        api_error(ApiErrorCode::Unavailable, error)
                    }
                })
        })
        .await
}

pub async fn local_models_dir_get(context: &ApiContext) -> Result<dto::LocalModelsDir, ApiError> {
    context
        .blocking(|context| {
            let info = crate::llm_models_dir_info(&device_settings(context)?, app_folder(context)?)
                .map_err(|error| api_error(ApiErrorCode::Unavailable, error))?;
            Ok(dto::LocalModelsDir {
                path: info.path.to_string_lossy().into_owned(),
                default_path: info.default_path.to_string_lossy().into_owned(),
                is_custom: info.is_custom,
                model_count: info.model_count,
            })
        })
        .await
}

pub(crate) fn busy(reason: dto::LocalModelsBusyReason) -> ApiError {
    ApiError {
        code: ApiErrorCode::Busy,
        message: "the local models folder is in use".to_owned(),
        details: Some(ApiErrorDetails::LocalModelsBusy { reason }),
    }
}

/// The local image job (generation, upscale, probe or LoRA discovery) that
/// has not ended, or `Some(None)` when a local call runs without a job.
fn image_work_active(context: &ApiContext) -> Option<Option<String>> {
    use lettuce_jobs::{JobCatalog, JobKind, JobListFilter, JobState, ResourceClass};
    use lettuce_types::{PageLimit, PageRequest};
    const ACTIVE: [JobState; 5] = [
        JobState::Queued,
        JobState::Claimed,
        JobState::Running,
        JobState::CancellationRequested,
        JobState::CleaningUp,
    ];
    let database = context.backend().database();
    for kind in [
        JobKind::ImageGenerate,
        JobKind::MediaTransform,
        JobKind::RuntimePrepare,
        JobKind::Maintenance,
    ] {
        let mut cursor = None;
        loop {
            let page = database.list_jobs(&JobListFilter {
                kinds: vec![kind],
                states: ACTIVE.to_vec(),
                subject: None,
                page: PageRequest {
                    cursor: cursor.take(),
                    limit: PageLimit::new(200),
                },
            });
            let Ok(page) = page else {
                return Some(None);
            };
            if let Some(job) = page.items.iter().find(|job| {
                if kind == JobKind::Maintenance {
                    super::jobs::is_lora_discovery(job)
                } else {
                    job.resources.contains(&ResourceClass::Process)
                }
            }) {
                return Some(Some(job.id.to_string()));
            }
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    if context
        .backend()
        .local_diffusion()
        .is_some_and(|engine| engine.call_active())
    {
        return Some(None);
    }
    None
}

/// Why moving the folder at `root` would break work in progress: an install
/// writing below it, a model llama.cpp holds open from it, or local image
/// work that reads the image models below it.
pub(crate) fn folder_busy(context: &ApiContext, root: &Path) -> Result<Option<dto::LocalModelsBusyReason>, ApiError> {
    if let Some(job_id) = image_work_active(context) {
        return Ok(Some(dto::LocalModelsBusyReason::ImageWorkActive { job_id }));
    }
    if let Some((job_id, _)) = context
        .jobs()
        .install_roots()
        .into_iter()
        .find(|(_, install_root)| install_root.starts_with(root))
    {
        return Ok(Some(dto::LocalModelsBusyReason::InstallActive {
            job_id: job_id.to_string(),
        }));
    }
    if let Some(path) = context.backend().whisper_runtime().resident_files()
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
        .into_iter().find(|path| path.starts_with(root)) {
        return Ok(Some(dto::LocalModelsBusyReason::ModelLoaded { path: path.to_string_lossy().into_owned() }));
    }
    {
        let roots = context.retained_model_roots_for_guard()?.unwrap_or_default();
        for (kind, path) in [(dto::RequiredModel::Embedding, roots.embedding), (dto::RequiredModel::Emotion, roots.thymos)] {
            if context.models().is_loaded(kind)
                && let Some(path) = path
                && Path::new(&path).starts_with(root) {
                return Ok(Some(dto::LocalModelsBusyReason::ModelLoaded { path }));
            }
        }
    }
    if let Some((job_id, path)) = super::jobs::speech::active_local_files(context, root)? {
        if Path::new(&path).starts_with(root) {
            return Ok(Some(dto::LocalModelsBusyReason::SpeechWorkActive { job_id: job_id.to_string() }));
        }
    }
    Ok(context
        .local_models()
        .resident_files(context)
        .into_iter()
        .find(|file| Path::new(file).starts_with(root))
        .map(|path| dto::LocalModelsBusyReason::ModelLoaded { path }))
}

/// Uses another models folder as a job: with `move_existing` the current
/// folder's files move there and model paths follow, refused while an
/// install writes into the current folder or llama.cpp holds a model from
/// it.
pub async fn local_models_dir_set(
    context: &ApiContext,
    request: dto::LocalModelsDirSetRequest,
) -> Result<dto::JobAccepted, ApiError> {
    if cfg!(any(target_os = "android", target_os = "ios")) {
        return Err(unsupported_on_mobile("Moving the models folder"));
    }
    super::jobs::admit_models_folder_move(context, request).await
}

fn sidecar_path(source: Option<&dto::FileSource>, field: &str) -> Result<Option<String>, ApiError> {
    optional_path(source, field)
}

/// How well a model file runs here with the editor's sidecars, or a saved
/// model with its own.
pub async fn local_file_runnability(
    context: &ApiContext,
    request: dto::LocalFileRunnabilityRequest,
) -> Result<dto::LocalFileRunnability, ApiError> {
    let file = match request.target {
        dto::LocalRunnabilityTarget::Draft { model, sidecars } => LlamaFileSettings {
            model_path: local_path(&model, "model")?,
            settings: LlamaCppSettings {
                mmproj_path: sidecar_path(sidecars.mmproj.as_ref(), "mmproj")?,
                mtp_enabled: Some(sidecars.mtp_enabled),
                mtp_placement: sidecars.mtp_placement.map(draft_placement),
                mtp_model_path: sidecar_path(sidecars.mtp_model.as_ref(), "mtp_model")?,
                dflash_enabled: Some(sidecars.dflash_enabled),
                dflash_model_path: sidecar_path(sidecars.dflash_model.as_ref(), "dflash_model")?,
                ..LlamaCppSettings::default()
            },
        },
        dto::LocalRunnabilityTarget::Saved { model_profile_id } => {
            context
                .blocking(move |context| saved_llama_model(context, &model_profile_id))
                .await?
        }
    };
    context
        .blocking(move |context| {
            let defaults = runnability_defaults(context)?;
            let settings = &file.settings;
            let sidecars = LocalModelSidecars {
                mmproj_path: settings.mmproj_path.as_deref(),
                mtp_enabled: settings.mtp_enabled.unwrap_or(false),
                mtp_on_cpu: settings.mtp_placement == Some(LlamaMtpPlacement::Cpu),
                mtp_model_path: settings.mtp_model_path.as_deref(),
                dflash_enabled: settings.dflash_enabled.unwrap_or(false),
                dflash_model_path: settings.dflash_model_path.as_deref(),
            };
            let (runnability, metadata_available) = crate::local_file_runnability(
                &file.model_path,
                &sidecars,
                crate::local_runnability_hardware(),
                defaults,
            )
            .map_err(|error| {
                if error == "File does not exist" {
                    api_error(ApiErrorCode::NotFound, error)
                } else {
                    api_error(ApiErrorCode::Unavailable, error)
                }
            })?;
            let configuration = runnability.configuration;
            Ok(dto::LocalFileRunnability {
                score: configuration.score,
                label: runnability_label(runnability.label),
                fits_in_ram: configuration.fits_in_ram,
                fits_in_vram: configuration.fits_in_vram,
                memory_score: configuration.memory_score,
                gpu_score: configuration.gpu_score,
                kv_score: configuration.kv_score,
                gpu_mode: gpu_mode(configuration.gpu_mode),
                quant_score: runnability.quant_score,
                available_ram: runnability.available_ram,
                available_vram: runnability.available_vram,
                model_size: runnability.model_size,
                quantization: runnability.quantization,
                metadata_available,
            })
        })
        .await
}

pub(crate) const fn runnability_label(
    label: lettuce_model_hub::RunnabilityLabel,
) -> dto::RunnabilityLabel {
    use lettuce_model_hub::RunnabilityLabel as Label;
    match label {
        Label::Excellent => dto::RunnabilityLabel::Excellent,
        Label::Good => dto::RunnabilityLabel::Good,
        Label::Marginal => dto::RunnabilityLabel::Marginal,
        Label::Poor => dto::RunnabilityLabel::Poor,
        Label::Unrunnable => dto::RunnabilityLabel::Unrunnable,
    }
}

pub(crate) const fn gpu_mode(mode: lettuce_model_hub::GpuMode) -> dto::GpuModeDto {
    use lettuce_model_hub::GpuMode;
    match mode {
        GpuMode::Full => dto::GpuModeDto::Full,
        GpuMode::NearFull => dto::GpuModeDto::NearFull,
        GpuMode::KvSpill => dto::GpuModeDto::KvSpill,
        GpuMode::KvHeavySpill => dto::GpuModeDto::KvHeavySpill,
        GpuMode::RamModelVramCtx => dto::GpuModeDto::RamModelVramCtx,
        GpuMode::RamModelRamCtx => dto::GpuModeDto::RamModelRamCtx,
        GpuMode::MostLayers => dto::GpuModeDto::MostLayers,
        GpuMode::HalfLayers => dto::GpuModeDto::HalfLayers,
        GpuMode::FewLayers => dto::GpuModeDto::FewLayers,
        GpuMode::Cpu => dto::GpuModeDto::Cpu,
    }
}
