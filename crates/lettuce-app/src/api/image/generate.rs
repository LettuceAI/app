//! Image generation requests, the playground history, upscaling and what the
//! playground form offers per model.

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_image_generation::{
    ImageAttribution, ImageFailureKind, ImageGenerationRecord, ImageGenerationRepository,
    ImageGenerationRepositoryError, ImageGenerationRequest, ImageGenerationSource,
    ImageGenerationState, ImageOutputPolicy, ImageRequestValidationError, PlaygroundHistoryEntry,
    PlaygroundHistoryError, PlaygroundHistoryRepository, PlaygroundOrigin, PlaygroundUpscale,
    image_capability_catalog, playground_page_size,
};
use lettuce_jobs::{JobSnapshot, JobStore, StoreError};
use lettuce_media::MediaAssetRepository;
use lettuce_models::{
    ModelProfileRepository, ProviderAccountRepository, StableDiffusionLora, StableDiffusionSettings,
};
use lettuce_types::{
    AssetId, CharacterId, ConversationId, ModelProfileId, RequestId, TimestampMillis,
};

use super::internal;
use super::{ApiContext, engine, failure_kind, image_error};
use crate::api::error::{IntoApiError, api_error, invalid_field, parse_id};
use crate::api::jobs::{ImageToolDetail, admit_tool};
use crate::{ImageGenerationCoordinator, ImageGenerationError};

pub(crate) fn settings(value: &dto::ImageSettings) -> Result<StableDiffusionSettings, ApiError> {
    let json = serde_json::to_value(value).map_err(internal)?;
    serde_json::from_value(json).map_err(|error| invalid_field("settings", error.to_string()))
}

pub(super) fn lora(value: &dto::ImageLora) -> StableDiffusionLora {
    StableDiffusionLora {
        path: value.path.clone(),
        multiplier: value.multiplier,
        is_high_noise: value.is_high_noise,
        keywords: value.keywords.clone(),
    }
}

fn admission_error(error: ImageGenerationError) -> ApiError {
    match error {
        ImageGenerationError::Invalid(error) => {
            let field = match error {
                ImageRequestValidationError::EmptyPrompt
                | ImageRequestValidationError::PromptTooLong => "prompt",
                ImageRequestValidationError::Count => "count",
                ImageRequestValidationError::TooManyInputImages => "input_images",
                ImageRequestValidationError::Field(field) => field,
                ImageRequestValidationError::Settings => "settings",
                ImageRequestValidationError::Record => "request",
            };
            invalid_field(field, error.to_string())
        }
        ImageGenerationError::ModelMissing => {
            api_error(ApiErrorCode::NotFound, "the image model was not found")
        }
        ImageGenerationError::Profile(error) => invalid_field("model_id", error.to_string()),
        ImageGenerationError::Repository(ImageGenerationRepositoryError::Conflict)
        | ImageGenerationError::Jobs(StoreError::IdempotencyConflict) => api_error(
            ApiErrorCode::Conflict,
            "request_id names another image request",
        ),
        error => internal(error),
    }
}

