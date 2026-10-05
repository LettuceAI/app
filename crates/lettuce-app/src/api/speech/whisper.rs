use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode, ApiErrorDetails};
use lettuce_model_hub::{InstalledWhisperManifest, RemoteWhisperModel};
use lettuce_settings::DeviceSettingsStore;
use lettuce_speech::{AsrModelDescriptor, TranscriptionOptions};

use super::errors::whisper_required;
use crate::WhisperCatalogError;
use crate::api::ApiContext;
use crate::api::error::{IntoApiError, api_error, invalid_field};
use crate::api::jobs::{InstallWork, admit_install};

fn unavailable(error: impl std::fmt::Display) -> ApiError {
    api_error(ApiErrorCode::Unavailable, error.to_string())
}

pub(crate) fn whisper_root(context: &ApiContext) -> Result<PathBuf, ApiError> {
    context
        .retained_model_roots()?
        .whisper
        .map(PathBuf::from)
        .ok_or_else(|| api_error(ApiErrorCode::Unavailable, "the Whisper root is unavailable"))
}

fn filename(model_id: &str) -> String {
    format!("ggml-{model_id}.bin")
}

/// Admits the Whisper models an earlier version of the app left in the
/// models folder, once per process; a model already admitted is left as it
/// is.
pub(super) fn ensure_legacy_admitted(context: &ApiContext) -> Result<(), ApiError> {
    let state = context.speech_state();
    if state.legacy_whisper_admitted().load(Ordering::Acquire) {
        return Ok(());
    }
    let _admission = state.legacy_whisper_admission();
    if state.legacy_whisper_admitted().load(Ordering::Acquire) {
        return Ok(());
    }
    let root = whisper_root(context)?;
    if root.is_dir() {
        let database = context.backend().database();
        let coordinator = context.backend().whisper_models();
        let known = coordinator
            .list()
            .map_err(IntoApiError::into_api_error)?
            .into_iter()
            .map(|manifest| manifest.model_id)
            .collect::<HashSet<_>>();
        let found = lettuce_model_hub::inspect_legacy_whisper_models(&root, context.now())
            .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
        for manifest in found
            .into_iter()
            .filter(|manifest| !known.contains(&manifest.model_id))
        {
            lettuce_model_hub::WhisperModelRepository::admit_whisper_model(database, manifest)
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
        }
    }
    state
        .legacy_whisper_admitted()
        .store(true, Ordering::Release);
    Ok(())
}

pub(super) fn installed(context: &ApiContext) -> Result<Vec<InstalledWhisperManifest>, ApiError> {
    ensure_legacy_admitted(context)?;
    let mut models = context
        .backend()
        .whisper_models()
        .list()
        .map_err(IntoApiError::into_api_error)?;
    models.sort_by_key(|model| filename(&model.model_id));
    Ok(models)
}

fn dictation_model_id(context: &ApiContext) -> Result<Option<String>, ApiError> {
    Ok(context
        .backend()
        .database()
        .load_device_settings()
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
        .speech
        .dictation_model_id)
}

/// The model a transcription runs with: the requested one, else the
/// dictation choice, else the first installed.
pub(crate) fn resolve_model(
    context: &ApiContext,
    requested: Option<&str>,
) -> Result<AsrModelDescriptor, ApiError> {
    ensure_legacy_admitted(context)?;
    let coordinator = context.backend().whisper_models();
    if let Some(model_id) = requested {
        return coordinator
            .resolve(Some(model_id))
            .map(|(descriptor, _)| descriptor)
            .map_err(IntoApiError::into_api_error);
    }
    if let Some(model_id) = dictation_model_id(context)?
        && let Ok((descriptor, _)) = coordinator.resolve(Some(&model_id))
    {
        return Ok(descriptor);
    }
    coordinator
        .resolve(None)
        .map(|(descriptor, _)| descriptor)
        .map_err(IntoApiError::into_api_error)
}

pub(super) fn engine_options(options: &dto::TranscribeOptions) -> TranscriptionOptions {
    TranscriptionOptions {
        language: options.language.clone(),
        scopes: options.scopes.clone(),
        initial_prompt: options.initial_prompt.clone(),
        translate: options.translate,
        detect_language: options.detect_language,
        threads: options.threads.map(|threads| threads as usize),
        use_gpu: options.run.use_gpu,
        force_cpu: options.run.force_cpu,
        flash_attention: options.run.flash_attention,
        gpu_device: options.run.gpu_device,
        keep_model_loaded: options.keep_model_loaded,
        ..TranscriptionOptions::default()
    }
}

fn catalog_error(error: WhisperCatalogError) -> ApiError {
    match error {
        WhisperCatalogError::Network(_) => ApiError {
            code: ApiErrorCode::Unavailable,
            message: error.to_string(),
            details: Some(ApiErrorDetails::HuggingFace {
                failure: dto::HfFailure::Offline,
            }),
        },
        error => unavailable(error),
    }
}

async fn remote_models(context: &ApiContext) -> Result<Vec<RemoteWhisperModel>, ApiError> {
    context
        .backend()
        .whisper_remote_catalog()
        .map_err(unavailable)?
        .list()
        .await
        .map_err(catalog_error)
}

