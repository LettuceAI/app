//! The Hugging Face browser: search, repository files, model cards,
//! authors and avatars, the fit recommendation, GGUF downloads and the
//! access token.

use std::sync::Arc;

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_model_hub::{HfBrowseMode, HfSearch};
use lettuce_models::{GlobalModelSettingsRepository, ProviderAccountRepository};
use lettuce_types::ProviderAccountId;

use crate::HuggingFaceBrowser;
use crate::api::ApiContext;
use crate::api::error::{api_error, hf_error, invalid_field, parse_id};

fn browser(context: &ApiContext) -> Result<Arc<HuggingFaceBrowser>, ApiError> {
    context.local_models().browser(context)
}

pub fn hf_browser_links() -> dto::HfBrowserLinks {
    dto::HfBrowserLinks {
        repository_base: lettuce_model_hub::HUGGING_FACE_ENDPOINT.to_owned(),
        ollama_prefix: lettuce_model_hub::HUGGING_FACE_OLLAMA_PREFIX.to_owned(),
    }
}

const fn mode(mode: dto::HfBrowseMode) -> HfBrowseMode {
    match mode {
        dto::HfBrowseMode::Llm => HfBrowseMode::Llm,
        dto::HfBrowseMode::Image => HfBrowseMode::Image,
    }
}

const fn sort_key(sort: dto::HfSort) -> &'static str {
    match sort {
        dto::HfSort::TrendingScore => "trendingScore",
        dto::HfSort::Downloads => "downloads",
        dto::HfSort::Likes => "likes",
        dto::HfSort::LastModified => "lastModified",
    }
}

fn summary(result: lettuce_model_hub::HfSearchResult) -> dto::HfModelSummary {
    dto::HfModelSummary {
        model_id: result.model_id,
        author: result.author,
        likes: result.likes,
        downloads: result.downloads,
        tags: result.tags,
        pipeline_tag: result.pipeline_tag,
        last_modified: result.last_modified,
        trending_score: result.trending_score,
    }
}

fn required(value: &str, field: &str) -> Result<String, ApiError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(invalid_field(field, format!("{field} is empty")));
    }
    Ok(value.to_owned())
}

pub async fn hf_search(
    context: &ApiContext,
    request: dto::HfSearchRequest,
) -> Result<dto::HfSearchResults, ApiError> {
    let browser = browser(context)?;
    let results = browser
        .search(
            context.secret_store().as_ref(),
            &HfSearch {
                query: request.query,
                limit: request.limit,
                sort: request.sort.map(|sort| sort_key(sort).to_owned()),
                offset: request.offset,
                author: request.author,
                mode: mode(request.mode),
                unfiltered: request.unfiltered,
            },
        )
        .await
        .map_err(hf_error)?;
    Ok(dto::HfSearchResults {
        models: results.into_iter().map(summary).collect(),
    })
}

pub async fn hf_model_files(
    context: &ApiContext,
    request: dto::HfModelRequest,
) -> Result<dto::HfModelInfo, ApiError> {
    let model_id = required(&request.model_id, "model_id")?;
    let info = browser(context)?
        .model_files(
            context.secret_store().as_ref(),
            &model_id,
            mode(request.mode),
        )
        .await
        .map_err(hf_error)?;
    Ok(dto::HfModelInfo {
        model_id: info.model_id,
        revision: info.revision,
        author: info.author,
        likes: info.likes,
        downloads: info.downloads,
        tags: info.tags,
        architecture: info.architecture,
        context_length: info.context_length,
        parameter_count: info.parameter_count,
        last_modified: info.last_modified,
        files: info
            .files
            .into_iter()
            .map(|file| dto::HfModelFile {
                filename: file.filename,
                display_name: file.display_name,
                parts: file
                    .parts
                    .into_iter()
                    .map(|part| dto::HfFilePart {
                        filename: part.filename,
                        size: part.size,
                    })
                    .collect(),
                size: file.size,
                quantization: file.quantization,
                is_mmproj: file.is_mmproj,
                is_mtp: file.is_mtp,
                imatrix: file.imatrix,
            })
            .collect(),
    })
}

