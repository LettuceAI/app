//! The labels a chat feature job stores in its error when the user can act
//! on why it failed; the API turns them into `JobFailureReason`.

pub(crate) const HELP_ME_REPLY_DISABLED: &str = "reply-helper-disabled";
pub(crate) const HELP_ME_REPLY_NO_HISTORY: &str = "reply-helper-no-history";
pub(crate) const HELP_ME_REPLY_NO_MODEL: &str = "reply-helper-no-model";
pub(crate) const HELP_ME_REPLY_NO_REPLY: &str = "reply-helper-no-reply";
pub(crate) const SCENE_PROMPT_DISABLED: &str = "scene-prompt-disabled";
pub(crate) const SCENE_PROMPT_NO_MODEL: &str = "scene-prompt-no-model";
pub(crate) const SCENE_PROMPT_NO_REPLY: &str = "scene-prompt-no-reply";
pub(crate) const SCENE_IMAGE_DISABLED: &str = "scene-image-disabled";
pub(crate) const SCENE_IMAGE_NO_MODEL: &str = "scene-image-no-model";
pub(crate) const SCENE_IMAGE_NO_IMAGE: &str = "scene-image-no-image";
pub(crate) const RESULT_STORAGE_FAILED: &str = "feature-result-storage-failed";
