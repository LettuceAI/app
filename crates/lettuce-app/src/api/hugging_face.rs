//! The Hugging Face browser: search, repository files, model cards,
//! authors and avatars, how well files run here, the download planner's
//! recommendation, GGUF downloads and the access token.

use std::sync::Arc;

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_model_hub::{HfBrowseMode, HfSearch, RunnabilityFile, RunnabilityHardware};
use lettuce_models::ProviderAccountRepository;
use lettuce_types::ProviderAccountId;

use super::ApiContext;
use super::error::{api_error, hf_error, invalid_field, parse_id};
use super::local_models::runnability_defaults;
use crate::HuggingFaceBrowser;

fn browser(context: &ApiContext) -> Result<Arc<HuggingFaceBrowser>, ApiError> {
    context.local_models().browser(context)
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
        files: info
            .files
            .into_iter()
            .map(|file| dto::HfModelFile {
                filename: file.filename,
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
            failure: error.failure().map(super::error::hf_failure),
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

fn runnability_files(files: &[dto::HfRunnabilityFile]) -> Vec<RunnabilityFile> {
    files
        .iter()
        .map(|file| RunnabilityFile {
            filename: file.filename.clone(),
            size: file.size,
            quantization: lettuce_model_hub::extract_quantization(&file.filename),
        })
        .collect()
}

/// The hardware the files would run on: this machine, or the machine behind
/// the Ollama account's Sprout probe; `None` for an Ollama account without
/// one.
async fn hardware(
    context: &ApiContext,
    ollama_account_id: Option<&str>,
) -> Result<Option<RunnabilityHardware>, ApiError> {
    let account = match ollama_account_id {
        Some(id) => {
            let id: ProviderAccountId = parse_id(id, "ollama_account_id")?;
            let account = context
                .blocking(move |context| {
                    ProviderAccountRepository::get(context.backend().database(), id)
                        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))
                })
                .await?
                .ok_or_else(|| api_error(ApiErrorCode::NotFound, "the account was not found"))?;
            Some(account)
        }
        None => None,
    };
    let tls = context
        .backend()
        .tls_policy()
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
    let client = lettuce_network::JsonClient::with_tls(&tls)
        .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?;
    crate::runnability_hardware(&client, context.secret_store().as_ref(), account.as_ref())
        .await
        .map_err(|error| api_error(ApiErrorCode::Unavailable, error))
}

fn header_source() -> Result<lettuce_network::ArtifactDownloadClient, ApiError> {
    lettuce_network::ArtifactDownloadClient::new()
        .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))
}

async fn defaults(
    context: &ApiContext,
) -> Result<lettuce_model_hub::RunnabilityDefaults, ApiError> {
    context.blocking(runnability_defaults).await
}

pub async fn hf_runnability(
    context: &ApiContext,
    request: dto::HfRunnabilityRequest,
) -> Result<dto::HfRunnability, ApiError> {
    let model_id = required(&request.model_id, "model_id")?;
    let revision = request
        .revision
        .as_deref()
        .map(str::trim)
        .filter(|revision| !revision.is_empty())
        .unwrap_or("main")
        .to_owned();
    let Some(hardware) = hardware(context, request.ollama_account_id.as_deref()).await? else {
        return Ok(dto::HfRunnability {
            hardware_available: false,
            metadata_available: false,
            scores: Vec::new(),
        });
    };
    let files = runnability_files(&request.files);
    let (scores, metadata_available) = browser(context)?
        .runnability(
            context.secret_store().as_ref(),
            &header_source()?,
            crate::RemoteModel {
                id: &model_id,
                revision: &revision,
            },
            &files,
            hardware,
            defaults(context).await?,
        )
        .await
        .map_err(hf_error)?;
    Ok(dto::HfRunnability {
        hardware_available: true,
        metadata_available,
        scores: scores
            .into_iter()
            .map(|score| dto::HfRunnabilityScore {
                filename: score.filename,
                score: score.score,
                label: super::local_models::runnability_label(score.label),
                fits_in_ram: score.fits_in_ram,
                fits_in_vram: score.fits_in_vram,
                gpu_mode: super::local_models::gpu_mode(score.gpu_mode),
            })
            .collect(),
    })
}

const fn plan_offload(offload: dto::HfModelOffload) -> lettuce_model_hub::ModelOffload {
    match offload {
        dto::HfModelOffload::Auto => lettuce_model_hub::ModelOffload::Auto,
        dto::HfModelOffload::Cpu => lettuce_model_hub::ModelOffload::Cpu,
        dto::HfModelOffload::Gpu => lettuce_model_hub::ModelOffload::Gpu,
        dto::HfModelOffload::Mixed => lettuce_model_hub::ModelOffload::Mixed,
    }
}