pub async fn hf_readme(
    context: &ApiContext,
    request: dto::HfReadmeRequest,
) -> Result<dto::HfReadme, ApiError> {
    let model_id = required(&request.model_id, "model_id")?;
    let markdown = browser(context)?
        .readme(context.secret_store().as_ref(), &model_id)
        .await
        .map_err(hf_error)?;
    Ok(dto::HfReadme { markdown })
}

/// An author's GGUF models and profile. A profile that cannot be read is
/// reported next to the models rather than failing them.
pub async fn hf_author(
    context: &ApiContext,
    request: dto::HfAuthorRequest,
) -> Result<dto::HfAuthor, ApiError> {
    let author = required(&request.author, "author")?;
    let browser = browser(context)?;
    let secrets = context.secret_store().as_ref();
    let models = browser
        .author_models(
            secrets,
            &author,
            request.search.as_deref(),
            request.limit,
            request.sort.map(sort_key),
            request.offset,
        )
        .await
        .map_err(hf_error)?;
    let profile = match browser.author_overview(secrets, &author).await {
        Ok(overview) => dto::HfAuthorProfile::Found {
            overview: dto::HfAuthorOverview {
                name: overview.name,
                fullname: overview.fullname,
                avatar_url: overview.avatar_url,
                details: overview.details,
                kind: overview.kind,
                is_pro: overview.is_pro,
                num_models: overview.num_models,
                num_datasets: overview.num_datasets,
                num_spaces: overview.num_spaces,
                num_likes: overview.num_likes,
                num_followers: overview.num_followers,
                num_following: overview.num_following,
                created_at: overview.created_at,
            },
        },
        Err(error) => dto::HfAuthorProfile::Unavailable {
            failure: error.failure().map(crate::api::error::hf_failure),
        },
    };
    Ok(dto::HfAuthor {
        profile,
        models: models.into_iter().map(summary).collect(),
    })
}

/// Each author's avatar, cached for the life of the process like the
/// legacy browser.
pub async fn hf_avatars(
    context: &ApiContext,
    request: dto::HfAvatarsRequest,
) -> Result<dto::HfAvatars, ApiError> {
    let authors = request
        .authors
        .iter()
        .map(|author| author.trim().to_owned())
        .filter(|author| !author.is_empty())
        .collect::<Vec<_>>();
    let found = browser(context)?
        .avatars(context.secret_store().as_ref(), &authors)
        .await
        .map_err(hf_error)?;
    Ok(dto::HfAvatars {
        avatars: authors
            .into_iter()
            .map(|author| dto::HfAvatar {
                url: found.get(&author).filter(|url| !url.is_empty()).cloned(),
                author,
            })
            .collect(),
    })
}

fn header_source() -> Result<lettuce_network::ArtifactDownloadClient, ApiError> {
    lettuce_network::ArtifactDownloadClient::new()
        .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))
}

pub async fn hf_recommendation(
    context: &ApiContext,
    request: dto::HfRecommendationRequest,
) -> Result<dto::HfRecommendation, ApiError> {
    if let Some(choice) = &request.plan
        && !crate::models::hf_canonical_preview::BROWSER_KV_TYPES
            .iter()
            .any(|kind| kind.eq_ignore_ascii_case(choice.kv_type.trim()))
    {
        return Err(invalid_field("plan.kv_type", "unsupported KV cache type"));
    }
    let model_id = required(&request.model_id, "model_id")?;
    let revision = request
        .revision
        .as_deref()
        .map(str::trim)
        .filter(|revision| !revision.is_empty())
        .unwrap_or("main")
        .to_owned();
    let canonical_hardware =
        canonical_hardware(context, request.ollama_account_id.as_deref()).await?;
    let canonical_defaults = context
        .blocking(|context| {
            context
                .backend()
                .database()
                .global_model_settings()
                .map(|(settings, _)| settings.llama_cpp)
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))
        })
        .await?;
    let preview = match canonical_hardware.as_ref() {
        Some(hardware) => Some(
            crate::models::hf_canonical_preview::canonical_recommendation(
                browser(context)?.as_ref(),
                context.secret_store().as_ref(),
                &header_source()?,
                crate::RemoteModel {
                    id: &model_id,
                    revision: &revision,
                },
                &request,
                hardware,
                &canonical_defaults,
            )
            .await
            .map_err(hf_error)?,
        ),
        None => None,
    };
    let hardware = canonical_hardware.as_ref();
    Ok(dto::HfRecommendation {
        hardware_available: hardware.is_some(),
        metadata_available: preview
            .as_ref()
            .is_some_and(|preview| preview.files.iter().any(|file| file.max_context.is_some())),
        available_ram: hardware
            .and_then(|value| value.available_memory_bytes)
            .unwrap_or(0),
        available_vram: hardware
            .and_then(|value| value.available_vram_bytes)
            .unwrap_or(0),
        supports_gpu_offload: hardware.is_some_and(|value| value.supports_gpu_offload),
        unified_memory: hardware.is_some_and(|value| value.unified_memory),
        total_available: 0,
        kv_base_per_token: None,
        kv_context_cap: None,
        model_max_context: 0,
        arch: None,
        files: Vec::new(),
        best: None,
        gpu_layer_count: None,
        kv_types: Vec::new(),
        plan: None,
        preview,
    })
}