/// The Whisper models Hugging Face offers, with which are installed.
pub async fn whisper_catalog(context: &ApiContext) -> Result<dto::WhisperCatalog, ApiError> {
    let remote = remote_models(context).await?;
    let installed = context
        .blocking(|context| {
            Ok(installed(context)?
                .into_iter()
                .map(|manifest| manifest.model_id)
                .collect::<HashSet<_>>())
        })
        .await?;
    Ok(dto::WhisperCatalog {
        models: remote
            .into_iter()
            .map(|model| dto::WhisperCatalogModel {
                installed: installed.contains(&model.model_id),
                id: model.model_id,
                filename: model.filename,
                size_bytes: model.byte_size,
                english_only: model.english_only,
                quantized: model.quantized,
                recommended: model.recommended,
                recommended_for_mobile: model.recommended_for_mobile,
                recommended_for_desktop: model.recommended_for_desktop,
            })
            .collect(),
    })
}

/// The installed Whisper models, in file name order; the first one is what
/// dictation uses until a choice is stored.
pub async fn whisper_models_list(
    context: &ApiContext,
) -> Result<dto::WhisperInstalledModels, ApiError> {
    context
        .blocking(|context| {
            let chosen = dictation_model_id(context)?;
            let models = installed(context)?;
            let dictation = chosen
                .filter(|chosen| models.iter().any(|model| &model.model_id == chosen))
                .or_else(|| models.first().map(|model| model.model_id.clone()));
            Ok(dto::WhisperInstalledModels {
                models: models
                    .into_iter()
                    .map(|manifest| dto::WhisperInstalledModel {
                        dictation: dictation.as_deref() == Some(manifest.model_id.as_str()),
                        filename: filename(&manifest.model_id),
                        size_bytes: manifest.model.byte_size,
                        english_only: manifest.english_only,
                        quantized: manifest.quantized,
                        id: manifest.model_id,
                    })
                    .collect(),
            })
        })
        .await
}

/// Downloads a catalog model as a job. A download of a model already
/// queued or installed joins its job.
pub async fn whisper_download(
    context: &ApiContext,
    request: dto::WhisperModelRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let model = remote_models(context)
        .await?
        .into_iter()
        .find(|model| model.model_id == request.model_id)
        .ok_or_else(|| {
            api_error(
                ApiErrorCode::NotFound,
                "the Whisper catalog has no such model",
            )
        })?;
    let install_root = context
        .blocking(|context| {
            let root = whisper_root(context)?;
            std::fs::create_dir_all(&root).map_err(unavailable)?;
            Ok(root)
        })
        .await?;
    admit_install(
        context,
        InstallWork::Whisper {
            model,
            install_root,
        },
    )
    .await
}

/// Removes an installed model and clears every loaded Whisper context; a
/// model that is not installed is left alone.
pub async fn whisper_delete(
    context: &ApiContext,
    request: dto::WhisperModelRequest,
) -> Result<(), ApiError> {
    if request.model_id.trim().is_empty() {
        return Err(invalid_field("model_id", "model_id is empty"));
    }
    context
        .blocking(move |context| {
            let root = whisper_root(context)?;
            std::fs::create_dir_all(&root).map_err(unavailable)?;
            ensure_legacy_admitted(context)?;
            context
                .backend()
                .remove_managed_whisper_model(&root, &request.model_id)
                .map_err(IntoApiError::into_api_error)?;
            let database = context.backend().database();
            let mut device = database
                .load_device_settings()
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
            if device.speech.dictation_model_id.as_deref() == Some(request.model_id.as_str()) {
                device.speech.dictation_model_id = None;
                database
                    .save_device_settings(device)
                    .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
            }
            Ok(())
        })
        .await
}

/// Loads a model into memory so the first transcription does not wait for
/// it.
pub async fn whisper_preload(
    context: &ApiContext,
    request: dto::WhisperPreloadRequest,
) -> Result<(), ApiError> {
    context
        .blocking(move |context| {
            let _folder_access = context.local_models().folder_access();
            let root = whisper_root(context)?;
            if root.starts_with(crate::api::local_models::models_root(context)?) {
                crate::api::jobs::local::folder_move_active(context)?;
            }
            let model = resolve_model(context, request.model_id.as_deref())?;
            let options = engine_options(&dto::TranscribeOptions {
                run: request.run,
                ..dto::TranscribeOptions::default()
            });
            context
                .backend()
                .whisper_runtime()
                .preload(&model, &options)
                .map_err(|error| match error {
                    lettuce_speech::AsrRuntimeError::ModelUnavailable => whisper_required(),
                    error => api_error(ApiErrorCode::Unavailable, error.to_string()),
                })
        })
        .await
}

/// Drops every loaded Whisper context and says how many there were.
pub async fn whisper_clear_cache(
    context: &ApiContext,
) -> Result<dto::WhisperCacheCleared, ApiError> {
    context
        .blocking(|context| {
            let cleared = context
                .backend()
                .whisper_runtime()
                .clear_cache()
                .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?;
            Ok(dto::WhisperCacheCleared {
                cleared: u32::try_from(cleared).unwrap_or(u32::MAX),
            })
        })
        .await
}

/// Chooses the model dictation uses; `None` goes back to the first
/// installed one.
pub async fn whisper_dictation_model_set(
    context: &ApiContext,
    request: dto::DictationModelSetRequest,
) -> Result<(), ApiError> {
    context
        .blocking(move |context| {
            if let Some(model_id) = &request.model_id {
                let known = installed(context)?
                    .iter()
                    .any(|manifest| &manifest.model_id == model_id);
                if !known {
                    return Err(whisper_required());
                }
            }
            let database = context.backend().database();
            let mut device = database
                .load_device_settings()
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
            device.speech.dictation_model_id = request.model_id;
            database
                .save_device_settings(device)
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))
        })
        .await
}