const fn plan_gpu_mode(mode: lettuce_model_hub::PlannerGpuMode) -> dto::HfPlanGpuMode {
    use lettuce_model_hub::PlannerGpuMode as Mode;
    match mode {
        Mode::Full => dto::HfPlanGpuMode::Full,
        Mode::NearFull => dto::HfPlanGpuMode::NearFull,
        Mode::KvSpill => dto::HfPlanGpuMode::KvSpill,
        Mode::KvHeavySpill => dto::HfPlanGpuMode::KvHeavySpill,
        Mode::RamModelVramCtx => dto::HfPlanGpuMode::RamModelVramCtx,
        Mode::RamModelRamCtx => dto::HfPlanGpuMode::RamModelRamCtx,
        Mode::MostLayers => dto::HfPlanGpuMode::MostLayers,
        Mode::HalfLayers => dto::HfPlanGpuMode::HalfLayers,
        Mode::FewLayers => dto::HfPlanGpuMode::FewLayers,
        Mode::Cpu => dto::HfPlanGpuMode::Cpu,
        Mode::GpuUnavailable => dto::HfPlanGpuMode::GpuUnavailable,
    }
}

/// The planner's report for `choice` over `recommendation`.
fn plan_report(
    recommendation: &lettuce_model_hub::RecommendationData,
    choice: &dto::HfPlanChoice,
    sidecar_reserve_bytes: u64,
) -> Option<dto::HfPlan> {
    let report = lettuce_model_hub::planner_report(
        &lettuce_model_hub::PlannerModel::from_recommendation(recommendation),
        &lettuce_model_hub::PlannerChoice {
            filename: choice.filename.clone(),
            kv_type: choice.kv_type.clone(),
            model_offload: plan_offload(choice.model_offload),
            kv_placement: match choice.kv_placement {
                dto::HfKvPlacement::Auto => lettuce_model_hub::PlannerKvPlacement::Auto,
                dto::HfKvPlacement::Ram => lettuce_model_hub::PlannerKvPlacement::Ram,
                dto::HfKvPlacement::Vram => lettuce_model_hub::PlannerKvPlacement::Vram,
            },
            context_length: choice.context_length,
            sidecar_reserve_bytes,
        },
    )?;
    Some(dto::HfPlan {
        filename: report.filename,
        max_context: report.max_context,
        context_length: report.context_length,
        effective_kv_context: report.effective_kv_context,
        kv_bytes: report.kv_bytes,
        overhead_bytes: report.overhead_bytes,
        total_needed_bytes: report.total_needed_bytes,
        gpu_resident_bytes: report.gpu_resident_bytes,
        headroom_bytes: report.headroom_bytes,
        vram_budget_bytes: report.vram_budget_bytes,
        score: report.score.score,
        label: super::local_models::runnability_label(report.score.label),
        fits_vram: report.score.fits_vram,
        gpu_mode: plan_gpu_mode(report.score.gpu_mode),
        gpu_score: report.score.gpu_score,
        memory_score: report.memory_score,
        kv_score: report.kv_score,
        gpu_optimal_context: report.gpu_optimal_context,
        ram_max_context: report.ram_max_context,
        show_gpu_planning: report.show_gpu_planning,
        offload_percent: report.offload_percent,
        total_layers: report.total_layers,
        recommended_layers: report.recommended_layers,
        full_gpu_context: report.full_gpu_context,
        kv_distribution: report.kv_distribution.map(|kv| dto::HfPlanKvDistribution {
            vram_percent: kv.vram_percent,
            on_vram_bytes: kv.on_vram_bytes,
            on_ram_bytes: kv.on_ram_bytes,
        }),
        mixed_gpu_layers: report.mixed_gpu_layers,
        requested_gpu_layers: report.requested_gpu_layers,
        upgrade: report.upgrade.map(|upgrade| dto::HfPlanUpgrade {
            filename: upgrade.filename,
            score: upgrade.score,
        }),
        default_context: report.default_context,
        headroom: match report.headroom {
            lettuce_model_hub::HeadroomStatus::Comfortable => dto::HfHeadroomStatus::Comfortable,
            lettuce_model_hub::HeadroomStatus::Ok => dto::HfHeadroomStatus::Ok,
            lettuce_model_hub::HeadroomStatus::Tight => dto::HfHeadroomStatus::Tight,
            lettuce_model_hub::HeadroomStatus::Risky => dto::HfHeadroomStatus::Risky,
        },
        run: match report.run {
            lettuce_model_hub::RunStatus::Yes => dto::HfRunStatus::Yes,
            lettuce_model_hub::RunStatus::Borderline => dto::HfRunStatus::Borderline,
            lettuce_model_hub::RunStatus::No => dto::HfRunStatus::No,
        },
        prefill_speed: speed(report.performance.0),
        generation_speed: speed(report.performance.1),
        offload_kqv: report.offload_kqv,
    })
}

