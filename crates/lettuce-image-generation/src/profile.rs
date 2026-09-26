use lettuce_models::{
    CapabilityStatus, ModelProfile, ProviderAccount, ProviderProtocol, StableDiffusionSettings,
    validate_provider_connection,
};
use lettuce_types::{ModelProfileId, ProviderAccountId, Revision};

/// The provider kind of the embedded stable-diffusion.cpp runtime.
pub const LOCAL_DIFFUSION_PROVIDER_KIND: &str = "sdcpp";

/// A model that can generate images, with its account, checked when a
/// generation runs.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedImageProfile {
    pub model_profile_id: ModelProfileId,
    pub model_revision: Revision,
    pub account: ProviderAccount,
    pub external_model_id: String,
    pub display_name: String,
    pub settings: StableDiffusionSettings,
    /// Providers that can answer with text are asked for text alongside the
    /// image when the model outputs text.
    pub text_output: bool,
}

impl ResolvedImageProfile {
    #[must_use]
    pub const fn provider_account_id(&self) -> ProviderAccountId {
        self.account.id
    }

    /// Whether the model runs on this device's stable-diffusion.cpp runtime.
    #[must_use]
    pub fn is_local_diffusion(&self) -> bool {
        self.account.protocol == ProviderProtocol::StableDiffusion
            && self
                .account
                .provider_kind
                .eq_ignore_ascii_case(LOCAL_DIFFUSION_PROVIDER_KIND)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ImageProfileError {
    #[error("the image model belongs to a different provider account")]
    AccountMismatch,
    #[error("the image model's provider account is disabled")]
    AccountDisabled,
    #[error("the image model's provider connection is invalid")]
    InvalidConnection,
    #[error("the model does not generate images")]
    NotAnImageModel,
    #[error("the image model's settings are invalid")]
    InvalidSettings,
}

/// Any model whose output scopes include images is an image model, whatever
/// its kind.
pub fn resolve_image_profile(
    profile: &ModelProfile,
    account: &ProviderAccount,
) -> Result<ResolvedImageProfile, ImageProfileError> {
    if profile.provider_account_id != account.id {
        return Err(ImageProfileError::AccountMismatch);
    }
    if !account.enabled {
        return Err(ImageProfileError::AccountDisabled);
    }
    validate_provider_connection(account).map_err(|_| ImageProfileError::InvalidConnection)?;
    let capabilities = &profile.config.capabilities;
    if capabilities.output_modalities.image != CapabilityStatus::Supported {
        return Err(ImageProfileError::NotAnImageModel);
    }
    profile
        .config
        .stable_diffusion
        .validate()
        .map_err(|_| ImageProfileError::InvalidSettings)?;
    Ok(ResolvedImageProfile {
        model_profile_id: profile.id,
        model_revision: profile.revision,
        account: account.clone(),
        external_model_id: profile.external_model_id.clone(),
        display_name: profile.display_name.clone(),
        settings: profile.config.stable_diffusion.clone(),
        text_output: capabilities.output_modalities.text == CapabilityStatus::Supported,
    })
}
