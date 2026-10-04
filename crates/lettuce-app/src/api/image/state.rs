use std::sync::{Mutex, MutexGuard};

use lettuce_contracts::{ApiError, ApiErrorCode};

use super::ApiContext;
use crate::api::error::api_error;
use crate::{AvatarGradients, CivitaiBrowser};

/// The image API's per-process state: the avatar gradients computed so far
/// and where CivitAI is reached.
pub(crate) struct ImageApiState {
    gradients: AvatarGradients,
    civitai_endpoint: Mutex<String>,
    bundle_admission: tokio::sync::Mutex<()>,
}

impl Default for ImageApiState {
    fn default() -> Self {
        Self {
            gradients: AvatarGradients::default(),
            civitai_endpoint: Mutex::new(lettuce_image_generation::CIVITAI_API_ENDPOINT.to_owned()),
            bundle_admission: tokio::sync::Mutex::new(()),
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl ImageApiState {
    /// Held from the running-install check of an image bundle install through
    /// its job's admission, so a second install sees the first one's job.
    pub(crate) const fn bundle_admission(&self) -> &tokio::sync::Mutex<()> {
        &self.bundle_admission
    }

    pub(crate) const fn gradients(&self) -> &AvatarGradients {
        &self.gradients
    }

    /// A CivitAI browser over the device's trusted certificates.
    pub(crate) fn civitai(&self, context: &ApiContext) -> Result<CivitaiBrowser, ApiError> {
        let tls = context
            .backend()
            .tls_policy()
            .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
        let client = lettuce_network::BulkHttpClient::with_tls(&tls)
            .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?;
        Ok(CivitaiBrowser::with_endpoint(
            client,
            lock(&self.civitai_endpoint).clone(),
        ))
    }

    #[cfg(test)]
    pub(crate) fn use_civitai_endpoint(&self, endpoint: String) {
        *lock(&self.civitai_endpoint) = endpoint;
    }
}
