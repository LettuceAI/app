//! The LoRA library, Hugging Face image bundles, the image model files on
//! disk and CivitAI.

use std::path::PathBuf;

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_image_generation::sd_runtime::lora_library::{
    InstalledLora, LoraArchitectureSource, LoraCompatibility, LoraKeywordDiscovery,
    LoraKeywordSource,
};
use lettuce_image_generation::{BundleAsset, CivitaiSearch};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use lettuce_image_generation::{CivitaiLoraDownload, bundle_root};
use lettuce_settings::GlobalSettingsStore;

use super::engine::{role_dto, role_of};
use super::generate::lora;
#[cfg(any(target_os = "android", target_os = "ios"))]
use super::unsupported;
use super::{ApiContext, engine, image_error, lora_library};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::HfBundleInstallRequest;
use crate::api::error::{api_error, hf_error, invalid_field};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::api::jobs::admit_install_with_detail;
use crate::api::jobs::{ImageToolDetail, admit_tool};
use crate::api::local_models::local_path;
use crate::{HfBundleRoleSearch, HuggingFaceBrowser};

pub(crate) fn to_engine_lora(value: &dto::ImageLora) -> lettuce_models::StableDiffusionLora {
    lora(value)
}

fn keyword_source(source: LoraKeywordSource) -> dto::LoraKeywordSource {
    match source {
        LoraKeywordSource::None => dto::LoraKeywordSource::None,
        LoraKeywordSource::Metadata => dto::LoraKeywordSource::Metadata,
        LoraKeywordSource::Civitai => dto::LoraKeywordSource::Civitai,
        LoraKeywordSource::Manual => dto::LoraKeywordSource::Manual,
    }
}

fn architecture_source(source: LoraArchitectureSource) -> dto::LoraArchitectureSource {
    match source {
        LoraArchitectureSource::None => dto::LoraArchitectureSource::None,
        LoraArchitectureSource::Metadata => dto::LoraArchitectureSource::Metadata,
        LoraArchitectureSource::Civitai => dto::LoraArchitectureSource::Civitai,
    }
}

fn compatibility(value: LoraCompatibility) -> dto::LoraCompatibility {
    match value {
        LoraCompatibility::Compatible => dto::LoraCompatibility::Compatible,
        LoraCompatibility::Incompatible => dto::LoraCompatibility::Incompatible,
        LoraCompatibility::Unknown => dto::LoraCompatibility::Unknown,
    }
}

pub(crate) fn lora_discovery(discovery: LoraKeywordDiscovery) -> dto::LoraKeywordDiscovery {
    dto::LoraKeywordDiscovery {
        keywords: discovery.keywords,
        source: keyword_source(discovery.source),
        sha256: discovery.sha256,
        architecture: discovery.architecture,
        architecture_source: architecture_source(discovery.architecture_source),
        compatibility: compatibility(discovery.compatibility),
    }
}

fn installed_lora(lora: InstalledLora) -> dto::InstalledLora {
    dto::InstalledLora {
        filename: lora.filename,
        path: lora.path,
        bytes_on_disk: lora.bytes_on_disk,
        keywords: lora.keywords,
        keyword_source: keyword_source(lora.keyword_source),
        architecture: lora.architecture,
        architecture_source: architecture_source(lora.architecture_source),
        compatibility: compatibility(lora.compatibility),
    }
}

/// The LoRAs in the library, with their compatibility with `profile_id`.
pub async fn loras_list(
    context: &ApiContext,
    request: dto::LorasListRequest,
) -> Result<dto::LoraList, ApiError> {
    engine(context)?;
    context
        .blocking(move |context| {
            let library = lora_library(context)?;
            let listed = library
                .list(request.profile_id.as_deref(), context.now())
                .map_err(image_error)?;
            Ok(dto::LoraList {
                loras: listed.into_iter().map(installed_lora).collect(),
            })
        })
        .await
}