const fn speed(speed: lettuce_model_hub::PlannerSpeed) -> dto::HfSpeed {
    match speed {
        lettuce_model_hub::PlannerSpeed::Fast => dto::HfSpeed::Fast,
        lettuce_model_hub::PlannerSpeed::Medium => dto::HfSpeed::Medium,
        lettuce_model_hub::PlannerSpeed::Slow => dto::HfSpeed::Slow,
    }
}

/// The recommendation, the planner's limits next to the chosen sidecars and,
/// for the planner's current choice, its full report; the GGUF header is
/// read once per file for the life of the process.
pub async fn hf_recommendation(
    context: &ApiContext,
    request: dto::HfRecommendationRequest,
) -> Result<dto::HfRecommendation, ApiError> {
    let model_id = required(&request.model_id, "model_id")?;
    let revision = request
        .revision
        .as_deref()
        .map(str::trim)
        .filter(|revision| !revision.is_empty())
        .unwrap_or("main")
        .to_owned();
    let hardware = hardware(context, request.ollama_account_id.as_deref()).await?;
    let files = runnability_files(&request.files);
    let (recommendation, metadata_available) = match hardware {
        Some(hardware) => browser(context)?
            .recommendation(
                context.secret_store().as_ref(),
                &header_source()?,
                crate::RemoteModel {
                    id: &model_id,
                    revision: &revision,
                },
                &files,
                hardware,
                defaults(context).await?,
            )
            .await
            .map_err(hf_error)?,
        None => (lettuce_model_hub::RecommendationData::empty(), false),
    };
    let sidecar_reserve_bytes = request.sidecar_reserve_bytes.unwrap_or(0);
    let limits = lettuce_model_hub::planner_limits(&recommendation, sidecar_reserve_bytes);
    let plan = request
        .plan
        .as_ref()
        .and_then(|choice| plan_report(&recommendation, choice, sidecar_reserve_bytes));
    Ok(dto::HfRecommendation {
        hardware_available: hardware.is_some(),
        metadata_available,
        available_ram: recommendation.available_ram,
        available_vram: recommendation.available_vram,
        supports_gpu_offload: recommendation.supports_gpu_offload,
        unified_memory: recommendation.unified_memory,
        total_available: recommendation.total_available,
        kv_base_per_token: recommendation.kv_base_per_token,
        kv_context_cap: recommendation.kv_context_cap,
        model_max_context: recommendation.model_max_context,
        arch: recommendation.arch.map(|arch| dto::HfModelArch {
            architecture: arch.meta.architecture,
            block_count: arch.meta.block_count,
            embedding_length: arch.meta.embedding_length,
            head_count: arch.meta.head_count,
            head_count_kv: arch.meta.head_count_kv,
            context_length: arch.meta.context_length,
            expert_count: arch.meta.expert_count,
            expert_used_count: arch.meta.expert_used_count,
            is_moe: arch.is_moe,
            active_weight_ratio: arch.active_weight_ratio,
            incomplete_parse: arch.incomplete_parse,
        }),
        files: recommendation
            .files
            .into_iter()
            .zip(limits.files)
            .map(|(file, limits)| dto::HfFileRecommendation {
                filename: file.filename,
                size: file.size,
                quantization: file.quantization,
                quant_quality: file.quant_quality,
                max_context_f16: file.max_context_f16,
                max_context_q8_0: file.max_context_q8_0,
                max_context_q4_0: file.max_context_q4_0,
                optimal_gpu_ctx: file.optimal_gpu_ctx,
                optimal_ram_ctx: file.optimal_ram_ctx,
                max_context_by_kv_type: lettuce_model_hub::PLANNER_KV_TYPES
                    .iter()
                    .zip(limits.max_context_by_kv_type)
                    .map(|((kv_type, _), max_context)| dto::HfKvContextLimit {
                        kv_type: (*kv_type).to_owned(),
                        max_context,
                    })
                    .collect(),
            })
            .collect(),
        best: recommendation.best.map(|best| dto::HfBestRecommendation {
            filename: best.filename,
            context_length: best.context_length,
            kv_type: best.kv_type,
            score: best.score,
            viable: best.viable,
        }),
        gpu_layer_count: limits.gpu_layer_count,
        kv_types: lettuce_model_hub::PLANNER_KV_TYPES
            .iter()
            .map(|(kv_type, bytes_per_value)| dto::HfKvType {
                kv_type: (*kv_type).to_owned(),
                bytes_per_value: *bytes_per_value,
            })
            .collect(),
        plan,
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
        return Err(super::local_models::unsupported_on_mobile(
            "Downloading local models",
        ));
    }
    super::jobs::admit_gguf_download(context, request).await
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
