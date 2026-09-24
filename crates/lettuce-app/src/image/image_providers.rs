use std::sync::Arc;

use async_trait::async_trait;
use lettuce_image_generation::{
    ImageProviderError, ImageProviderPort, LOCAL_DIFFUSION_PROVIDER_KIND, ProviderImageOutput,
    ProviderImageRequest,
};
use lettuce_models::ProviderProtocol;
use lettuce_network::{BulkHttpClient, JsonClientError, TlsPolicy};
use lettuce_providers::RemoteImageProviders;
use lettuce_settings::SecretStore;

/// Routes an image request to the provider that serves its account: the
/// embedded stable-diffusion.cpp engine for sdcpp accounts, the remote
/// adapters for every other kind.
#[derive(Clone)]
pub struct AppImageProviders {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(crate) local:
        Option<Arc<lettuce_image_generation::sd_runtime::server::LocalDiffusionEngine>>,
    pub(crate) remote: Option<Arc<dyn ImageProviderPort>>,
}

impl std::fmt::Debug for AppImageProviders {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = formatter.debug_struct("AppImageProviders");
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        debug.field("local", &self.local.is_some());
        debug.field("remote", &self.remote.is_some()).finish()
    }
}

#[async_trait]
impl ImageProviderPort for AppImageProviders {
    async fn generate(
        &self,
        request: ProviderImageRequest,
    ) -> Result<ProviderImageOutput, ImageProviderError> {
        let local = request.account.protocol == ProviderProtocol::StableDiffusion
            && request
                .account
                .provider_kind
                .eq_ignore_ascii_case(LOCAL_DIFFUSION_PROVIDER_KIND);
        if local {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            if let Some(engine) = &self.local {
                return engine.generate(request).await;
            }
            return Err(ImageProviderError::Unsupported(
                request.account.provider_kind.clone(),
            ));
        }
        match &self.remote {
            Some(remote) => remote.generate(request).await,
            None => Err(ImageProviderError::Unsupported(
                request.account.provider_kind.clone(),
            )),
        }
    }
}

impl crate::AppBackend {
    /// Every image provider the app serves, with remote requests reading
    /// their keys from `secret_store` under the host's TLS policy.
    pub fn image_providers<S>(
        &self,
        secret_store: Arc<S>,
        tls_policy: &TlsPolicy,
    ) -> Result<AppImageProviders, JsonClientError>
    where
        S: SecretStore + ?Sized + 'static,
    {
        Ok(AppImageProviders {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            local: self.local_diffusion.clone(),
            remote: Some(Arc::new(RemoteImageProviders::new(
                secret_store,
                BulkHttpClient::with_tls(tls_policy)?,
            ))),
        })
    }
}
