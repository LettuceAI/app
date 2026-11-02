use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_image_generation::resolve_image_profile;
use lettuce_models::{CapabilityStatus, ModelProfileRepository, ProviderAccountRepository};
use lettuce_settings::GlobalSettingsStore;
use lettuce_types::{AssetId, ModelProfileId};

use super::{ApiContext, internal};
use crate::api::error::{IntoApiError, api_error, invalid_field, parse_id};
use crate::{FeatureModel, ImageFeature, ImageFeatureModelError, image_feature_model};

fn model_view(model: &FeatureModel) -> dto::AvatarGenerationModel {
    dto::AvatarGenerationModel {
        id: model.profile.id.to_string(),
        name: model.profile.display_name.clone(),
        provider_kind: model.account.provider_kind.clone(),
        supports_editing: model.profile.config.capabilities.input_modalities.image
            == CapabilityStatus::Supported,
    }
}

fn selected_model(context: &ApiContext, requested: Option<&str>) -> Result<FeatureModel, ApiError> {
    let database = context.backend().database();
    let settings = GlobalSettingsStore::load(database).map_err(internal)?.settings;
    if !settings.image_generation.avatar_enabled {
        return Err(api_error(ApiErrorCode::Unsupported, "avatar generation is disabled"));
    }
    let model = if let Some(id) = requested {
        let id: ModelProfileId = parse_id(id, "model_id")?;
        let profile = ModelProfileRepository::get(database, id)
            .map_err(internal)?
            .ok_or_else(|| api_error(ApiErrorCode::NotFound, "the avatar model was not found"))?;
        let account = ProviderAccountRepository::get(database, profile.provider_account_id)
            .map_err(internal)?
            .ok_or_else(|| api_error(ApiErrorCode::NotFound, "the avatar provider was not found"))?;
        FeatureModel { profile, account }
    } else {
        image_feature_model(database, &settings, ImageFeature::Avatar).map_err(|error| match error {
            ImageFeatureModelError::NoImageModel => api_error(ApiErrorCode::ModelRequired, error.to_string()),
            error => internal(error),
        })?
    };
    resolve_image_profile(&model.profile, &model.account)
        .map_err(|error| invalid_field("model_id", error.to_string()))?;
    let capabilities = &model.profile.config.capabilities;
    if capabilities.input_modalities.image != CapabilityStatus::Supported
        || capabilities.output_modalities.image != CapabilityStatus::Supported
    {
        return Err(invalid_field("model_id", "the avatar model must support image input and output"));
    }
    if cfg!(any(target_os = "android", target_os = "ios")) && model.is_local_diffusion() {
        return Err(super::unsupported());
    }
    Ok(model)
}

pub async fn avatar_generation_options(context: &ApiContext) -> Result<dto::AvatarGenerationOptions, ApiError> {
    context.blocking(|context| {
        let enabled = GlobalSettingsStore::load(context.backend().database())
            .map_err(internal)?.settings.image_generation.avatar_enabled;
        let model = if enabled {
            match selected_model(context, None) {
                Ok(model) => Some(model_view(&model)),
                Err(error) if error.code == ApiErrorCode::ModelRequired => None,
                Err(error) => return Err(error),
            }
        } else {
            None
        };
        Ok(dto::AvatarGenerationOptions { enabled, model })
    }).await
}

pub async fn avatar_generate(context: &ApiContext, request: dto::AvatarGenerateRequest) -> Result<dto::AvatarGenerationAccepted, ApiError> {
    let (model, kind, source) = context.blocking(move |context| {
        let model = selected_model(context, request.model_id.as_deref())?;
        let editing = matches!(&request.kind, dto::AvatarPromptKind::Edit { .. });
        let authored = match &request.kind {
            dto::AvatarPromptKind::Generation { avatar_request, .. } => avatar_request,
            dto::AvatarPromptKind::Edit { edit_request, .. } => edit_request,
        };
        if authored.trim().is_empty() {
            return Err(invalid_field("kind", "the avatar request is empty"));
        }
        if editing != request.source_asset_id.is_some() {
            return Err(invalid_field("source_asset_id", "only an edit requires a source image"));
        }
        if editing && !model_view(&model).supports_editing {
            return Err(api_error(ApiErrorCode::Unsupported, "the avatar model does not accept image input"));
        }
        if let Some(source) = &request.source_asset_id {
            let id: AssetId = parse_id(source, "source_asset_id")?;
            let media = context.media().ok_or_else(|| api_error(ApiErrorCode::Unavailable, "media storage is unavailable"))?;
            let image = media.open_ready(id).map_err(IntoApiError::into_api_error)?;
            if image.asset.kind.blob_kind() != lettuce_media::MediaKind::Image {
                return Err(invalid_field("source_asset_id", "the source asset is not an image"));
            }
        }
        Ok((model, request.kind.clone(), request))
    }).await?;
    let prepared = super::avatar_prompt(context, dto::AvatarPromptRequest {
        model_id: model.profile.id.to_string(),
        kind,
    }).await?;
    let accepted = super::image_generate(context, dto::ImageGenerateRequest {
        request_id: source.request_id,
        model_id: model.profile.id.to_string(),
        prompt: prepared.prompt.clone(),
        settings: dto::ImageSettings::default(),
        input_images: source.source_asset_id.into_iter().collect(),
        mask_image: None,
        loras: Vec::new(),
        size: None,
        quality: None,
        style: None,
        count: Some(1),
        source: dto::ImageRequestSource::Direct,
        attribution: dto::ImageAttribution { conversation_id: None, character_id: source.character_id },
        output: dto::ImageOutput::Retained,
    }).await?;
    Ok(dto::AvatarGenerationAccepted { job_id: accepted.job_id, model: model_view(&model), prompt: prepared.prompt })
}
