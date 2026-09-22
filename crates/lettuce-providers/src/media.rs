//! The media bytes a provider request may inline. The application supplies
//! the source; adapters only see a MIME type and bytes.

use lettuce_types::AssetId;

#[derive(Clone, PartialEq, Eq)]
pub struct ProviderMedia {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

impl std::fmt::Debug for ProviderMedia {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderMedia")
            .field("mime_type", &self.mime_type)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProviderMediaError {
    #[error("media asset is unavailable")]
    Unavailable,
}

pub trait ProviderMediaSource: Send + Sync {
    fn load(&self, asset_id: AssetId) -> Result<ProviderMedia, ProviderMediaError>;
}