/// Downloads a GGUF model with its projector and MTP draft model as one
/// job. A request for an install already queued or running joins it when
/// its setup is the same and is a conflict otherwise; a retried
/// `client_operation_id` returns its job.
pub async fn hf_download(
    context: &ApiContext,
    request: dto::HfDownloadRequest,
) -> Result<dto::JobAccepted, ApiError> {
    if cfg!(any(target_os = "android", target_os = "ios")) {
        return Err(super::local::unsupported_on_mobile(
            "Downloading local models",
        ));
    }
    crate::api::jobs::admit_gguf_download(context, request).await
}

fn token_status(status: &lettuce_model_hub::HfAuthStatus) -> dto::HfTokenStatus {
    use lettuce_model_hub::HfAuthErrorKind;
    match (status.valid, status.error_kind, &status.username) {
        (true, _, Some(username)) => dto::HfTokenStatus::Valid {
            username: username.clone(),
        },
        (_, Some(HfAuthErrorKind::MissingToken), _) => dto::HfTokenStatus::Missing,
        (_, Some(HfAuthErrorKind::Unknown { offline }), _) => {
            dto::HfTokenStatus::Unknown { offline }
        }
        _ => dto::HfTokenStatus::Invalid,
    }
}

pub async fn hf_auth_status(context: &ApiContext) -> Result<dto::HfTokenStatus, ApiError> {
    let status = browser(context)?
        .auth_status(context.secret_store().as_ref())
        .await
        .map_err(hf_error)?;
    Ok(token_status(&status))
}

/// Saves the token once Hugging Face accepts it.
pub async fn hf_auth_save(
    context: &ApiContext,
    request: dto::HfAuthSaveRequest,
) -> Result<dto::HfTokenStatus, ApiError> {
    if request.token.trim().is_empty() {
        return Err(invalid_field("token", "token is empty"));
    }
    let status = browser(context)?
        .save_token(context.secret_store().as_ref(), &request.token)
        .await
        .map_err(hf_error)?;
    Ok(token_status(&status))
}

pub async fn hf_auth_clear(context: &ApiContext) -> Result<(), ApiError> {
    browser(context)?
        .clear_token(context.secret_store().as_ref())
        .await
        .map_err(hf_error)
}

async fn canonical_hardware(
    context: &ApiContext,
    account_id: Option<&str>,
) -> Result<Option<lettuce_local_llm::context_preview::PreviewHardware>, ApiError> {
    let Some(id) = account_id else {
        return Ok(crate::models::hf_canonical_preview::local_preview_hardware());
    };
    let id: ProviderAccountId = parse_id(id, "ollama_account_id")?;
    let account = context
        .blocking(move |context| {
            ProviderAccountRepository::get(context.backend().database(), id)
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))
        })
        .await?
        .ok_or_else(|| api_error(ApiErrorCode::NotFound, "the account was not found"))?;
    let tls = context
        .backend()
        .tls_policy()
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
    let client = lettuce_network::JsonClient::with_tls(&tls)
        .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?;
    let bytes = crate::models::model_runnability::sprout_specs_bytes(
        &client,
        context.secret_store().as_ref(),
        &account,
    )
    .await
    .map_err(|error| api_error(ApiErrorCode::Unavailable, error))?;
    bytes
        .as_deref()
        .map(crate::models::hf_canonical_preview::sprout_preview_hardware)
        .transpose()
        .map_err(|error| api_error(ApiErrorCode::Unavailable, error))
}
