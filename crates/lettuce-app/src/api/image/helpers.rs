//! The avatar image prompt, the avatar gradient and the design reference
//! writer.

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_models::{ModelProfileRepository, ProviderAccountRepository};
use lettuce_types::{AssetId, ModelProfileId};

use super::ApiContext;
use super::internal;
use crate::api::error::{api_error, invalid_field, parse_id};
use crate::{AvatarGradientError, AvatarPromptError, AvatarPromptRequest};

/// The prompt an avatar generation or edit sends to the model's provider; a
/// local stable-diffusion.cpp model receives the request as typed.
pub async fn avatar_prompt(
    context: &ApiContext,
    request: dto::AvatarPromptRequest,
) -> Result<dto::AvatarPrompt, ApiError> {
    let model_id: ModelProfileId = parse_id(&request.model_id, "model_id")?;
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let missing = || api_error(ApiErrorCode::NotFound, "the image model was not found");
            let profile = ModelProfileRepository::get(database, model_id)
                .map_err(internal)?
                .ok_or_else(missing)?;
            let account = ProviderAccountRepository::get(database, profile.provider_account_id)
                .map_err(internal)?
                .ok_or_else(missing)?;
            let prompt = match request.kind {
                dto::AvatarPromptKind::Generation {
                    subject_name,
                    subject_description,
                    avatar_request,
                } => AvatarPromptRequest::Generation {
                    subject_name,
                    subject_description,
                    avatar_request,
                },
                dto::AvatarPromptKind::Edit {
                    subject_name,
                    subject_description,
                    current_avatar_prompt,
                    edit_request,
                } => AvatarPromptRequest::Edit {
                    subject_name,
                    subject_description,
                    current_avatar_prompt,
                    edit_request,
                },
            };
            context
                .backend()
                .avatar_image_prompt(&account.provider_kind, &prompt)
                .map(|prompt| dto::AvatarPrompt { prompt })
                .map_err(|error| match error {
                    AvatarPromptError::Unavailable => {
                        api_error(ApiErrorCode::Unavailable, error.to_string())
                    }
                    AvatarPromptError::Render => internal(error),
                })
        })
        .await
}

/// The gradient colors behind an avatar image, computed once per image and
/// process unless `force` asks again.
pub async fn avatar_gradient(
    context: &ApiContext,
    request: dto::AvatarGradientRequest,
) -> Result<dto::AvatarGradient, ApiError> {
    let asset_id: AssetId = parse_id(&request.asset_id, "asset_id")?;
    context
        .blocking(move |context| {
            let media = context
                .media()
                .ok_or_else(|| api_error(ApiErrorCode::Unavailable, "no media store is open"))?;
            let gradient = context
                .image_state()
                .gradients()
                .gradient(media, asset_id, request.force)
                .map_err(|error| match error {
                    AvatarGradientError::NotFound(_) => {
                        api_error(ApiErrorCode::NotFound, error.to_string())
                    }
                    AvatarGradientError::Decode(_) => invalid_field("asset_id", error.to_string()),
                    AvatarGradientError::Media(_) => internal(error),
                })?;
            Ok(dto::AvatarGradient {
                colors: gradient
                    .colors
                    .into_iter()
                    .map(|color| dto::GradientColor {
                        r: color.r,
                        g: color.g,
                        b: color.b,
                        hex: color.hex,
                    })
                    .collect(),
                gradient_css: gradient.gradient_css,
                dominant_hue: gradient.dominant_hue,
                text_color: gradient.text_color,
                text_secondary: gradient.text_secondary,
            })
        })
        .await
}

/// Writes design notes from a subject's avatar and reference images, as a
/// job that streams the draft and ends with the text as its result.
pub async fn image_design_reference(
    context: &ApiContext,
    request: dto::ImageDesignReferenceRequest,
) -> Result<dto::JobAccepted, ApiError> {
    crate::api::jobs::admit_design_reference(context, request).await
}
