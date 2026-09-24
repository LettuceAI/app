pub(crate) mod avatar_gradient;
pub(crate) mod avatar_prompt;
pub(crate) mod design_reference_writer;
pub(crate) mod image_feature_models;
pub(crate) mod image_generation;
pub(crate) mod image_providers;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) mod local_diffusion;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) mod local_diffusion_install;
pub(crate) mod playground;
pub(crate) mod provider_media;
pub(crate) mod reply_images;
pub(crate) mod scene_image;
pub(crate) mod scene_loras;
pub(crate) mod scene_prompt_writer;

pub use avatar_gradient::{AvatarGradient, AvatarGradientError, AvatarGradients, GradientColor};
pub use avatar_prompt::{AvatarPromptError, AvatarPromptRequest, avatar_image_prompt};
pub use design_reference_writer::{
    DesignReferenceError, DesignReferenceReply, DesignReferenceRequest, DesignReferenceSources,
    DesignReferenceWriter,
};
pub use image_feature_models::*;
pub use image_generation::*;
pub use image_providers::AppImageProviders;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use local_diffusion_install::*;
pub use provider_media::*;
pub use reply_images::{ReplyImageFacts, SceneImageFollowUp};
pub use scene_image::{SceneImageError, SceneImageRequest, generate_scene_image};
pub use scene_prompt_writer::{
    ScenePromptError, ScenePromptReply, ScenePromptRequest, ScenePromptSources, ScenePromptWriter,
};