/// Copies a LoRA file into the library; a different LoRA of the same name is
/// `Conflict`.
pub async fn loras_import(
    context: &ApiContext,
    request: dto::LorasImportRequest,
) -> Result<dto::InstalledLora, ApiError> {
    engine(context)?;
    let path = local_path(&request.source, "source")?;
    let library = lora_library(context)?;
    library
        .import(&path)
        .await
        .map(installed_lora)
        .map_err(image_error)
}

/// Deletes a LoRA no local model uses; one that is in use is `Conflict`.
pub async fn loras_delete(
    context: &ApiContext,
    request: dto::LorasDeleteRequest,
) -> Result<dto::LoraDeleted, ApiError> {
    engine(context)?;
    context
        .blocking(move |context| {
            let library = lora_library(context)?;
            let outcome = library.delete(&request.path).map_err(image_error)?;
            Ok(dto::LoraDeleted {
                left_behind: outcome
                    .left_behind
                    .into_iter()
                    .map(|path| path.display().to_string())
                    .collect(),
            })
        })
        .await
}

/// Sets the keywords the user wants for a LoRA; discovery never replaces
/// them.
pub async fn loras_update_keywords(
    context: &ApiContext,
    request: dto::LorasUpdateKeywordsRequest,
) -> Result<dto::LoraKeywordDiscovery, ApiError> {
    engine(context)?;
    context
        .blocking(move |context| {
            let library = lora_library(context)?;
            library
                .update_keywords(
                    &request.path,
                    request.keywords,
                    request.profile_id.as_deref(),
                    context.now(),
                )
                .map(lora_discovery)
                .map_err(image_error)
        })
        .await
}

/// Finds a LoRA's keywords and base architecture, as a job: it reads the
/// file's metadata, hashes the whole file and asks CivitAI once.
pub async fn lora_keywords_discover(
    context: &ApiContext,
    request: dto::LoraKeywordsDiscoverRequest,
) -> Result<dto::JobAccepted, ApiError> {
    engine(context)?;
    context
        .blocking(move |context| {
            admit_tool(
                context,
                "lora_keywords_discover",
                &request.client_operation_id,
                ImageToolDetail::LoraDiscover {
                    path: request.path,
                    profile_id: request.profile_id,
                },
                || crate::api::jobs::folder_move_active(context),
            )
        })
        .await
}

fn browser(context: &ApiContext) -> Result<std::sync::Arc<HuggingFaceBrowser>, ApiError> {
    context.local_models().browser(context)
}

pub async fn hf_image_bundle_profiles(
    _context: &ApiContext,
) -> Result<dto::ImageBundleProfiles, ApiError> {
    Ok(dto::ImageBundleProfiles {
        profiles:
            lettuce_image_generation::sd_runtime::server::LocalDiffusionEngine::bundle_profiles()
                .into_iter()
                .map(super::engine::bundle_profile)
                .collect(),
    })
}

const fn sort_key(sort: dto::HfSort) -> &'static str {
    match sort {
        dto::HfSort::TrendingScore => "trendingScore",
        dto::HfSort::Downloads => "downloads",
        dto::HfSort::Likes => "likes",
        dto::HfSort::LastModified => "lastModified",
    }
}

/// Repositories that can fill one role of an image architecture.
pub async fn hf_image_bundle_search(
    context: &ApiContext,
    request: dto::HfImageBundleSearchRequest,
) -> Result<dto::HfImageBundleSearchResults, ApiError> {
    let results = browser(context)?
        .bundle_role_search(
            context.secret_store().as_ref(),
            &HfBundleRoleSearch {
                profile_id: request.profile_id,
                role: role_of(request.role),
                query: request.query,
                sort: sort_key(request.sort.unwrap_or(dto::HfSort::Downloads)).to_owned(),
                author: request.author,
                format: request.format,
            },
        )
        .await
        .map_err(hf_error)?;
    Ok(dto::HfImageBundleSearchResults {
        models: results
            .into_iter()
            .map(|result| dto::HfModelSummary {
                model_id: result.model_id,
                author: result.author,
                likes: result.likes,
                downloads: result.downloads,
                tags: result.tags,
                pipeline_tag: result.pipeline_tag,
                last_modified: result.last_modified,
                trending_score: result.trending_score,
            })
            .collect(),
    })
}

