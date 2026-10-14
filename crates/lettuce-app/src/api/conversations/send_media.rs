use std::collections::HashSet;

use lettuce_contracts::{ApiError, ApiErrorCode, ApiErrorDetails};
use lettuce_conversations::Conversation;
use lettuce_database::Database;
use lettuce_media::{AssetKind, MediaAssetRepository};
use lettuce_models::{CapabilityStatus, ModelProfileRepository};
use lettuce_types::{AssetId, ModelProfileId};

use super::super::error::{IntoApiError, api_error, invalid_field, parse_id};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AttachmentKind {
    Image,
    Audio,
}

impl AttachmentKind {
    const fn field(self) -> &'static str {
        match self {
            Self::Image => "media.image",
            Self::Audio => "media.audio",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Audio => "audio",
        }
    }
}

pub(super) fn parse_media(media: &[String]) -> Result<Vec<AssetId>, ApiError> {
    let mut seen = HashSet::new();
    let mut ids = Vec::with_capacity(media.len());
    for value in media {
        let id: AssetId = parse_id(value, "media")?;
        if !seen.insert(id) {
            return Err(invalid_field("media", "an attachment is listed twice"));
        }
        ids.push(id);
    }
    Ok(ids)
}

pub(super) fn attachment_kinds(
    database: &Database,
    ids: &[AssetId],
) -> Result<Vec<AttachmentKind>, ApiError> {
    ids.iter()
        .map(|id| {
            let asset = MediaAssetRepository::get(database, *id)
                .map_err(|_| api_error(ApiErrorCode::Internal, "the attachment could not be read"))?
                .ok_or_else(|| invalid_field("media", "an attachment does not exist"))?;
            match asset.kind {
                AssetKind::MessageImage => Ok(AttachmentKind::Image),
                AssetKind::MessageAudio => Ok(AttachmentKind::Audio),
                _ => Err(invalid_field(
                    "media",
                    "an asset is not a message attachment",
                )),
            }
        })
        .collect()
}

pub(super) fn require_input_modalities(
    database: &Database,
    conversation: &Conversation,
    kinds: &[AttachmentKind],
) -> Result<(), ApiError> {
    let default_model = lettuce_settings::GlobalSettingsStore::load(database)
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
        .default_model_profile_id;
    let speakers =
        crate::generation::live_sources::speaker_models(database, conversation, default_model)
            .map_err(IntoApiError::into_api_error)?;
    let mut models: Vec<Option<ModelProfileId>> = Vec::new();
    for model in speakers {
        if !models.contains(&model) {
            models.push(model);
        }
    }
    if models.is_empty() && !kinds.is_empty() {
        return Err(ApiError {
            code: ApiErrorCode::Unsupported,
            message: "the chat has no member that can receive attachments".into(),
            details: Some(ApiErrorDetails::InvalidField {
                field: "media.speakers".into(),
            }),
        });
    }
    for kind in [AttachmentKind::Image, AttachmentKind::Audio] {
        if !kinds.contains(&kind) {
            continue;
        }
        for model in &models {
            let supported = match model {
                Some(id) => ModelProfileRepository::get(database, *id)
                    .map_err(|_| api_error(ApiErrorCode::Internal, "the model could not be read"))?
                    .is_some_and(|profile| {
                        let input = profile.config.capabilities.input_modalities;
                        let status = match kind {
                            AttachmentKind::Image => input.image,
                            AttachmentKind::Audio => input.audio,
                        };
                        status == CapabilityStatus::Supported
                    }),
                None => false,
            };
            if !supported {
                return Err(ApiError {
                    code: ApiErrorCode::Unsupported,
                    message: format!("the chat's model does not accept {} input", kind.label()),
                    details: Some(ApiErrorDetails::InvalidField {
                        field: kind.field().to_owned(),
                    }),
                });
            }
        }
    }
    Ok(())
}