/// Queues an image generation and returns its job. The request's id is its
/// idempotency key: repeating the request returns the same job, another
/// request under the same id is `Conflict`. A playground request replaces
/// the model's base LoRAs by the draft's, so a LoRA removed from the draft
/// does not run.
pub async fn image_generate(
    context: &ApiContext,
    request: dto::ImageGenerateRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let id: RequestId = parse_id(&request.request_id, "request_id")?;
    let model_profile_id: ModelProfileId = parse_id(&request.model_id, "model_id")?;
    let input_images = request
        .input_images
        .iter()
        .map(|asset| parse_id::<AssetId>(asset, "input_images"))
        .collect::<Result<Vec<_>, _>>()?;
    let mask_image = request
        .mask_image
        .as_deref()
        .map(|asset| parse_id::<AssetId>(asset, "mask_image"))
        .transpose()?;
    let attribution = ImageAttribution {
        conversation_id: request
            .attribution
            .conversation_id
            .as_deref()
            .map(|value| parse_id::<ConversationId>(value, "attribution.conversation_id"))
            .transpose()?,
        character_id: request
            .attribution
            .character_id
            .as_deref()
            .map(|value| parse_id::<CharacterId>(value, "attribution.character_id"))
            .transpose()?,
    };
    let source = match request.source {
        dto::ImageRequestSource::Direct => ImageGenerationSource::Direct,
        dto::ImageRequestSource::Playground => ImageGenerationSource::Playground,
        dto::ImageRequestSource::CreationHelper => ImageGenerationSource::CreationHelper,
    };
    let mut sampling = settings(&request.settings)?;
    if source == ImageGenerationSource::Playground && sampling.base_loras.is_none() {
        sampling.base_loras = Some(Vec::new());
    }
    let output_policy = match request.output {
        dto::ImageOutput::Retained => ImageOutputPolicy::Retained,
        dto::ImageOutput::Preview { expires_at } => ImageOutputPolicy::Preview {
            expires_at: TimestampMillis::new(expires_at),
        },
    };
    let generation = ImageGenerationRequest {
        id,
        model_profile_id,
        prompt: request.prompt,
        settings: sampling,
        input_images,
        mask_image,
        loras: request.loras.iter().map(lora).collect(),
        size: request.size,
        quality: request.quality,
        style: request.style,
        count: request.count.unwrap_or(1),
        source,
        attribution,
        output_policy,
        created_at: context.now(),
    };
    let job_id = context
        .blocking(move |context| {
            let database = context.backend().database();
            ImageGenerationCoordinator::new(database, database)
                .admit(generation, database)
                .map(|admission| admission.job.id)
                .map_err(admission_error)
        })
        .await?;
    context.jobs().wake();
    Ok(dto::JobAccepted {
        job_id: job_id.to_string(),
    })
}

/// The sizes, samplers and options the playground form offers for a model
/// or provider.
pub async fn image_capabilities(
    context: &ApiContext,
    request: dto::ImageCapabilitiesRequest,
) -> Result<dto::ImageCapabilities, ApiError> {
    context
        .blocking(move |context| {
            let (provider_kind, model, default_size) = match request.target {
                dto::ImageCapabilityTarget::Provider {
                    provider_kind,
                    model,
                } => (provider_kind, model, None),
                dto::ImageCapabilityTarget::Model { model_id } => {
                    let database = context.backend().database();
                    let model_id: ModelProfileId = parse_id(&model_id, "model_id")?;
                    let profile = ModelProfileRepository::get(database, model_id)
                        .map_err(internal)?
                        .ok_or_else(|| {
                            api_error(ApiErrorCode::NotFound, "the image model was not found")
                        })?;
                    let account =
                        ProviderAccountRepository::get(database, profile.provider_account_id)
                            .map_err(internal)?
                            .ok_or_else(|| {
                                api_error(ApiErrorCode::NotFound, "the image model was not found")
                            })?;
                    let default_size = profile
                        .config
                        .stable_diffusion
                        .cpp
                        .profile_id
                        .as_deref()
                        .and_then(|id| {
                            lettuce_image_generation::diffusion_catalog()
                                .profile(id)
                                .ok()
                        })
                        .map(|catalog| {
                            format!("{}x{}", catalog.default_width, catalog.default_height)
                        });
                    (
                        account.provider_kind,
                        Some(profile.external_model_id),
                        default_size,
                    )
                }
            };
            let values = image_capability_catalog().values(&provider_kind, model.as_deref());
            Ok(dto::ImageCapabilities {
                local: provider_kind
                    .eq_ignore_ascii_case(lettuce_image_generation::LOCAL_DIFFUSION_PROVIDER_KIND),
                provider_kind,
                sizes: values.sizes,
                default_size,
                samplers: values.samplers,
                schedulers: values.schedulers,
                negative_prompt: values.negative_prompt,
                quality: values.quality,
                styles: values.styles,
                max_count: values.max_count,
            })
        })
        .await
}

fn status(status: &str) -> dto::PlaygroundStatus {
    match status {
        "complete" => dto::PlaygroundStatus::Complete,
        "pending" => dto::PlaygroundStatus::Pending,
        "cancelled" => dto::PlaygroundStatus::Cancelled,
        _ => dto::PlaygroundStatus::Failed,
    }
}

