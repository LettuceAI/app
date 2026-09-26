//! Which model an image or text feature uses: the old app's settings choice
//! when it suits the feature, else its fallback (the default model or the
//! first suitable model in settings order).

use lettuce_image_generation::LOCAL_DIFFUSION_PROVIDER_KIND;
use lettuce_models::{
    CapabilityStatus, ModelCatalog, ModelProfile, ModelRepositoryError, ProviderAccount,
};
use lettuce_settings::GlobalSettings;
use lettuce_types::ModelProfileId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFeature {
    Avatar,
    Scene,
    CreationHelper,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImageFeatureModelError {
    #[error("Scene generation is disabled in settings")]
    SceneDisabled,
    #[error("Avatar generation is disabled in settings")]
    AvatarDisabled,
    #[error("No image generation model is configured")]
    NoImageModel,
    #[error("Configured scene generation model does not support image output")]
    SceneModelUnsupported,
    #[error("{0}")]
    SceneWriter(&'static str),
    #[error("No model configured")]
    NoModel,
    #[error("Model not found")]
    ModelNotFound,
    #[error("{0}")]
    LorebookEntryGenerator(&'static str),
    #[error("{0}")]
    SoulWriter(&'static str),
    #[error("model storage is unavailable")]
    Storage,
}

impl From<ModelRepositoryError> for ImageFeatureModelError {
    fn from(_: ModelRepositoryError) -> Self {
        Self::Storage
    }
}

/// A chosen model with the account that serves it.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureModel {
    pub profile: ModelProfile,
    pub account: ProviderAccount,
}

impl FeatureModel {
    #[must_use]
    pub fn is_local_diffusion(&self) -> bool {
        self.account
            .provider_kind
            .eq_ignore_ascii_case(LOCAL_DIFFUSION_PROVIDER_KIND)
    }
}

fn supported(status: CapabilityStatus) -> bool {
    status == CapabilityStatus::Supported
}

/// The old `isImageGenerationModelAvailable`: image output, and no local
/// stable-diffusion.cpp model on mobile.
fn image_model(model: &FeatureModel) -> bool {
    supported(model.profile.config.capabilities.output_modalities.image)
        && !(cfg!(any(target_os = "android", target_os = "ios")) && model.is_local_diffusion())
}

/// The old `supports_scene_writer_model`.
fn writes_scenes(model: &FeatureModel, requires_vision: bool) -> bool {
    let capabilities = &model.profile.config.capabilities;
    supported(capabilities.input_modalities.text)
        && (!requires_vision || supported(capabilities.input_modalities.image))
        && supported(capabilities.output_modalities.text)
}

fn catalog<C: ModelCatalog + ?Sized>(
    models: &C,
) -> Result<Vec<FeatureModel>, ImageFeatureModelError> {
    let accounts = models.provider_accounts()?;
    Ok(models
        .model_profiles()?
        .into_iter()
        .filter_map(|profile| {
            let account = accounts
                .iter()
                .find(|account| account.id == profile.provider_account_id)?
                .clone();
            Some(FeatureModel { profile, account })
        })
        .collect())
}

fn preferred(candidates: &[FeatureModel], id: Option<ModelProfileId>) -> Option<&FeatureModel> {
    id.and_then(|id| candidates.iter().find(|model| model.profile.id == id))
}

/// The model `feature` generates with. Scenes follow the old backend (a
/// configured model that cannot output images is an error); avatars and the
/// creation helper follow the old settings pickers (fall back to the first
/// image model).
pub fn image_feature_model<C: ModelCatalog + ?Sized>(
    models: &C,
    settings: &GlobalSettings,
    feature: ImageFeature,
) -> Result<FeatureModel, ImageFeatureModelError> {
    let images = &settings.image_generation;
    let (enabled, disabled, chosen) = match feature {
        ImageFeature::Avatar => (
            images.avatar_enabled,
            ImageFeatureModelError::AvatarDisabled,
            images.avatar_model_profile_id,
        ),
        ImageFeature::Scene => (
            images.scene_enabled,
            ImageFeatureModelError::SceneDisabled,
            images.scene_model_profile_id,
        ),
        ImageFeature::CreationHelper => (
            true,
            ImageFeatureModelError::NoImageModel,
            images.creation_helper_model_profile_id,
        ),
    };
    if !enabled {
        return Err(disabled);
    }
    let all = catalog(models)?;
    if feature == ImageFeature::Scene
        && let Some(model) = preferred(&all, chosen)
    {
        return if image_model(model) {
            Ok(model.clone())
        } else {
            Err(ImageFeatureModelError::SceneModelUnsupported)
        };
    }
    let candidates = all.into_iter().filter(image_model).collect::<Vec<_>>();
    preferred(&candidates, chosen)
        .or_else(|| candidates.first())
        .cloned()
        .ok_or(ImageFeatureModelError::NoImageModel)
}

/// The old `supports_lorebook_entry_writer_model` /
/// `supports_text_generation_model`.
fn generates_text(model: &FeatureModel) -> bool {
    let capabilities = &model.profile.config.capabilities;
    supported(capabilities.input_modalities.text) && supported(capabilities.output_modalities.text)
}

/// The creation helper's chat model: its own setting, else the default
/// model; no capability is checked.
pub fn creation_helper_model<C: ModelCatalog + ?Sized>(
    models: &C,
    settings: &GlobalSettings,
    default_model: Option<ModelProfileId>,
) -> Result<FeatureModel, ImageFeatureModelError> {
    let id = settings
        .creation_helper
        .model_profile_id
        .or(default_model)
        .ok_or(ImageFeatureModelError::NoModel)?;
    preferred(&catalog(models)?, Some(id))
        .cloned()
        .ok_or(ImageFeatureModelError::ModelNotFound)
}

/// The old `resolve_lorebook_entry_writer_target`, used by the entry writer
/// and the keyword generator: the configured model must generate text,
/// otherwise the first model that does.
pub fn lorebook_entry_generator_model<C: ModelCatalog + ?Sized>(
    models: &C,
    settings: &GlobalSettings,
) -> Result<FeatureModel, ImageFeatureModelError> {
    let all = catalog(models)?;
    if let Some(id) = settings.lorebook_entry_generator.model_profile_id {
        let model =
            preferred(&all, Some(id)).ok_or(ImageFeatureModelError::LorebookEntryGenerator(
                "Configured lorebook entry generator model could not be resolved",
            ))?;
        return if generates_text(model) {
            Ok(model.clone())
        } else {
            Err(ImageFeatureModelError::LorebookEntryGenerator(
                "Configured lorebook entry generator model must support text input and text output",
            ))
        };
    }
    all.into_iter()
        .find(generates_text)
        .ok_or(ImageFeatureModelError::LorebookEntryGenerator(
            "No compatible lorebook entry generator model is configured",
        ))
}

/// The old `resolve_companion_soul_writer_target`: a model the request names
/// must generate text; otherwise the configured model, then the default
/// model, then the first model that generates text, skipping unsuitable ones.
pub fn soul_writer_model<C: ModelCatalog + ?Sized>(
    models: &C,
    settings: &GlobalSettings,
    default_model: Option<ModelProfileId>,
    requested: Option<ModelProfileId>,
) -> Result<FeatureModel, ImageFeatureModelError> {
    let all = catalog(models)?;
    if let Some(id) = requested {
        let model = preferred(&all, Some(id)).ok_or(ImageFeatureModelError::SoulWriter(
            "Selected Soul writer model could not be resolved",
        ))?;
        return if generates_text(model) {
            Ok(model.clone())
        } else {
            Err(ImageFeatureModelError::SoulWriter(
                "Selected Soul writer model must support text input and text output",
            ))
        };
    }
    [
        settings.companion_soul_writer.model_profile_id,
        default_model,
    ]
    .into_iter()
    .find_map(|id| preferred(&all, id).filter(|model| generates_text(model)))
    .or_else(|| all.iter().find(|model| generates_text(model)))
    .cloned()
    .ok_or(ImageFeatureModelError::SoulWriter(
        "No text generation model is configured",
    ))
}

/// The old `resolve_companion_soul_writer_fallback_target`: the configured
/// fallback model when it generates text.
pub fn soul_writer_fallback_model<C: ModelCatalog + ?Sized>(
    models: &C,
    settings: &GlobalSettings,
) -> Result<Option<FeatureModel>, ImageFeatureModelError> {
    Ok(preferred(
        &catalog(models)?,
        settings.companion_soul_writer.fallback_model_profile_id,
    )
    .filter(|model| generates_text(model))
    .cloned())
}

/// The old `resolve_scene_writer_target`: vision input is required unless the
/// scene image model runs locally.
pub fn scene_writer_model<C: ModelCatalog + ?Sized>(
    models: &C,
    settings: &GlobalSettings,
    requires_vision: bool,
) -> Result<FeatureModel, ImageFeatureModelError> {
    let all = catalog(models)?;
    if let Some(id) = settings.image_generation.scene_writer_model_profile_id {
        let model = preferred(&all, Some(id)).ok_or(ImageFeatureModelError::SceneWriter(
            "Configured scene writer model could not be resolved",
        ))?;
        return if writes_scenes(model, requires_vision) {
            Ok(model.clone())
        } else {
            Err(ImageFeatureModelError::SceneWriter(if requires_vision {
                "Configured scene writer model must support text and image input with text output"
            } else {
                "Configured scene writer model must support text input with text output"
            }))
        };
    }
    all.into_iter()
        .find(|model| writes_scenes(model, requires_vision))
        .ok_or(ImageFeatureModelError::SceneWriter(if requires_vision {
            "No compatible scene writer model is configured. Add an image-text-to-text model in Settings > Image Generation."
        } else {
            "No compatible scene writer model is configured. Add a text model in Settings > Image Generation."
        }))
}

#[cfg(test)]
mod tests {
    use lettuce_models::{
        ModelProfileRepository, ProviderAccountRepository, ProviderConfig, ProviderProtocol,
    };
    use lettuce_settings::SecretOwnerId;
    use lettuce_types::{ProviderAccountId, Revision, TimestampMillis};

    use super::*;

    fn account(kind: &str) -> ProviderAccount {
        ProviderAccount {
            id: ProviderAccountId::new(),
            secret_owner_id: SecretOwnerId::new(),
            provider_kind: kind.into(),
            protocol: if kind == LOCAL_DIFFUSION_PROVIDER_KIND {
                ProviderProtocol::StableDiffusion
            } else {
                ProviderProtocol::OpenAiCompatible
            },
            label: kind.into(),
            endpoint: None,
            enabled: true,
            streaming_enabled: true,
            allow_invalid_tls: false,
            api_key_ref: None,
            secret_headers: Vec::new(),
            config: ProviderConfig::Standard,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        }
    }

    fn profile(
        account: &ProviderAccount,
        name: &str,
        created_at: i64,
        image_out: bool,
        image_in: bool,
    ) -> ModelProfile {
        let mut profile = ModelProfile {
            id: ModelProfileId::new(),
            provider_account_id: account.id,
            external_model_id: name.into(),
            display_name: name.into(),
            kind: lettuce_models::ModelKind::Chat,
            config: lettuce_models::ModelProfileConfig {
                llama_cpp: Default::default(),
                stable_diffusion: Default::default(),
                feature_parameters: Default::default(),
                chat_parameters: Default::default(),
                capabilities: Default::default(),
            },
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(created_at),
            updated_at: TimestampMillis::new(created_at),
        };
        let capabilities = &mut profile.config.capabilities;
        capabilities.input_modalities.text = CapabilityStatus::Supported;
        capabilities.output_modalities.text = if image_out {
            CapabilityStatus::Unsupported
        } else {
            CapabilityStatus::Supported
        };
        capabilities.output_modalities.image = if image_out {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::Unsupported
        };
        capabilities.input_modalities.image = if image_in {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::Unsupported
        };
        profile
    }

    #[test]
    fn features_pick_their_setting_or_the_first_suitable_model_in_settings_order() {
        let database = lettuce_database::Database::open_in_memory().expect("database");
        let remote =
            ProviderAccountRepository::upsert(&database, account("openai"), None).expect("account");
        let text = profile(&remote, "text", 1, false, false);
        let vision = profile(&remote, "vision", 2, false, true);
        let first_image = profile(&remote, "first-image", 3, true, false);
        let second_image = profile(&remote, "second-image", 4, true, false);
        for model in [&second_image, &text, &first_image, &vision] {
            ModelProfileRepository::upsert(&database, model.clone(), None).expect("model");
        }
        let mut settings = GlobalSettings::default();
        let pick = |settings: &GlobalSettings, feature| {
            image_feature_model(&database, settings, feature).map(|model| model.profile.id)
        };
        assert_eq!(pick(&settings, ImageFeature::Avatar), Ok(first_image.id));
        assert_eq!(
            pick(&settings, ImageFeature::Scene),
            Err(ImageFeatureModelError::SceneDisabled)
        );
        settings.image_generation.scene_enabled = true;
        settings.image_generation.avatar_model_profile_id = Some(second_image.id);
        settings.image_generation.scene_model_profile_id = Some(text.id);
        settings.image_generation.creation_helper_model_profile_id = Some(text.id);
        assert_eq!(pick(&settings, ImageFeature::Avatar), Ok(second_image.id));
        assert_eq!(
            pick(&settings, ImageFeature::Scene),
            Err(ImageFeatureModelError::SceneModelUnsupported)
        );
        assert_eq!(
            pick(&settings, ImageFeature::CreationHelper),
            Ok(first_image.id)
        );
        settings.image_generation.avatar_enabled = false;
        assert_eq!(
            pick(&settings, ImageFeature::Avatar),
            Err(ImageFeatureModelError::AvatarDisabled)
        );
        assert_eq!(
            scene_writer_model(&database, &settings, true).map(|model| model.profile.id),
            Ok(vision.id)
        );
        assert_eq!(
            scene_writer_model(&database, &settings, false).map(|model| model.profile.id),
            Ok(text.id)
        );
        settings.image_generation.scene_writer_model_profile_id = Some(text.id);
        assert!(matches!(
            scene_writer_model(&database, &settings, true),
            Err(ImageFeatureModelError::SceneWriter(_))
        ));
    }

    #[test]
    fn text_features_follow_the_old_model_fallbacks() {
        let database = lettuce_database::Database::open_in_memory().expect("database");
        let remote =
            ProviderAccountRepository::upsert(&database, account("openai"), None).expect("account");
        let image = profile(&remote, "image", 1, true, false);
        let first_text = profile(&remote, "first-text", 2, false, false);
        let second_text = profile(&remote, "second-text", 3, false, false);
        for model in [&second_text, &image, &first_text] {
            ModelProfileRepository::upsert(&database, model.clone(), None).expect("model");
        }
        let id = |result: Result<FeatureModel, ImageFeatureModelError>| {
            result.map(|model| model.profile.id)
        };
        let mut settings = GlobalSettings::default();
        assert_eq!(
            id(creation_helper_model(&database, &settings, None)),
            Err(ImageFeatureModelError::NoModel)
        );
        assert_eq!(
            id(creation_helper_model(&database, &settings, Some(image.id))),
            Ok(image.id)
        );
        assert_eq!(
            id(lorebook_entry_generator_model(&database, &settings)),
            Ok(first_text.id)
        );
        assert_eq!(
            id(soul_writer_model(
                &database,
                &settings,
                Some(image.id),
                None
            )),
            Ok(first_text.id)
        );
        assert_eq!(
            id(soul_writer_model(
                &database,
                &settings,
                None,
                Some(image.id)
            )),
            Err(ImageFeatureModelError::SoulWriter(
                "Selected Soul writer model must support text input and text output"
            ))
        );
        settings.creation_helper.model_profile_id = Some(ModelProfileId::new());
        settings.lorebook_entry_generator.model_profile_id = Some(image.id);
        settings.companion_soul_writer.model_profile_id = Some(image.id);
        settings.companion_soul_writer.fallback_model_profile_id = Some(second_text.id);
        assert_eq!(
            id(creation_helper_model(
                &database,
                &settings,
                Some(first_text.id)
            )),
            Err(ImageFeatureModelError::ModelNotFound)
        );
        assert!(matches!(
            lorebook_entry_generator_model(&database, &settings),
            Err(ImageFeatureModelError::LorebookEntryGenerator(_))
        ));
        assert_eq!(
            id(soul_writer_model(
                &database,
                &settings,
                Some(second_text.id),
                None
            )),
            Ok(second_text.id)
        );
        assert_eq!(
            soul_writer_fallback_model(&database, &settings)
                .expect("fallback")
                .map(|model| model.profile.id),
            Some(second_text.id)
        );
    }
}
