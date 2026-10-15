use lettuce_contracts::{self as dto, ApiError};
use crate::api::{ApiContext, error::IntoApiError};

pub async fn library_counts(context: &ApiContext) -> Result<dto::LibraryCounts, ApiError> {
    context.blocking(|context| {
        let counts = context.backend().database().library_counts().map_err(IntoApiError::into_api_error)?;
        Ok(dto::LibraryCounts { characters: counts.characters, personas: counts.personas, lorebooks: counts.lorebooks, images: counts.images, audio: counts.audio })
    }).await
}