fn asset_dto(asset: BundleAsset) -> dto::ImageBundleAsset {
    dto::ImageBundleAsset {
        selection_id: asset.selection_id,
        profile_id: asset.profile_id,
        role: role_dto(asset.role),
        model_id: asset.model_id,
        revision: asset.revision,
        relative_path: asset.relative_path,
        format: asset.format,
        quantization: asset.quantization,
        size: asset.size,
        sha256: asset.sha256,
        architecture: asset.architecture,
        gated: asset.gated,
    }
}

fn asset_of(asset: dto::ImageBundleAsset) -> BundleAsset {
    BundleAsset {
        selection_id: asset.selection_id,
        profile_id: asset.profile_id,
        role: role_of(asset.role),
        model_id: asset.model_id,
        revision: asset.revision,
        relative_path: asset.relative_path,
        format: asset.format,
        quantization: asset.quantization,
        size: asset.size,
        sha256: asset.sha256,
        architecture: asset.architecture,
        gated: asset.gated,
    }
}

/// The files of a repository that can fill a role, pinned to its current
/// revision.
pub async fn hf_image_bundle_files(
    context: &ApiContext,
    request: dto::HfImageBundleFilesRequest,
) -> Result<dto::HfImageBundleFiles, ApiError> {
    let secrets = context.secret_store().as_ref();
    let headers = HuggingFaceBrowser::download_client(secrets)
        .await
        .map_err(hf_error)?;
    let assets = browser(context)?
        .bundle_files(
            secrets,
            &headers,
            &request.profile_id,
            &request.model_id,
            role_of(request.role),
        )
        .await
        .map_err(hf_error)?;
    Ok(dto::HfImageBundleFiles {
        assets: assets.into_iter().map(asset_dto).collect(),
    })
}

fn busy_download(path: &std::path::Path) -> ApiError {
    api_error(
        ApiErrorCode::Busy,
        format!("{} is already being downloaded.", path.display()),
    )
}

