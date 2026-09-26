use lettuce_models::{StableDiffusionLora, StableDiffusionSettings};
use lettuce_types::{
    AssetId, CharacterId, ContentHash, ConversationId, JobId, ModelProfileId, RequestId,
    TimestampMillis,
};
use serde::{Deserialize, Serialize};

pub const MAX_IMAGE_COUNT: u32 = 10;
pub const MAX_INPUT_IMAGES: usize = 16;
pub const MAX_PROMPT_BYTES: usize = 64 * 1024;
const MAX_OPTION_BYTES: usize = 64;

/// Which part of the app asked for the image, recorded as the usage source;
/// direct requests have no source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageGenerationSource {
    Direct,
    Scene,
    Playground,
    CreationHelper,
}

/// Where the image belongs for usage reporting; a generic image generation
/// owner is used when neither is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageAttribution {
    #[serde(default)]
    pub conversation_id: Option<ConversationId>,
    #[serde(default)]
    pub character_id: Option<CharacterId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ImageOutputPolicy {
    Retained,
    Preview { expires_at: TimestampMillis },
}

/// One image generation. `settings` are per-request settings laid over the
/// model's own; an empty `settings.base_loras` drops the model's base LoRAs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageGenerationRequest {
    pub id: RequestId,
    pub model_profile_id: ModelProfileId,
    pub prompt: String,
    #[serde(default)]
    pub settings: StableDiffusionSettings,
    #[serde(default)]
    pub input_images: Vec<AssetId>,
    #[serde(default)]
    pub mask_image: Option<AssetId>,
    #[serde(default)]
    pub loras: Vec<StableDiffusionLora>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub quality: Option<String>,
    #[serde(default)]
    pub style: Option<String>,
    pub count: u32,
    pub source: ImageGenerationSource,
    #[serde(default)]
    pub attribution: ImageAttribution,
    pub output_policy: ImageOutputPolicy,
    pub created_at: TimestampMillis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ImageRequestValidationError {
    #[error("Prompt cannot be empty")]
    EmptyPrompt,
    #[error("image prompt is too long")]
    PromptTooLong,
    #[error("image count must be between 1 and {MAX_IMAGE_COUNT}")]
    Count,
    #[error("at most {MAX_INPUT_IMAGES} input images are allowed")]
    TooManyInputImages,
    #[error("image request field {0} is invalid")]
    Field(&'static str),
    #[error("image request settings are invalid")]
    Settings,
    #[error("image generation record is invalid")]
    Record,
}

impl ImageGenerationRequest {
    pub fn validate(&self) -> Result<(), ImageRequestValidationError> {
        if self.prompt.trim().is_empty() {
            return Err(ImageRequestValidationError::EmptyPrompt);
        }
        if self.prompt.len() > MAX_PROMPT_BYTES {
            return Err(ImageRequestValidationError::PromptTooLong);
        }
        if !(1..=MAX_IMAGE_COUNT).contains(&self.count) {
            return Err(ImageRequestValidationError::Count);
        }
        if self.input_images.len() > MAX_INPUT_IMAGES {
            return Err(ImageRequestValidationError::TooManyInputImages);
        }
        for (field, value) in [
            ("size", &self.size),
            ("quality", &self.quality),
            ("style", &self.style),
        ] {
            if value.as_deref().is_some_and(|value| {
                value.trim() != value || value.is_empty() || value.len() > MAX_OPTION_BYTES
            }) {
                return Err(ImageRequestValidationError::Field(field));
            }
        }
        if self.settings.cpp != lettuce_models::StableDiffusionCppBinding::default() {
            return Err(ImageRequestValidationError::Field("settings.cpp"));
        }
        self.settings
            .validate()
            .map_err(|_| ImageRequestValidationError::Settings)?;
        if self.loras.iter().any(|lora| {
            lora.path.trim().is_empty()
                || !lora.multiplier.is_finite()
                || !(0.0..=2.0).contains(&lora.multiplier)
        }) {
            return Err(ImageRequestValidationError::Field("loras"));
        }
        if let ImageOutputPolicy::Preview { expires_at } = self.output_policy
            && expires_at <= self.created_at
        {
            return Err(ImageRequestValidationError::Field("output_policy"));
        }
        Ok(())
    }
}

/// One stored output image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedImage {
    pub asset_id: AssetId,
    pub content_hash: ContentHash,
    pub mime_type: String,
    pub byte_size: u64,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    /// Text the provider returned with the image.
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageGenerationResult {
    pub request_id: RequestId,
    pub images: Vec<GeneratedImage>,
    /// Provider outputs that were not valid images and were dropped.
    #[serde(default)]
    pub rejected_outputs: u32,
    pub completed_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ImageGenerationState {
    Pending,
    Succeeded {
        result: ImageGenerationResult,
    },
    Failed {
        message: String,
        completed_at: TimestampMillis,
    },
    Cancelled {
        completed_at: TimestampMillis,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageGenerationRecord {
    pub job_id: JobId,
    pub request: ImageGenerationRequest,
    pub state: ImageGenerationState,
}

impl ImageGenerationRecord {
    pub fn validate(&self) -> Result<(), ImageRequestValidationError> {
        self.request.validate()?;
        let completed_at = match &self.state {
            ImageGenerationState::Pending => return Ok(()),
            ImageGenerationState::Succeeded { result } => {
                if result.request_id != self.request.id || result.images.is_empty() {
                    return Err(ImageRequestValidationError::Record);
                }
                result.completed_at
            }
            ImageGenerationState::Failed {
                message,
                completed_at,
            } => {
                if message.trim().is_empty() {
                    return Err(ImageRequestValidationError::Record);
                }
                *completed_at
            }
            ImageGenerationState::Cancelled { completed_at } => *completed_at,
        };
        if completed_at < self.request.created_at {
            return Err(ImageRequestValidationError::Record);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ImageGenerationRepositoryError {
    #[error("image generation was not found")]
    NotFound,
    #[error("image generation conflicts with the stored record")]
    Conflict,
    #[error("stored image generation data are invalid")]
    InvalidData,
    #[error("image generation storage failed")]
    Storage,
}

pub trait ImageGenerationRepository: Send + Sync {
    fn admit(
        &self,
        record: ImageGenerationRecord,
    ) -> Result<ImageGenerationRecord, ImageGenerationRepositoryError>;
    fn get(&self, job_id: JobId) -> Result<ImageGenerationRecord, ImageGenerationRepositoryError>;
    /// Moves a pending record to its terminal state once; an identical retry
    /// returns the stored record.
    fn settle(
        &self,
        job_id: JobId,
        state: ImageGenerationState,
    ) -> Result<ImageGenerationRecord, ImageGenerationRepositoryError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ImageGenerationRequest {
        ImageGenerationRequest {
            id: RequestId::new(),
            model_profile_id: ModelProfileId::new(),
            prompt: "a lighthouse".to_owned(),
            settings: StableDiffusionSettings::default(),
            input_images: Vec::new(),
            mask_image: None,
            loras: Vec::new(),
            size: Some("1024x1024".to_owned()),
            quality: None,
            style: None,
            count: 1,
            source: ImageGenerationSource::Direct,
            attribution: ImageAttribution::default(),
            output_policy: ImageOutputPolicy::Retained,
            created_at: TimestampMillis::new(10),
        }
    }

    #[test]
    fn requests_are_bounded_and_keep_the_legacy_empty_prompt_message() {
        assert_eq!(request().validate(), Ok(()));
        let blank = ImageGenerationRequest {
            prompt: "  ".to_owned(),
            ..request()
        };
        assert_eq!(
            blank.validate().map_err(|error| error.to_string()),
            Err("Prompt cannot be empty".to_owned())
        );
        for count in [0, MAX_IMAGE_COUNT + 1] {
            let invalid = ImageGenerationRequest { count, ..request() };
            assert_eq!(invalid.validate(), Err(ImageRequestValidationError::Count));
        }
        let padded = ImageGenerationRequest {
            size: Some(" 512x512".to_owned()),
            ..request()
        };
        assert_eq!(
            padded.validate(),
            Err(ImageRequestValidationError::Field("size"))
        );
        let mut bound = request();
        bound.settings.cpp.profile_id = Some("z-image".to_owned());
        assert_eq!(
            bound.validate(),
            Err(ImageRequestValidationError::Field("settings.cpp"))
        );
        let bad_lora = ImageGenerationRequest {
            loras: vec![StableDiffusionLora {
                path: " ".to_owned(),
                multiplier: 1.0,
                is_high_noise: false,
                keywords: Vec::new(),
            }],
            ..request()
        };
        assert_eq!(
            bad_lora.validate(),
            Err(ImageRequestValidationError::Field("loras"))
        );
        let blank_keyword = ImageGenerationRequest {
            loras: vec![StableDiffusionLora {
                path: "style.safetensors".to_owned(),
                multiplier: 1.0,
                is_high_noise: false,
                keywords: vec![" ".to_owned()],
            }],
            ..request()
        };
        assert_eq!(blank_keyword.validate(), Ok(()));
    }

    #[test]
    fn records_round_trip_and_terminal_states_must_fit_the_request() {
        let request = request();
        let record = ImageGenerationRecord {
            job_id: JobId::new(),
            request: request.clone(),
            state: ImageGenerationState::Failed {
                message: "API error 400: bad".to_owned(),
                completed_at: TimestampMillis::new(20),
            },
        };
        assert_eq!(record.validate(), Ok(()));
        let encoded = serde_json::to_string(&record).expect("record JSON");
        assert_eq!(
            serde_json::from_str::<ImageGenerationRecord>(&encoded).expect("record reads"),
            record
        );
        let empty = ImageGenerationRecord {
            state: ImageGenerationState::Succeeded {
                result: ImageGenerationResult {
                    request_id: request.id,
                    images: Vec::new(),
                    rejected_outputs: 1,
                    completed_at: TimestampMillis::new(20),
                },
            },
            ..record.clone()
        };
        assert_eq!(empty.validate(), Err(ImageRequestValidationError::Record));
        let early = ImageGenerationRecord {
            state: ImageGenerationState::Cancelled {
                completed_at: TimestampMillis::new(1),
            },
            ..record
        };
        assert_eq!(early.validate(), Err(ImageRequestValidationError::Record));
    }
}