fn upscale_of(params_json: &str) -> Option<String> {
    let params = serde_json::from_str::<serde_json::Value>(params_json).ok()?;
    ["upscale_of", "upscaleOf"]
        .into_iter()
        .find_map(|key| params.get(key).and_then(serde_json::Value::as_str))
        .map(str::to_owned)
}

/// The typed failure a failed job's label names, with the stored words.
pub(crate) fn failure_of_label(label: &str, message: &str) -> dto::ImageFailure {
    dto::ImageFailure {
        kind: failure_kind(ImageFailureKind::from_label(label).unwrap_or(ImageFailureKind::Other)),
        message: message.to_owned(),
    }
}

fn history_entry(
    context: &ApiContext,
    entry: PlaygroundHistoryEntry,
) -> Result<dto::PlaygroundEntry, ApiError> {
    let database = context.backend().database();
    let entry_status = status(&entry.status);
    let failure = if entry_status == dto::PlaygroundStatus::Failed {
        let label = match entry.job_id {
            Some(job_id) => JobStore::get(database, job_id)
                .map_err(IntoApiError::into_api_error)?
                .and_then(|job| job.error)
                .map(|error| error.message.as_str().to_owned()),
            None => None,
        };
        Some(failure_of_label(
            label.as_deref().unwrap_or_default(),
            entry.error.as_deref().unwrap_or_default(),
        ))
    } else {
        None
    };
    Ok(dto::PlaygroundEntry {
        upscale_of: upscale_of(&entry.params_json),
        id: entry.id,
        origin: match entry.origin {
            PlaygroundOrigin::Generated => dto::PlaygroundOrigin::Generated,
            PlaygroundOrigin::Imported => dto::PlaygroundOrigin::Imported,
        },
        job_id: entry.job_id.map(|id| id.to_string()),
        created_at: entry.created_at.get(),
        provider_kind: entry.provider_kind,
        model_id: entry.model_profile_id.map(|id| id.to_string()),
        model_name: entry.model_name,
        prompt: entry.prompt,
        negative_prompt: entry.negative_prompt,
        seed: entry.seed,
        params_json: entry.params_json,
        status: entry_status,
        failure,
        images: entry
            .images
            .into_iter()
            .map(|image| dto::PlaygroundImage {
                asset: image.asset_id.map(|asset_id| context.asset_ref(asset_id)),
                mime_type: image.mime_type,
                width: image.width,
                height: image.height,
            })
            .collect(),
    })
}

fn history_error(error: PlaygroundHistoryError) -> ApiError {
    match error {
        PlaygroundHistoryError::NotFound => {
            api_error(ApiErrorCode::NotFound, "the playground entry was not found")
        }
        PlaygroundHistoryError::Storage => internal(error),
    }
}

/// The playground's generations, newest first, 30 by default (1 to 200).
pub async fn playground_history_list(
    context: &ApiContext,
    request: dto::PlaygroundHistoryListRequest,
) -> Result<dto::PlaygroundHistoryPage, ApiError> {
    context
        .blocking(move |context| {
            let entries = context
                .backend()
                .database()
                .list_playground_history(
                    playground_page_size(request.limit),
                    request.before.map(TimestampMillis::new),
                )
                .map_err(history_error)?;
            Ok(dto::PlaygroundHistoryPage {
                entries: entries
                    .into_iter()
                    .map(|entry| history_entry(context, entry))
                    .collect::<Result<_, _>>()?,
            })
        })
        .await
}

/// Removes a playground entry and, with `delete_images`, its images unless
/// the library keeps them or something else uses them; the files go
/// straight away.
pub async fn playground_history_delete(
    context: &ApiContext,
    request: dto::PlaygroundHistoryDeleteRequest,
) -> Result<dto::PlaygroundHistoryDeleted, ApiError> {
    let id = request.id.trim().to_owned();
    if id.is_empty() {
        return Err(invalid_field("id", "id is empty"));
    }
    context
        .blocking(move |context| {
            let queued = context
                .backend()
                .delete_playground_history(&id, request.delete_images, context.now())
                .map_err(history_error)?;
            if !queued.is_empty() {
                collect_garbage(context)?;
            }
            let mut deleted = 0_u32;
            for asset_id in queued {
                if MediaAssetRepository::get(context.backend().database(), asset_id)
                    .map_err(internal)?
                    .is_none()
                {
                    deleted += 1;
                }
            }
            Ok(dto::PlaygroundHistoryDeleted {
                deleted_images: deleted,
            })
        })
        .await
}