/// The running bundle install whose files are exactly the wanted ones, or
/// `Busy` when any other running download shares a path or a hash with them.
fn running_bundle(
    context: &ApiContext,
    wanted: &[(PathBuf, String)],
) -> Result<Option<(lettuce_types::JobId, String)>, ApiError> {
    let mut partial = None;
    for (job_id, work) in context.jobs().installs() {
        let crate::api::InstallWork::Artifact { plan, finish } = &work else {
            continue;
        };
        let running = plan
            .artifacts
            .iter()
            .map(|planned| {
                (
                    planned
                        .artifact
                        .local_segments
                        .iter()
                        .fold(plan.root.clone(), |path, segment| path.join(segment)),
                    planned.artifact.sha256.clone().unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        let same_files = running.len() == wanted.len()
            && wanted
                .iter()
                .all(|(path, _)| running.iter().any(|(running, _)| running == path));
        if same_files
            && let crate::api::InstallFinish::HuggingFaceBundle { bundle_id, .. } = finish.as_ref()
        {
            return Ok(Some((job_id, bundle_id.clone())));
        }
        if let Some((path, _)) = wanted.iter().find(|(path, sha)| {
            running.iter().any(|(running, running_sha)| {
                running == path || (!sha.is_empty() && running_sha == sha)
            })
        }) {
            partial = Some(path.clone());
        }
    }
    match partial {
        Some(path) => Err(busy_download(&path)),
        None => Ok(None),
    }
}

/// Downloads a bundle of files and registers the model once they are in. A
/// bundle with exactly the files a running install fetches returns that job;
/// one that shares any file with a running download is `Busy`. One admission
/// at a time, so the second of two overlapping installs sees the first's job.
pub async fn hf_image_bundle_install(
    context: &ApiContext,
    request: dto::HfImageBundleInstallRequest,
) -> Result<dto::ImageBundleAccepted, ApiError> {
    let engine = engine(context)?;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let _admitting = context.image_state().bundle_admission().lock().await;
        super::refuse_during_move(context).await?;
        let paths = engine.paths();
        let root = bundle_root(&paths.image_root);
        let mut wanted = Vec::with_capacity(request.assets.len());
        for asset in &request.assets {
            let segments = asset_of(asset.clone())
                .local_segments()
                .map_err(|message| invalid_field("assets", message))?;
            wanted.push((
                segments
                    .iter()
                    .fold(root.clone(), |path, segment| path.join(segment)),
                asset.sha256.to_ascii_lowercase(),
            ));
        }
        if let Some((job_id, bundle_id)) = running_bundle(context, &wanted)? {
            return Ok(dto::ImageBundleAccepted {
                job_id: job_id.to_string(),
                bundle_id,
            });
        }
        let install = HfBundleInstallRequest {
            profile_id: request.profile_id,
            display_name: request.display_name,
            runtime_release: request.runtime_release,
            runtime_asset: request.runtime_asset,
            assets: request.assets.into_iter().map(asset_of).collect(),
        };
        let (manifest, plan) = browser(context)?
            .bundle_install(context.secret_store().as_ref(), &engine, &install)
            .await
            .map_err(hf_error)?;
        let detail =
            crate::api::jobs::image_bundle_detail(&manifest.bundle_id, &manifest.display_name)?;
        let accepted = admit_install_with_detail(
            context,
            crate::api::InstallWork::Artifact {
                plan,
                finish: Box::new(crate::api::InstallFinish::HuggingFaceBundle {
                    paths: (*paths).clone(),
                    bundle_id: manifest.bundle_id.clone(),
                }),
            },
            Some(detail),
        )
        .await?;
        Ok(dto::ImageBundleAccepted {
            job_id: accepted.job_id,
            bundle_id: manifest.bundle_id,
        })
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = (engine, request);
        Err(unsupported())
    }
}

/// Fetches the files of a bundle that are missing or unverified; `None`
/// when every file is in.
pub async fn hf_image_bundle_retry(
    context: &ApiContext,
    request: dto::HfImageBundleRetryRequest,
) -> Result<dto::ImageBundleRetried, ApiError> {
    let engine = engine(context)?;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        super::refuse_during_move(context).await?;
        let paths = engine.paths();
        let plan = browser(context)?
            .bundle_retry_downloads(
                context.secret_store().as_ref(),
                &paths.image_root,
                &request.bundle_id,
            )
            .await
            .map_err(hf_error)?;
        let Some(plan) = plan else {
            return Ok(dto::ImageBundleRetried { job_id: None });
        };
        let manifest =
            lettuce_image_generation::read_bundle_manifest(&paths.image_root, &request.bundle_id)
                .map_err(|message| api_error(ApiErrorCode::NotFound, message))?;
        let detail =
            crate::api::jobs::image_bundle_detail(&request.bundle_id, &manifest.display_name)?;
        let accepted = admit_install_with_detail(
            context,
            crate::api::InstallWork::Artifact {
                plan,
                finish: Box::new(crate::api::InstallFinish::HuggingFaceBundle {
                    paths: (*paths).clone(),
                    bundle_id: request.bundle_id,
                }),
            },
            Some(detail),
        )
        .await?;
        Ok(dto::ImageBundleRetried {
            job_id: Some(accepted.job_id),
        })
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = (engine, request);
        Err(unsupported())
    }
}

/// Registers a fully downloaded bundle's model again after a failure.
pub async fn hf_image_bundle_retry_registration(
    context: &ApiContext,
    request: dto::HfImageBundleRetryRequest,
) -> Result<dto::ImageBundleRegistered, ApiError> {
    let engine = engine(context)?;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let paths = engine.paths();
        let now = context.now();
        let model_id = crate::retry_hf_bundle_registration(
            context.backend().database(),
            &paths,
            &request.bundle_id,
            now,
        )
        .await
        .map_err(|message| api_error(ApiErrorCode::Conflict, message))?;
        Ok(dto::ImageBundleRegistered { model_id })
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = (engine, request);
        Err(unsupported())
    }
}

