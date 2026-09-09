use std::collections::{BTreeMap, HashSet};

use async_trait::async_trait;
use lettuce_settings::SecretValue;
use lettuce_types::{AudioProviderId, TimestampMillis};

use crate::AudioProvider;

pub const MAX_DISCOVERED_VOICES: usize = 1_000;
const MAX_ID_BYTES: usize = 4_096;
const MAX_NAME_BYTES: usize = 256;
const MAX_URL_BYTES: usize = 4_096;
const MAX_LABELS: usize = 64;
const MAX_LABEL_KEY_BYTES: usize = 128;
const MAX_LABEL_VALUE_BYTES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredVoiceDraft {
    pub voice_id: String,
    pub name: String,
    pub preview_url: Option<String>,
    pub labels: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredVoice {
    pub provider_id: AudioProviderId,
    pub ordinal: u32,
    pub voice_id: String,
    pub name: String,
    pub preview_url: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub cached_at: TimestampMillis,
}

impl DiscoveredVoiceDraft {
    pub fn validate(&self) -> Result<(), VoiceDiscoveryError> {
        validate_text(&self.voice_id, MAX_ID_BYTES, false)?;
        validate_text(&self.name, MAX_NAME_BYTES, false)?;
        if let Some(url) = &self.preview_url {
            validate_text(url, MAX_URL_BYTES, false)?;
        }
        if self.labels.len() > MAX_LABELS {
            return Err(VoiceDiscoveryError::InvalidData);
        }
        for (key, value) in &self.labels {
            validate_text(key, MAX_LABEL_KEY_BYTES, false)?;
            validate_text(value, MAX_LABEL_VALUE_BYTES, true)?;
        }
        Ok(())
    }
}

impl DiscoveredVoice {
    pub fn validate(&self) -> Result<(), VoiceDiscoveryError> {
        DiscoveredVoiceDraft {
            voice_id: self.voice_id.clone(),
            name: self.name.clone(),
            preview_url: self.preview_url.clone(),
            labels: self.labels.clone(),
        }
        .validate()
    }
}

pub fn materialize_discovered_voices(
    provider_id: AudioProviderId,
    drafts: Vec<DiscoveredVoiceDraft>,
    cached_at: TimestampMillis,
) -> Result<Vec<DiscoveredVoice>, VoiceDiscoveryError> {
    if drafts.len() > MAX_DISCOVERED_VOICES {
        return Err(VoiceDiscoveryError::InvalidData);
    }
    let mut ids = HashSet::with_capacity(drafts.len());
    drafts
        .into_iter()
        .enumerate()
        .map(|(ordinal, draft)| {
            draft.validate()?;
            if !ids.insert(draft.voice_id.clone()) {
                return Err(VoiceDiscoveryError::InvalidData);
            }
            Ok(DiscoveredVoice {
                provider_id,
                ordinal: u32::try_from(ordinal).map_err(|_| VoiceDiscoveryError::InvalidData)?,
                voice_id: draft.voice_id,
                name: draft.name,
                preview_url: draft.preview_url,
                labels: draft.labels,
                cached_at,
            })
        })
        .collect()
}

#[async_trait]
pub trait VoiceDiscovery: Send + Sync {
    async fn fetch_configured_voices(
        &self,
        provider: &AudioProvider,
        credential: &SecretValue,
    ) -> Result<Vec<DiscoveredVoiceDraft>, VoiceDiscoveryError>;
}

pub trait DiscoveredVoiceRepository: Send + Sync {
    fn replace_discovered_voices(
        &self,
        provider_id: AudioProviderId,
        voices: Vec<DiscoveredVoice>,
    ) -> Result<Vec<DiscoveredVoice>, VoiceDiscoveryRepositoryError>;

    fn list_discovered_voices(
        &self,
        provider_id: AudioProviderId,
    ) -> Result<Vec<DiscoveredVoice>, VoiceDiscoveryRepositoryError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum VoiceDiscoveryError {
    #[error("voice discovery input or response is invalid")]
    InvalidData,
    #[error("voice discovery is temporarily unavailable")]
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum VoiceDiscoveryRepositoryError {
    #[error("TTS provider does not exist")]
    ProviderMissing,
    #[error("discovered voice data is invalid")]
    InvalidData,
    #[error("discovered voice storage failed")]
    Storage,
}

fn validate_text(
    value: &str,
    max_bytes: usize,
    allow_empty: bool,
) -> Result<(), VoiceDiscoveryError> {
    if value.len() > max_bytes || value.contains('\0') || (!allow_empty && value.is_empty()) {
        return Err(VoiceDiscoveryError::InvalidData);
    }
    Ok(())
}