/// Removes the files of images no asset record keeps any more.
pub(super) fn collect_garbage(context: &ApiContext) -> Result<(), ApiError> {
    let (Some(store), Some(files)) = (context.media(), context.database_files()) else {
        return Ok(());
    };
    crate::collect_media_garbage(
        context.backend().database(),
        &crate::MediaGarbageScope {
            store,
            location: &files.location,
            open_database: &files.active,
        },
        context.now(),
    )
    .map(|_| ())
    .map_err(internal)
}

pub(crate) fn upscale_entry_exists(context: &ApiContext, entry_id: &str) -> Result<bool, ApiError> {
    context
        .backend()
        .database()
        .playground_entry_exists(entry_id)
        .map_err(history_error)
}

/// Records an upscale as a playground entry.
pub(crate) fn record_upscale(
    context: &ApiContext,
    job_id: lettuce_types::JobId,
    source_entry_id: String,
    image: lettuce_image_generation::GeneratedImage,
) -> Result<String, ApiError> {
    context
        .backend()
        .database()
        .record_playground_upscale(PlaygroundUpscale {
            job_id,
            source_entry_id,
            image,
            created_at: context.now(),
        })
        .map_err(history_error)
}

/// What the job view shows of an image generation: why it failed and the
/// images it stored.
pub(crate) struct GenerationView {
    pub failure: Option<dto::ImageFailure>,
    pub result: Option<dto::JobResultDto>,
}

pub(crate) fn generation_view(
    context: &ApiContext,
    job: &JobSnapshot,
) -> Result<GenerationView, ApiError> {
    let record = match ImageGenerationRepository::get(context.backend().database(), job.id) {
        Ok(record) => Some(record),
        Err(ImageGenerationRepositoryError::NotFound) => None,
        Err(error) => return Err(internal(error)),
    };
    Ok(view_of(context, job, record.as_ref()))
}

fn view_of(
    context: &ApiContext,
    job: &JobSnapshot,
    record: Option<&ImageGenerationRecord>,
) -> GenerationView {
    let message = record.and_then(|record| match &record.state {
        ImageGenerationState::Failed { message, .. } => Some(message.as_str()),
        _ => None,
    });
    let failure = job
        .error
        .as_ref()
        .map(|error| failure_of_label(error.message.as_str(), message.unwrap_or_default()));
    let result = record.and_then(|record| match &record.state {
        ImageGenerationState::Succeeded { result } => Some(dto::JobResultDto::ImageGeneration {
            images: result
                .images
                .iter()
                .map(|image| dto::GeneratedImage {
                    asset: context.asset_ref(image.asset_id),
                    mime_type: image.mime_type.clone(),
                    width: image.width,
                    height: image.height,
                    text: image.text.clone(),
                })
                .collect(),
            rejected_outputs: result.rejected_outputs,
        }),
        _ => None,
    });
    GenerationView { failure, result }
}

/// Queues an upscale of a stored image and returns its job; an upscale of a
/// playground entry's image is recorded as an entry of its own when it
/// ends. The key names the job, so repeating the request returns it.
pub async fn image_upscale(
    context: &ApiContext,
    request: dto::ImageUpscaleRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let asset_id: AssetId = parse_id(&request.asset_id, "asset_id")?;
    let entry_id = request.origin.map(|origin| match origin {
        dto::ImageUpscaleOrigin::Playground { entry_id } => entry_id,
    });
    let engine = engine(context)?;
    let accepted = context
        .blocking(move |context| {
            admit_tool(
                context,
                "image_upscale",
                &request.client_operation_id,
                ImageToolDetail::Upscale {
                    asset_id: asset_id.to_string(),
                    entry_id: entry_id.clone(),
                },
                || {
                    engine.check_upscale_ready().map_err(image_error)?;
                    if let Some(entry_id) = &entry_id
                        && !upscale_entry_exists(context, entry_id)?
                    {
                        return Err(api_error(
                            ApiErrorCode::NotFound,
                            "the playground entry was not found",
                        ));
                    }
                    Ok(())
                },
            )
        })
        .await?;
    Ok(accepted)
}