/// The image model files below the image folders.
pub async fn image_models_downloaded(
    context: &ApiContext,
) -> Result<dto::DownloadedImageModels, ApiError> {
    let engine = engine(context)?;
    context
        .blocking(move |_| {
            Ok(dto::DownloadedImageModels {
                models: crate::downloaded_image_models(&engine.paths())
                    .into_iter()
                    .map(|model| dto::DownloadedImageModel {
                        model_id: model.model_id,
                        filename: model.filename,
                        path: model.path,
                        size: model.size,
                        quantization: model.quantization,
                        is_mmproj: model.is_mmproj,
                        architecture: model.architecture,
                        role: role_dto(model.role),
                    })
                    .collect(),
            })
        })
        .await
}

fn civitai_image(image: lettuce_image_generation::CivitaiImage) -> dto::CivitaiImage {
    dto::CivitaiImage {
        url: image.url,
        nsfw_level: image.nsfw_level,
        width: image.width,
        height: image.height,
    }
}

fn civitai_error(message: String) -> ApiError {
    if message == crate::CIVITAI_MODEL_NOT_FOUND {
        api_error(ApiErrorCode::NotFound, message)
    } else {
        api_error(ApiErrorCode::Unavailable, message)
    }
}

fn pure_mode(context: &ApiContext) -> Result<lettuce_settings::PureMode, ApiError> {
    GlobalSettingsStore::load(context.backend().database())
        .map(|stored| stored.settings.pure_mode)
        .map_err(|_| api_error(ApiErrorCode::Internal, "the settings could not be read"))
}

/// LoRAs on CivitAI for the supported base models; NSFW is hidden at every
/// Pure mode level except off.
pub async fn civitai_search(
    context: &ApiContext,
    request: dto::CivitaiSearchRequest,
) -> Result<dto::CivitaiSearchPage, ApiError> {
    let pure = pure_mode(context)?;
    let browser = context.image_state().civitai(context)?;
    let page = browser
        .search(
            context.secret_store().as_ref(),
            &CivitaiSearch {
                query: request.query,
                sort: request.sort,
                period: request.period,
                base_models: request.base_models,
                cursor: request.cursor,
                limit: request.limit,
            },
            pure,
        )
        .await
        .map_err(civitai_error)?;
    Ok(dto::CivitaiSearchPage {
        items: page
            .items
            .into_iter()
            .map(|item| dto::CivitaiLoraSummary {
                id: item.id,
                name: item.name,
                nsfw: item.nsfw,
                nsfw_level: item.nsfw_level,
                creator_username: item.creator_username,
                download_count: item.download_count,
                thumbs_up_count: item.thumbs_up_count,
                preview_image: item.preview_image.map(civitai_image),
                base_models: item.base_models,
                latest_version_id: item.latest_version_id,
            })
            .collect(),
        next_cursor: page.next_cursor,
    })
}

fn civitai_detail(detail: lettuce_image_generation::CivitaiModelDetail) -> dto::CivitaiModelDetail {
    dto::CivitaiModelDetail {
        id: detail.id,
        name: detail.name,
        description: detail.description,
        nsfw: detail.nsfw,
        nsfw_level: detail.nsfw_level,
        creator_username: detail.creator_username,
        download_count: detail.download_count,
        thumbs_up_count: detail.thumbs_up_count,
        tags: detail.tags,
        versions: detail
            .versions
            .into_iter()
            .map(|version| dto::CivitaiVersion {
                id: version.id,
                name: version.name,
                base_model: version.base_model,
                architecture: version.architecture,
                published_at: version.published_at,
                trained_words: version.trained_words,
                images: version.images.into_iter().map(civitai_image).collect(),
                files: version
                    .files
                    .into_iter()
                    .map(|file| dto::CivitaiFile {
                        id: file.id,
                        name: file.name,
                        size_kb: file.size_kb,
                        primary: file.primary,
                        format: file.format,
                        fp: file.fp,
                        sha256: file.sha256,
                    })
                    .collect(),
            })
            .collect(),
    }
}

