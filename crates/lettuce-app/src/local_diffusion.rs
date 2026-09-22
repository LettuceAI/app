//! The embedded stable-diffusion.cpp engine composed with the llama.cpp
//! runtime: each gives way to the other like legacy (the image server
//! unloads llama.cpp before it starts, llama.cpp stops the image server
//! before it runs).

use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use lettuce_image_generation::sd_runtime::layout::DiffusionPaths;
use lettuce_image_generation::sd_runtime::output::GenerationProgressSink;
use lettuce_image_generation::sd_runtime::policy::HardwareGpu;
use lettuce_image_generation::sd_runtime::server::{EngineHost, LocalDiffusionEngine};
use lettuce_image_generation::{
    ImageProviderError, ImageProviderPort, LOCAL_DIFFUSION_PROVIDER_KIND, ProviderImageOutput,
    ProviderImageRequest,
};
use lettuce_models::ProviderProtocol;
use lettuce_providers::{LocalLlama, LocalRuntimeExclusion};

pub(crate) type SharedLocalLlama = Arc<OnceLock<Option<LocalLlama>>>;

pub(crate) async fn unload_started_llama(local_llama: &SharedLocalLlama) -> Result<(), String> {
    let Some(local_llama) = local_llama.get().cloned().flatten() else {
        return Ok(());
    };
    let (sender, receiver) = tokio::sync::oneshot::channel();
    local_llama.runtime().unload(Box::new(move |result| {
        let _ = sender.send(result);
    }));
    receiver
        .await
        .map_err(|_| "llama.cpp inference worker dropped its response".to_owned())?
        .map_err(|error| error.to_string())
}

struct AppEngineHost {
    local_llama: SharedLocalLlama,
}

#[async_trait]
impl EngineHost for AppEngineHost {
    async fn gpu_devices(&self) -> Result<Vec<HardwareGpu>, String> {
        let devices = tokio::task::spawn_blocking(lettuce_local_llm::hardware::list_gpu_devices)
            .await
            .map_err(|_| "Failed to read GPU memory information.".to_owned())?;
        Ok(devices
            .into_iter()
            .map(|device| HardwareGpu {
                index: device.index,
                name: device.name,
                description: device.description,
                memory_total: device.memory_total,
                memory_free: device.memory_free,
            })
            .collect())
    }

    fn available_memory_bytes(&self) -> Option<u64> {
        lettuce_local_llm::hardware::get_available_memory_bytes()
    }

    async fn unload_local_llm(&self) -> Result<(), String> {
        unload_started_llama(&self.local_llama).await
    }
}

pub(crate) struct DiffusionExclusion(pub(crate) Arc<LocalDiffusionEngine>);

#[async_trait]
impl LocalRuntimeExclusion for DiffusionExclusion {
    async fn before_local_llama(&self) -> Result<(), String> {
        self.0.stop_for_llama().await
    }
}

pub(crate) fn start_engine(
    paths: DiffusionPaths,
    progress: Arc<dyn GenerationProgressSink>,
    local_llama: SharedLocalLlama,
) -> Result<Arc<LocalDiffusionEngine>, lettuce_network::JsonClientError> {
    Ok(Arc::new(LocalDiffusionEngine::new(
        paths,
        lettuce_network::BulkHttpClient::new()?,
        Arc::new(AppEngineHost { local_llama }),
        progress,
    )))
}

/// Routes an image request to the provider that serves its account.
#[derive(Clone)]
pub struct AppImageProviders {
    pub(crate) local: Option<Arc<LocalDiffusionEngine>>,
}

impl std::fmt::Debug for AppImageProviders {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AppImageProviders")
            .field("local", &self.local.is_some())
            .finish()
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
        match (&self.local, local) {
            (Some(engine), true) => engine.generate(request).await,
            _ => Err(ImageProviderError::Unsupported(
                request.account.provider_kind.clone(),
            )),
        }
    }
}

impl crate::AppBackend {
    /// The embedded stable-diffusion.cpp engine, when the host configured
    /// its folders.
    #[must_use]
    pub fn local_diffusion(&self) -> Option<&Arc<LocalDiffusionEngine>> {
        self.local_diffusion.as_ref()
    }

    #[must_use]
    pub fn image_providers(&self) -> AppImageProviders {
        AppImageProviders {
            local: self.local_diffusion.clone(),
        }
    }
}
