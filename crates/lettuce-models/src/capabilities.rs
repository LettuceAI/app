use serde::{Deserialize, Serialize};

use lettuce_types::TimestampMillis;

use crate::Modality;

pub const MODEL_CAPABILITIES_FORMAT_VERSION: u32 = 1;

/// A capability is deliberately three-valued.  A missing probe or catalog
/// record is not evidence that the provider does not support a feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    Supported,
    Unsupported,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityEvidenceSource {
    Unspecified,
    ProviderReported,
    Catalog,
    Probed,
    UserOverride,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityEvidence {
    pub source: CapabilityEvidenceSource,
    pub source_version: u32,
    pub observed_at: TimestampMillis,
}

impl Default for CapabilityEvidence {
    fn default() -> Self {
        Self {
            source: CapabilityEvidenceSource::Unspecified,
            source_version: 0,
            observed_at: TimestampMillis::new(0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModalityCapabilities {
    pub text: CapabilityStatus,
    pub image: CapabilityStatus,
    pub audio: CapabilityStatus,
}

impl Default for ModalityCapabilities {
    fn default() -> Self {
        Self {
            text: CapabilityStatus::Unknown,
            image: CapabilityStatus::Unknown,
            audio: CapabilityStatus::Unknown,
        }
    }
}

impl ModalityCapabilities {
    pub const fn unknown() -> Self {
        Self {
            text: CapabilityStatus::Unknown,
            image: CapabilityStatus::Unknown,
            audio: CapabilityStatus::Unknown,
        }
    }

    pub const fn get(self, modality: Modality) -> CapabilityStatus {
        match modality {
            Modality::Text => self.text,
            Modality::Image => self.image,
            Modality::Audio => self.audio,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCapabilities {
    pub format_version: u32,
    pub evidence: CapabilityEvidence,
    pub input_modalities: ModalityCapabilities,
    pub output_modalities: ModalityCapabilities,
    pub streaming: CapabilityStatus,
    pub tools: CapabilityStatus,
    pub structured_output: CapabilityStatus,
    pub reasoning: CapabilityStatus,
    pub prompt_cache: CapabilityStatus,
    /// `None` means the source did not establish a limit, not zero.
    pub context_length: Option<u32>,
    /// `None` means the source did not establish a limit, not zero.
    pub max_visible_output_tokens: Option<u32>,
    /// Total completion allowance, including hidden reasoning tokens.
    pub max_total_completion_tokens: Option<u32>,
    pub parameter_support: ParameterSupport,
}

impl Default for ModelCapabilities {
    fn default() -> Self {
        Self::unknown(CapabilityEvidence::default())
    }
}

impl ModelCapabilities {
    pub fn unknown(evidence: CapabilityEvidence) -> Self {
        Self {
            format_version: MODEL_CAPABILITIES_FORMAT_VERSION,
            evidence,
            input_modalities: ModalityCapabilities::unknown(),
            output_modalities: ModalityCapabilities::unknown(),
            streaming: CapabilityStatus::Unknown,
            tools: CapabilityStatus::Unknown,
            structured_output: CapabilityStatus::Unknown,
            reasoning: CapabilityStatus::Unknown,
            prompt_cache: CapabilityStatus::Unknown,
            context_length: None,
            max_visible_output_tokens: None,
            max_total_completion_tokens: None,
            parameter_support: ParameterSupport::default(),
        }
    }

    pub fn validate(&self) -> Result<(), CapabilityValidationError> {
        if self.format_version != MODEL_CAPABILITIES_FORMAT_VERSION {
            return Err(CapabilityValidationError::UnsupportedFormatVersion);
        }
        if ((self.evidence.source == CapabilityEvidenceSource::Unspecified)
            != (self.evidence.source_version == 0))
            || self.context_length == Some(0)
            || self.max_visible_output_tokens == Some(0)
            || self.max_total_completion_tokens == Some(0)
            || matches!(
                (
                    self.max_visible_output_tokens,
                    self.max_total_completion_tokens
                ),
                (Some(visible), Some(total)) if total < visible
            )
        {
            return Err(CapabilityValidationError::InvalidEvidenceOrLimit);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ParameterSupport {
    pub temperature: CapabilityStatus,
    pub top_p: CapabilityStatus,
    pub top_k: CapabilityStatus,
    pub frequency_penalty: CapabilityStatus,
    pub presence_penalty: CapabilityStatus,
    pub repetition_penalty: CapabilityStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CapabilityValidationError {
    #[error("capability document format version is unsupported")]
    UnsupportedFormatVersion,
    #[error("capability document contains invalid evidence or limits")]
    InvalidEvidenceOrLimit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_explicitly_unknown_and_empty_documents_are_not_accepted() {
        let capabilities = ModelCapabilities::default();
        assert_eq!(
            capabilities.evidence.source,
            CapabilityEvidenceSource::Unspecified
        );
        assert!(capabilities.validate().is_ok());
        assert!(serde_json::from_str::<ModelCapabilities>("{}").is_err());
    }

    #[test]
    fn evidence_and_output_limits_are_coherent() {
        let mut capabilities = ModelCapabilities::default();
        capabilities.evidence.source_version = 1;
        assert_eq!(
            capabilities.validate(),
            Err(CapabilityValidationError::InvalidEvidenceOrLimit)
        );

        capabilities.evidence.source = CapabilityEvidenceSource::Catalog;
        capabilities.max_visible_output_tokens = Some(2_000);
        capabilities.max_total_completion_tokens = Some(1_000);
        assert_eq!(
            capabilities.validate(),
            Err(CapabilityValidationError::InvalidEvidenceOrLimit)
        );
    }
}