pub async fn civitai_model(
    context: &ApiContext,
    request: dto::CivitaiModelRequest,
) -> Result<dto::CivitaiModelDetail, ApiError> {
    let pure = pure_mode(context)?;
    context
        .image_state()
        .civitai(context)?
        .model(context.secret_store().as_ref(), request.model_id, pure)
        .await
        .map(civitai_detail)
        .map_err(civitai_error)
}

fn auth_status(status: crate::CivitaiAuthStatus) -> dto::CivitaiAuthStatus {
    dto::CivitaiAuthStatus {
        saved: status.saved,
        valid: status.valid,
        error_kind: status.error_kind.map(|kind| match kind {
            crate::CivitaiAuthErrorKind::MissingToken => dto::CivitaiAuthErrorKind::MissingToken,
            crate::CivitaiAuthErrorKind::Unverified => dto::CivitaiAuthErrorKind::Unverified,
            crate::CivitaiAuthErrorKind::InvalidOrExpired => {
                dto::CivitaiAuthErrorKind::InvalidOrExpired
            }
        }),
    }
}

pub async fn civitai_auth_status(context: &ApiContext) -> Result<dto::CivitaiAuthStatus, ApiError> {
    context
        .image_state()
        .civitai(context)?
        .auth_status(context.secret_store().as_ref())
        .await
        .map(auth_status)
        .map_err(civitai_error)
}

/// Saves the token unless CivitAI refuses it.
pub async fn civitai_auth_save(
    context: &ApiContext,
    request: dto::CivitaiAuthSaveRequest,
) -> Result<dto::CivitaiAuthStatus, ApiError> {
    if request.token.trim().is_empty() {
        return Err(invalid_field("token", "token is empty"));
    }
    context
        .image_state()
        .civitai(context)?
        .save_token(context.secret_store().as_ref(), &request.token)
        .await
        .map(auth_status)
        .map_err(civitai_error)
}

pub async fn civitai_auth_clear(context: &ApiContext) -> Result<(), ApiError> {
    crate::CivitaiBrowser::clear_token(context.secret_store().as_ref())
        .await
        .map_err(civitai_error)
}

/// Downloads a LoRA file of a CivitAI model version into the library with
/// its trained words and base model, read from CivitAI again.
pub async fn civitai_lora_download(
    context: &ApiContext,
    request: dto::CivitaiLoraDownloadRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let engine = engine(context)?;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        super::refuse_during_move(context).await?;
        let pure = pure_mode(context)?;
        let secrets = context.secret_store().as_ref();
        let detail = context
            .image_state()
            .civitai(context)?
            .model(secrets, request.model_id, pure)
            .await
            .map_err(civitai_error)?;
        let version = detail
            .versions
            .iter()
            .find(|version| version.id == request.version_id)
            .ok_or_else(|| invalid_field("version_id", "that model has no such version"))?;
        let file = version
            .files
            .iter()
            .find(|file| file.id == request.file_id)
            .ok_or_else(|| invalid_field("file_id", "that version has no such file"))?;
        let download = CivitaiLoraDownload {
            model_name: detail.name.clone(),
            version_id: version.id,
            file_name: file.name.clone(),
            sha256: file.sha256.clone(),
            download_url: file.download_url.clone(),
            trained_words: version.trained_words.clone(),
            base_model: version.base_model.clone(),
        };
        let client = crate::CivitaiBrowser::download_client(secrets)
            .await
            .map_err(|message| api_error(ApiErrorCode::Unavailable, message))?;
        let token_saved = crate::CivitaiBrowser::saved_token(secrets)
            .await
            .map_err(civitai_error)?
            .is_some();
        let lora_root = engine.paths().loras.clone();
        let plan = crate::civitai_lora_install_plan(&client, &lora_root, &download, token_saved)
            .await
            .map_err(civitai_error)?;
        crate::api::jobs::admit_install(
            context,
            crate::api::InstallWork::Artifact {
                plan,
                finish: Box::new(crate::api::InstallFinish::CivitaiLora {
                    lora_root,
                    download,
                }),
            },
        )
        .await
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = (engine, request);
        Err(unsupported())
    }
}
