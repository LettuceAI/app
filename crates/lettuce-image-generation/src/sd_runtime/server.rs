//! The managed sd-server process and local generation, run as legacy ran
//! them: one server reused while its model, engine build and compute policy
//! stay the same, a native job API polled every 500 ms, and one retry with
//! CPU-offloaded weights after an out-of-memory failure.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use base64::Engine as _;
use lettuce_models::{ProviderProtocol, StableDiffusionCppBinding};
use lettuce_network::{BulkHttpClient, JsonAuth};
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};

use super::fit::{
    DeviceSource, FitDevice, FitEstimate, compute_auto_fit_estimate, file_components,
    manual_backend_specs,
};
use super::layout::{
    ActiveRuntime, DiffusionPaths, InstalledRuntime, effective_active_runtime, installed_runtimes,
    load_compute_policy, runtime_is_installed, server_executable_name,
};
use super::loras::normalize_loras;
use super::output::{
    GenerationProgress, GenerationProgressSink, OutputSegment, OutputSplitter, ProgressThrottle,
    RuntimeOutput, oom_signature_present,
};
use super::payload::{EngineGenerationInput, EngineModelLimits, build_generation_payload};
use super::policy::{
    ComputePolicy, HardwareGpu, RuntimeDevice, apply_policy_budgets, devices_for_policy,
    match_runtime_devices, max_vram_spec, parse_runtime_devices, validate_compute_policy,
};
use super::releases::{RuntimePlatform, runtime_backend};
use crate::{
    ImageInput, ImageProviderError, ImageProviderPort, LOCAL_DIFFUSION_PROVIDER_KIND,
    ProviderImage, ProviderImageOutput, ProviderImageRequest, diffusion_catalog,
};

pub const GENERATION_CANCELLED_MESSAGE: &str = "Local image generation was cancelled.";
const DESKTOP_ONLY_MESSAGE: &str = "Local stable-diffusion.cpp image generation is desktop-only.";
const READINESS_ATTEMPTS: u32 = 300;
const POLL_ATTEMPTS: u32 = 1_200;
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const DEVICE_QUERY_TIMEOUT: Duration = Duration::from_secs(10);

/// What the engine needs from the rest of the app.
#[async_trait]
pub trait EngineHost: Send + Sync {
    /// The system GPUs with live memory (the llama.cpp device list).
    async fn gpu_devices(&self) -> Result<Vec<HardwareGpu>, String>;
    fn available_memory_bytes(&self) -> Option<u64>;
    /// Legacy unloaded the llama.cpp model before starting the image server.
    async fn unload_local_llm(&self) -> Result<(), String>;
}

/// A registered local image model: its diffusion file and the binding the
/// installer stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineModel {
    pub display_name: String,
    pub diffusion_model_path: String,
    pub binding: StableDiffusionCppBinding,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedComputePolicy {
    pub backend: String,
    pub automatic: bool,
    pub available_devices: Vec<FitDevice>,
    pub effective_devices: Vec<FitDevice>,
}

struct ManagedServer {
    key: String,
    base_url: String,
    child: Child,
    output: Arc<RuntimeOutput>,
}

#[derive(Clone)]
struct ActiveGeneration {
    base_url: String,
    job_id: Option<String>,
    cancel: Arc<AtomicBool>,
}

enum GenerationJobError {
    Cancelled,
    Rejected(String),
    Failed(String),
    Infrastructure(String),
}

pub struct LocalDiffusionEngine {
    paths: DiffusionPaths,
    platform: RuntimePlatform,
    http: BulkHttpClient,
    host: Arc<dyn EngineHost>,
    progress: Arc<dyn GenerationProgressSink>,
    server: tokio::sync::Mutex<Option<ManagedServer>>,
    active: Mutex<Option<ActiveGeneration>>,
    shutting_down: AtomicBool,
}

impl std::fmt::Debug for LocalDiffusionEngine {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LocalDiffusionEngine")
            .field("paths", &self.paths)
            .finish_non_exhaustive()
    }
}

pub(super) fn library_path_env(
    command: &mut Command,
    runtime_dir: &Path,
    failure: &str,
) -> Result<(), String> {
    if cfg!(target_os = "linux") {
        let existing = std::env::var_os("LD_LIBRARY_PATH").unwrap_or_default();
        let mut paths = vec![runtime_dir.to_path_buf()];
        paths.extend(std::env::split_paths(&existing));
        let joined = std::env::join_paths(paths).map_err(|error| format!("{failure}: {error}"))?;
        command.env("LD_LIBRARY_PATH", joined);
    }
    Ok(())
}

fn data_url(image: &ImageInput) -> String {
    format!(
        "data:{};base64,{}",
        image.mime_type,
        base64::engine::general_purpose::STANDARD.encode(&image.bytes)
    )
}

fn split_endpoint(url: &str) -> Option<(&str, &str)> {
    let authority_start = url.find("://")? + 3;
    let path_start = url[authority_start..].find('/')? + authority_start;
    Some((&url[..path_start], &url[path_start..]))
}

fn forward_output<R>(
    stream: R,
    output: Arc<RuntimeOutput>,
    progress: Arc<dyn GenerationProgressSink>,
) where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut stream = stream;
        let mut buffer = [0_u8; 4096];
        let mut splitter = OutputSplitter::default();
        let mut throttle = ProgressThrottle::default();
        let handle = |segment: &str, throttle: &mut ProgressThrottle| match output.segment(
            segment,
            throttle,
            Instant::now(),
        ) {
            (OutputSegment::Log(line), _) => {
                tracing::info!(component = "sdcpp_runtime", "{line}");
            }
            (
                OutputSegment::Progress {
                    progress: event,
                    log,
                    ..
                },
                emit,
            ) => {
                if let Some(line) = log {
                    tracing::info!(component = "sdcpp_runtime", "{line}");
                }
                if emit {
                    progress.progress(event);
                }
            }
            (OutputSegment::Blank, _) => {}
        };
        loop {
            let read = match stream.read(&mut buffer).await {
                Ok(0) | Err(_) => break,
                Ok(read) => read,
            };
            for segment in splitter.push(&buffer[..read]) {
                handle(&segment, &mut throttle);
            }
        }
        if let Some(segment) = splitter.finish() {
            handle(&segment, &mut throttle);
        }
    });
}

impl LocalDiffusionEngine {
    #[must_use]
    pub fn new(
        paths: DiffusionPaths,
        http: BulkHttpClient,
        host: Arc<dyn EngineHost>,
        progress: Arc<dyn GenerationProgressSink>,
    ) -> Self {
        Self {
            paths,
            platform: RuntimePlatform::current(),
            http,
            host,
            progress,
            server: tokio::sync::Mutex::new(None),
            active: Mutex::new(None),
            shutting_down: AtomicBool::new(false),
        }
    }

    #[must_use]
    pub const fn paths(&self) -> &DiffusionPaths {
        &self.paths
    }

    #[must_use]
    pub const fn platform(&self) -> RuntimePlatform {
        self.platform
    }

    #[must_use]
    pub fn installed_runtimes(&self) -> Vec<InstalledRuntime> {
        installed_runtimes(&self.paths, self.platform)
    }

    /// The engine build generation uses: the saved selection, else the first
    /// installed one.
    #[must_use]
    pub fn effective_runtime(&self) -> Option<ActiveRuntime> {
        effective_active_runtime(&self.paths, &self.installed_runtimes())
    }

    /// The GPUs an engine build reports through `--list-devices`.
    pub async fn runtime_devices(
        &self,
        release: &str,
        asset: &str,
    ) -> Result<Vec<RuntimeDevice>, String> {
        let runtime_dir = self.paths.runtime_root(release, asset);
        let mut command = Command::new(runtime_dir.join(server_executable_name()));
        command
            .current_dir(&runtime_dir)
            .arg("--list-devices")
            .kill_on_drop(true);
        library_path_env(
            &mut command,
            &runtime_dir,
            "Failed to configure engine libraries",
        )?;
        let output = tokio::time::timeout(DEVICE_QUERY_TIMEOUT, command.output())
            .await
            .map_err(|_| "Timed out while asking the selected engine for its devices.".to_owned())?
            .map_err(|error| format!("Failed to query the selected engine devices: {error}"))?;
        if !output.status.success() {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(if detail.is_empty() {
                format!(
                    "The selected engine device query exited with {}.",
                    output.status
                )
            } else {
                format!("The selected engine device query failed: {detail}")
            });
        }
        Ok(parse_runtime_devices(&String::from_utf8_lossy(
            &output.stdout,
        )))
    }

    /// The engine's GPUs matched to live memory; a CPU build has none.
    pub async fn matched_runtime_devices(
        &self,
        release: &str,
        asset: &str,
    ) -> Result<Vec<FitDevice>, String> {
        let hardware = self.host.gpu_devices().await?;
        let backend = runtime_backend(asset, self.platform).ok_or_else(|| {
            "The selected engine variant is not supported on this platform.".to_owned()
        })?;
        if backend == "cpu" {
            return Ok(Vec::new());
        }
        let devices = self.runtime_devices(release, asset).await?;
        match_runtime_devices(devices, &hardware).map_err(|error| error.to_string())
    }

    pub async fn resolve_compute_policy(
        &self,
        release: &str,
        asset: &str,
        policy: &ComputePolicy,
    ) -> Result<ResolvedComputePolicy, String> {
        let backend = runtime_backend(asset, self.platform)
            .ok_or_else(|| {
                "The selected engine variant is not supported on this platform.".to_owned()
            })?
            .to_owned();
        let mut available_devices = self.matched_runtime_devices(release, asset).await?;
        validate_compute_policy(policy, &backend, &available_devices)
            .map_err(|error| error.to_string())?;
        let effective_devices = devices_for_policy(policy, available_devices.clone());
        apply_policy_budgets(policy, &mut available_devices);
        Ok(ResolvedComputePolicy {
            backend,
            automatic: policy.is_automatic(),
            available_devices,
            effective_devices,
        })
    }

    /// The placement estimate for files on disk under a resolved policy.
    #[must_use]
    pub fn estimate_files(
        &self,
        diffusion: &Path,
        text_encoder: Option<&Path>,
        vae: Option<&Path>,
        vision_encoder: Option<&Path>,
        devices: Vec<FitDevice>,
    ) -> FitEstimate {
        let size = |path: Option<&Path>| {
            path.and_then(|path| std::fs::metadata(path).ok())
                .map_or(0, |metadata| metadata.len())
        };
        compute_auto_fit_estimate(
            &file_components(
                size(Some(diffusion)),
                size(text_encoder),
                size(vae),
                size(vision_encoder),
            ),
            devices,
            self.host.available_memory_bytes(),
            DeviceSource::ConfiguredEnginePolicy,
        )
    }

    fn runtime_for(&self, binding: &StableDiffusionCppBinding) -> Result<ActiveRuntime, String> {
        match self.effective_runtime() {
            Some(active) => Ok(active),
            None => match (&binding.runtime_release, &binding.runtime_asset) {
                (Some(release), Some(asset)) => Ok(ActiveRuntime {
                    release: release.clone(),
                    asset: asset.clone(),
                }),
                _ => Err(
                    "No stable-diffusion.cpp engine build is installed. Install an engine in the Local Image Generation settings first."
                        .to_owned(),
                ),
            },
        }
    }

    async fn ensure_server(
        &self,
        model: &EngineModel,
        runtime: &ActiveRuntime,
        conservative: bool,
    ) -> Result<String, String> {
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err("Lettuce is shutting down.".to_owned());
        }
        if let Some(profile_id) = model.binding.profile_id.as_deref()
            && let Ok(profile) = diffusion_catalog().profile(profile_id)
        {
            profile
                .check_runtime(&runtime.release)
                .map_err(|error| error.to_string())?;
        }
        let compute_policy = load_compute_policy(&self.paths, &runtime.release, &runtime.asset);
        let policy_key = serde_json::to_string(&compute_policy)
            .map_err(|error| format!("Failed to fingerprint the compute policy: {error}"))?;
        let binding = &model.binding;
        let key = [
            model.diffusion_model_path.as_str(),
            binding.text_encoder_path.as_deref().unwrap_or(""),
            binding.vae_path.as_deref().unwrap_or(""),
            binding.vision_encoder_path.as_deref().unwrap_or(""),
            runtime.release.as_str(),
            runtime.asset.as_str(),
            policy_key.as_str(),
            if conservative { "conservative" } else { "" },
        ]
        .join("|");
        let mut managed = self.server.lock().await;
        if let Some(server) = managed.as_mut() {
            if server.key == key && server.child.try_wait().ok().flatten().is_none() {
                return Ok(server.base_url.clone());
            }
            server.child.kill().await.ok();
            server.child.wait().await.ok();
            *managed = None;
        }

        self.host.unload_local_llm().await?;

        let runtime_dir = self.paths.runtime_root(&runtime.release, &runtime.asset);
        let executable = runtime_dir.join(server_executable_name());
        if !runtime_is_installed(&self.paths, &runtime.release, &runtime.asset) {
            return Err(format!(
                "The selected stable-diffusion.cpp runtime is not fully installed: {}",
                executable.display()
            ));
        }
        let resolved = self
            .resolve_compute_policy(&runtime.release, &runtime.asset, &compute_policy)
            .await?;
        let diffusion = PathBuf::from(model.diffusion_model_path.trim());
        let configured = |path: Option<&str>| {
            path.map(str::trim)
                .filter(|path| !path.is_empty())
                .map(PathBuf::from)
        };
        let text_encoder = configured(binding.text_encoder_path.as_deref()).ok_or_else(|| {
            format!(
                "{} is not fully configured: set the text encoder file in the model editor first.",
                model.display_name
            )
        })?;
        let vae = configured(binding.vae_path.as_deref()).ok_or_else(|| {
            format!(
                "{} is not fully configured: set the VAE file in the model editor first.",
                model.display_name
            )
        })?;
        let vision_encoder = configured(binding.vision_encoder_path.as_deref());
        if !diffusion.is_file() {
            return Err(format!(
                "Local image model file not found: {}",
                diffusion.display()
            ));
        }
        for path in [Some(&text_encoder), Some(&vae), vision_encoder.as_ref()]
            .into_iter()
            .flatten()
        {
            if !path.is_file() {
                return Err(format!(
                    "Local image component is missing: {}",
                    path.display()
                ));
            }
        }
        let manual_estimate = (!resolved.automatic && !conservative).then(|| {
            self.estimate_files(
                &diffusion,
                Some(&text_encoder),
                Some(&vae),
                vision_encoder.as_deref(),
                resolved.effective_devices.clone(),
            )
        });

        let port = {
            let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
                .await
                .map_err(|error| format!("Failed to reserve a local image server port: {error}"))?;
            listener
                .local_addr()
                .map(|address| address.port())
                .map_err(|error| format!("Failed to read the local image server port: {error}"))?
        };
        std::fs::create_dir_all(&self.paths.loras)
            .map_err(|error| format!("Failed to create the local LoRA library: {error}"))?;
        std::fs::create_dir_all(&self.paths.upscalers)
            .map_err(|error| format!("Failed to create the local upscaler library: {error}"))?;
        let mut command = Command::new(&executable);
        command
            .current_dir(&runtime_dir)
            .arg("--diffusion-model")
            .arg(&diffusion)
            .arg("--lora-model-dir")
            .arg(&self.paths.loras)
            .arg("--hires-upscalers-dir")
            .arg(&self.paths.upscalers)
            .arg("--listen-ip")
            .arg("127.0.0.1")
            .arg("--listen-port")
            .arg(port.to_string())
            .arg("--diffusion-fa")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        command.arg("--llm").arg(&text_encoder);
        command.arg("--vae").arg(&vae);
        if conservative {
            command.arg("--offload-to-cpu");
        } else if let Some(estimate) = &manual_estimate {
            let (backend_spec, params_backend_spec) = manual_backend_specs(estimate);
            command.arg("--backend").arg(backend_spec);
            if let Some(params_backend_spec) = params_backend_spec {
                command.arg("--params-backend").arg(params_backend_spec);
            }
        } else {
            command.arg("--auto-fit");
        }
        command.arg("--split-mode").arg(&compute_policy.split_mode);
        if let Some(max_vram) = max_vram_spec(&compute_policy, &resolved.effective_devices) {
            command.arg("--max-vram").arg(max_vram);
        }
        if let Some(vision_encoder) = vision_encoder {
            command.arg("--llm_vision").arg(vision_encoder);
        }
        library_path_env(
            &mut command,
            &runtime_dir,
            "Failed to configure stable-diffusion.cpp libraries",
        )?;
        let mut child = command
            .spawn()
            .map_err(|error| format!("Failed to start stable-diffusion.cpp: {error}"))?;
        let output = Arc::new(RuntimeOutput::default());
        if let Some(stdout) = child.stdout.take() {
            forward_output(stdout, Arc::clone(&output), Arc::clone(&self.progress));
        }
        if let Some(stderr) = child.stderr.take() {
            forward_output(stderr, Arc::clone(&output), Arc::clone(&self.progress));
        }
        let base_url = format!("http://127.0.0.1:{port}");
        let mut ready = false;
        for _ in 0..READINESS_ATTEMPTS {
            if self.shutting_down.load(Ordering::SeqCst) {
                child.kill().await.ok();
                child.wait().await.ok();
                return Err("Lettuce is shutting down.".to_owned());
            }
            if let Some(status) = child
                .try_wait()
                .map_err(|error| format!("Failed to inspect stable-diffusion.cpp: {error}"))?
            {
                return Err(format!(
                    "stable-diffusion.cpp exited while loading the model ({status})"
                ));
            }
            if self
                .http
                .get(
                    &base_url,
                    "/sdcpp/v1/capabilities",
                    &[],
                    &[],
                    JsonAuth::None,
                    Vec::new(),
                    false,
                )
                .await
                .is_ok_and(|response| (200..300).contains(&response.status))
            {
                ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        if !ready {
            child.kill().await.ok();
            child.wait().await.ok();
            return Err("stable-diffusion.cpp did not become ready within five minutes".to_owned());
        }
        *managed = Some(ManagedServer {
            key,
            base_url: base_url.clone(),
            child,
            output,
        });
        Ok(base_url)
    }

    fn set_active(&self, entry: Option<ActiveGeneration>) {
        if let Ok(mut active) = self.active.lock() {
            *active = entry;
        }
    }

    /// Runs one local generation; `model` comes from the request's model.
    pub async fn generate_images(
        &self,
        model: &EngineModel,
        request: &ProviderImageRequest,
    ) -> Result<Vec<Vec<u8>>, String> {
        if model.diffusion_model_path.starts_with("sdcpp:") {
            return Err(format!(
                "This local image model uses an outdated registration. Open the Local Image Generation settings page to refresh it: {}",
                model.diffusion_model_path
            ));
        }
        let runtime = self.runtime_for(&model.binding)?;
        let profile = model
            .binding
            .profile_id
            .as_deref()
            .and_then(|id| diffusion_catalog().profile(id).ok());
        let references = request
            .input_images
            .iter()
            .map(data_url)
            .collect::<Vec<_>>();
        let mask = request.mask_image.as_ref().map(data_url);
        let loras = normalize_loras(
            &self.paths.loras,
            &request.loras,
            profile.map(|profile| profile.id.as_str()),
        )
        .map_err(|error| error.to_string())?;
        let payload = build_generation_payload(
            &EngineGenerationInput {
                prompt: &request.prompt,
                settings: &request.settings,
                size: request.size.as_deref(),
                count: request.count,
                references: &references,
                mask_image: mask.as_deref(),
                loras: &loras,
            },
            EngineModelLimits {
                display_name: &model.display_name,
                max_reference_images: model.binding.max_reference_images,
                requires_reference_image: model.binding.requires_reference_image.unwrap_or(false),
                supports_image_edit: model.binding.supports_image_edit.unwrap_or(false),
            },
            profile,
        )
        .map_err(|error| error.to_string())?;
        tracing::info!(
            component = "sdcpp",
            model = %model.diffusion_model_path,
            prompt_chars = request.prompt.chars().count(),
            loras = %loras
                .iter()
                .map(|lora| format!(
                    "{}@{}{}",
                    lora.path,
                    lora.multiplier,
                    if lora.is_high_noise { ":high-noise" } else { "" }
                ))
                .collect::<Vec<_>>()
                .join(", "),
            "submitting generation"
        );
        let payload = serde_json::to_vec(&payload)
            .map_err(|error| format!("Failed to submit local image generation: {error}"))?;
        let automatic_policy =
            load_compute_policy(&self.paths, &runtime.release, &runtime.asset).is_automatic();
        let cancel = Arc::new(AtomicBool::new(false));
        self.set_active(Some(ActiveGeneration {
            base_url: String::new(),
            job_id: None,
            cancel: Arc::clone(&cancel),
        }));
        self.progress.progress(GenerationProgress::Starting);
        let mut conservative = false;
        let outcome = loop {
            let base_url = match self.ensure_server(model, &runtime, conservative).await {
                Ok(base_url) => base_url,
                Err(error) => {
                    break Err(if cancel.load(Ordering::SeqCst) {
                        GENERATION_CANCELLED_MESSAGE.to_owned()
                    } else {
                        error
                    });
                }
            };
            if let Ok(mut active) = self.active.lock()
                && let Some(active) = active.as_mut()
            {
                active.base_url.clone_from(&base_url);
                active.job_id = None;
            }
            match self.run_generation_job(&base_url, &payload, &cancel).await {
                Ok(images) => break Ok(images),
                Err(GenerationJobError::Cancelled) => {
                    break Err(GENERATION_CANCELLED_MESSAGE.to_owned());
                }
                Err(
                    GenerationJobError::Rejected(message)
                    | GenerationJobError::Infrastructure(message),
                ) => break Err(message),
                Err(GenerationJobError::Failed(message)) => {
                    if cancel.load(Ordering::SeqCst) {
                        break Err(GENERATION_CANCELLED_MESSAGE.to_owned());
                    }
                    let tail = self
                        .server
                        .lock()
                        .await
                        .as_ref()
                        .map(|server| server.output.tail())
                        .unwrap_or_default();
                    let oom_detected = oom_signature_present(&tail)
                        || oom_signature_present(std::slice::from_ref(&message));
                    if !conservative && automatic_policy && oom_detected {
                        tracing::info!(
                            component = "sdcpp",
                            "generation ran out of memory; retrying once with CPU-offloaded weights: {message}"
                        );
                        self.progress.progress(GenerationProgress::Retrying);
                        self.stop().await;
                        conservative = true;
                        continue;
                    }
                    break Err(message);
                }
            }
        };
        self.set_active(None);
        let images = outcome?;
        images
            .iter()
            .map(|image| {
                let encoded = image
                    .get("b64_json")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "Local image result is missing image data".to_owned())?;
                base64::engine::general_purpose::STANDARD
                    .decode(encoded.trim())
                    .map_err(|error| format!("Failed to decode base64 image: {error}"))
            })
            .collect()
    }

    async fn run_generation_job(
        &self,
        base_url: &str,
        payload: &[u8],
        cancel: &AtomicBool,
    ) -> Result<Vec<Value>, GenerationJobError> {
        if cancel.load(Ordering::SeqCst) {
            return Err(GenerationJobError::Cancelled);
        }
        let response = self
            .http
            .post_json(
                base_url,
                "/sdcpp/v1/img_gen",
                &[],
                payload.to_vec(),
                &[],
                JsonAuth::None,
                Vec::new(),
                false,
            )
            .await
            .map_err(|error| {
                GenerationJobError::Infrastructure(format!(
                    "Failed to submit local image generation: {error}"
                ))
            })?;
        if !(200..300).contains(&response.status) {
            return Err(GenerationJobError::Rejected(format!(
                "stable-diffusion.cpp rejected image generation ({}): {}",
                lettuce_network::status_text(response.status),
                String::from_utf8_lossy(&response.body)
            )));
        }
        let accepted = serde_json::from_slice::<Value>(&response.body).map_err(|error| {
            GenerationJobError::Infrastructure(format!(
                "Failed to parse stable-diffusion.cpp job response: {error}"
            ))
        })?;
        let job_id = accepted
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if let Ok(mut active) = self.active.lock()
            && let Some(active) = active.as_mut()
        {
            active.job_id.clone_from(&job_id);
        }
        let poll_url = accepted
            .get("poll_url")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                GenerationJobError::Infrastructure(
                    "stable-diffusion.cpp response did not include a poll URL".to_owned(),
                )
            })?;
        let poll_url = if poll_url.starts_with("http://") || poll_url.starts_with("https://") {
            poll_url.to_owned()
        } else {
            format!("{base_url}{poll_url}")
        };
        let (poll_endpoint, poll_path) = split_endpoint(&poll_url).ok_or_else(|| {
            GenerationJobError::Infrastructure(
                "stable-diffusion.cpp response did not include a poll URL".to_owned(),
            )
        })?;

        let mut announced_generating = false;
        for _ in 0..POLL_ATTEMPTS {
            tokio::time::sleep(POLL_INTERVAL).await;
            let response = match self
                .http
                .get(
                    poll_endpoint,
                    poll_path,
                    &[],
                    &[],
                    JsonAuth::None,
                    Vec::new(),
                    false,
                )
                .await
            {
                Ok(response) => response,
                Err(error) => {
                    if cancel.load(Ordering::SeqCst) {
                        return Err(GenerationJobError::Cancelled);
                    }
                    return Err(GenerationJobError::Failed(format!(
                        "Failed to poll local image generation: {error}"
                    )));
                }
            };
            if !(200..300).contains(&response.status) {
                return Err(GenerationJobError::Failed(format!(
                    "stable-diffusion.cpp job polling failed with status {}",
                    lettuce_network::status_text(response.status)
                )));
            }
            let job = serde_json::from_slice::<Value>(&response.body).map_err(|error| {
                GenerationJobError::Infrastructure(format!(
                    "Failed to parse local image job: {error}"
                ))
            })?;
            match job.get("status").and_then(Value::as_str) {
                Some("queued") => {
                    self.progress.progress(GenerationProgress::Queued {
                        queue_position: job.get("queue_position").and_then(Value::as_u64),
                    });
                }
                Some("generating" | "running") => {
                    if !announced_generating {
                        announced_generating = true;
                        self.progress.progress(GenerationProgress::Generating);
                    }
                }
                Some("completed") => {
                    return job
                        .pointer("/result/images")
                        .and_then(Value::as_array)
                        .cloned()
                        .ok_or_else(|| {
                            GenerationJobError::Failed(
                                "Local image job completed without images".to_owned(),
                            )
                        });
                }
                Some("cancelled") => {
                    if cancel.load(Ordering::SeqCst) {
                        return Err(GenerationJobError::Cancelled);
                    }
                    let message = job
                        .pointer("/error/message")
                        .and_then(Value::as_str)
                        .unwrap_or("Local image generation was cancelled by the engine");
                    return Err(GenerationJobError::Failed(message.to_owned()));
                }
                Some("failed") => {
                    let message = job
                        .pointer("/error/message")
                        .and_then(Value::as_str)
                        .unwrap_or("Local image generation failed");
                    return Err(GenerationJobError::Failed(message.to_owned()));
                }
                other => {
                    return Err(GenerationJobError::Infrastructure(format!(
                        "Unknown stable-diffusion.cpp job status: {other:?}"
                    )));
                }
            }
        }
        Err(GenerationJobError::Infrastructure(
            "Local image generation timed out after ten minutes".to_owned(),
        ))
    }

    /// Cancels the running generation: the queued engine job when possible,
    /// otherwise by stopping the server. `false` when nothing was running.
    pub async fn cancel_generation(&self) -> bool {
        let active = self.active.lock().ok().and_then(|active| active.clone());
        let Some(active) = active else {
            return false;
        };
        active.cancel.store(true, Ordering::SeqCst);
        if let Some(job_id) = active
            .job_id
            .as_deref()
            .filter(|_| !active.base_url.is_empty())
        {
            let cancelled = self
                .http
                .post_json(
                    &active.base_url,
                    &format!("/sdcpp/v1/jobs/{job_id}/cancel"),
                    &[],
                    Vec::new(),
                    &[],
                    JsonAuth::None,
                    Vec::new(),
                    false,
                )
                .await
                .is_ok_and(|response| (200..300).contains(&response.status));
            if cancelled {
                tracing::info!(
                    component = "sdcpp",
                    "cancelled queued local image job {job_id}"
                );
                self.progress.progress(GenerationProgress::Cancelled);
                return true;
            }
        }
        tracing::info!(
            component = "sdcpp",
            "stopping stable-diffusion.cpp to abort the running generation"
        );
        self.stop().await;
        self.progress.progress(GenerationProgress::Cancelled);
        true
    }

    /// Stops the managed server (engine switch, delete, compute policy
    /// changes, cancellation and the out-of-memory retry).
    pub async fn stop(&self) {
        let mut managed = self.server.lock().await;
        if let Some(mut server) = managed.take() {
            server.child.kill().await.ok();
            server.child.wait().await.ok();
        }
    }

    /// Stops the server before a llama.cpp request; a failed kill fails that
    /// request like legacy.
    pub async fn stop_for_llama(&self) -> Result<(), String> {
        let mut managed = self.server.lock().await;
        if let Some(server) = managed.as_mut() {
            server
                .child
                .kill()
                .await
                .map_err(|error| format!("Failed to stop stable-diffusion.cpp: {error}"))?;
            server.child.wait().await.ok();
        }
        *managed = None;
        Ok(())
    }

    /// Refuses new servers and cancels the running generation.
    pub fn begin_shutdown(&self) {
        self.shutting_down.store(true, Ordering::SeqCst);
        if let Ok(mut active) = self.active.lock()
            && let Some(active) = active.take()
        {
            active.cancel.store(true, Ordering::SeqCst);
        }
    }

    pub async fn shutdown(&self) {
        self.stop().await;
    }
}

#[async_trait]
impl ImageProviderPort for LocalDiffusionEngine {
    async fn generate(
        &self,
        request: ProviderImageRequest,
    ) -> Result<ProviderImageOutput, ImageProviderError> {
        if request.account.protocol != ProviderProtocol::StableDiffusion
            || !request
                .account
                .provider_kind
                .eq_ignore_ascii_case(LOCAL_DIFFUSION_PROVIDER_KIND)
        {
            return Err(ImageProviderError::Unsupported(
                request.account.provider_kind.clone(),
            ));
        }
        if cfg!(any(target_os = "android", target_os = "ios")) {
            return Err(ImageProviderError::Failed(DESKTOP_ONLY_MESSAGE.to_owned()));
        }
        if request.cancellation.is_cancelled() {
            return Err(ImageProviderError::Cancelled);
        }
        let model = EngineModel {
            display_name: request.model_display_name.clone(),
            diffusion_model_path: request.external_model_id.clone(),
            binding: request.settings.cpp.clone(),
        };
        let generation = self.generate_images(&model, &request);
        let cancellation = async {
            request.cancellation.cancelled().await;
            self.cancel_generation().await;
        };
        tokio::pin!(generation);
        tokio::pin!(cancellation);
        let result = tokio::select! {
            biased;
            result = &mut generation => result,
            () = &mut cancellation => generation.await,
        };
        match result {
            Ok(images) => Ok(ProviderImageOutput {
                images: images
                    .into_iter()
                    .map(|bytes| ProviderImage {
                        bytes,
                        declared_mime_type: Some("image/png".to_owned()),
                        text: None,
                    })
                    .collect(),
                usage: None,
            }),
            Err(message) if message == GENERATION_CANCELLED_MESSAGE => {
                Err(ImageProviderError::Cancelled)
            }
            Err(message) => Err(ImageProviderError::Failed(message)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex as StdMutex;

    use lettuce_jobs::handle::CancellationToken;
    use lettuce_models::{
        ProviderAccount, ProviderConfig, StableDiffusionLora, StableDiffusionSettings,
    };
    use lettuce_settings::SecretOwnerId;
    use lettuce_types::{
        JobId, ModelProfileId, OperationId, ProviderAccountId, Revision, TimestampMillis,
    };

    use super::super::layout::{DiffusionPaths, write_runtime_manifest};
    use super::super::releases::RuntimeAsset;
    use super::*;

    #[derive(Default)]
    struct Host {
        devices: StdMutex<Vec<HardwareGpu>>,
        unloads: StdMutex<u32>,
    }

    #[async_trait]
    impl EngineHost for Host {
        async fn gpu_devices(&self) -> Result<Vec<HardwareGpu>, String> {
            Ok(self.devices.lock().expect("devices").clone())
        }

        fn available_memory_bytes(&self) -> Option<u64> {
            None
        }

        async fn unload_local_llm(&self) -> Result<(), String> {
            *self.unloads.lock().expect("unloads") += 1;
            Ok(())
        }
    }

    #[derive(Default)]
    struct Progress(StdMutex<Vec<GenerationProgress>>);

    impl GenerationProgressSink for Progress {
        fn progress(&self, progress: GenerationProgress) {
            self.0.lock().expect("progress").push(progress);
        }
    }

    fn png_dimensions(bytes: &[u8]) -> (u32, u32) {
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        (
            u32::from_be_bytes(bytes[16..20].try_into().expect("width")),
            u32::from_be_bytes(bytes[20..24].try_into().expect("height")),
        )
    }

    #[tokio::test]
    #[ignore = "needs a legacy app folder with an sd.cpp engine archive and FLUX.2 Klein 4B in LETTUCE_SD_APP_DIR"]
    async fn generates_with_the_real_engine_and_a_klein_lora() {
        let Ok(app_dir) = std::env::var("LETTUCE_SD_APP_DIR") else {
            return;
        };
        let app_dir = PathBuf::from(app_dir);
        let archive_dir = std::fs::read_dir(app_dir.join("downloads/sdcpp"))
            .expect("engine downloads")
            .flatten()
            .map(|entry| entry.path())
            .max()
            .expect("an engine release");
        let archive = std::fs::read_dir(&archive_dir)
            .expect("release folder")
            .flatten()
            .map(|entry| entry.path())
            .find(|path| path.extension().is_some_and(|extension| extension == "zip"))
            .expect("engine archive");
        let release = archive_dir
            .file_name()
            .and_then(|name| name.to_str())
            .expect("release")
            .to_owned();
        let asset = archive
            .file_name()
            .and_then(|name| name.to_str())
            .expect("asset")
            .to_owned();
        let root = std::env::temp_dir().join(format!("sd-engine-{}", OperationId::new()));
        let paths = DiffusionPaths::legacy_layout(&root, app_dir.join("models/image"));
        let build = paths.runtime_root(&release, &asset);
        write_runtime_manifest(
            &build,
            &RuntimeAsset {
                name: asset.clone(),
                backend: "vulkan".to_owned(),
                bytes: 1,
                sha256: None,
                download_url: String::new(),
                dependencies: Vec::new(),
            },
        )
        .expect("manifest");
        super::super::layout::extract_runtime_archive(&archive, &asset, &build).expect("extract");
        std::fs::create_dir_all(&paths.loras).expect("loras");
        std::fs::copy(
            app_dir.join("models/loras/Flux-Klein-4B-Art_10.safetensors"),
            paths.loras.join("Flux-Klein-4B-Art_10.safetensors"),
        )
        .expect("copy LoRA");

        let host = Arc::new(Host::default());
        let progress = Arc::new(Progress::default());
        let engine = LocalDiffusionEngine::new(
            paths.clone(),
            BulkHttpClient::new().expect("client"),
            Arc::clone(&host) as Arc<dyn EngineHost>,
            Arc::clone(&progress) as Arc<dyn GenerationProgressSink>,
        );
        let devices = engine
            .runtime_devices(&release, &asset)
            .await
            .expect("engine devices");
        *host.devices.lock().expect("devices") = devices
            .iter()
            .enumerate()
            .map(|(index, device)| HardwareGpu {
                index,
                name: device.name.clone(),
                description: device.description.clone(),
                memory_total: 8 << 30,
                memory_free: 6 << 30,
            })
            .collect();

        let catalog = diffusion_catalog();
        let (profile, variant) = catalog
            .find_variant("flux-2-klein-4b", "q4-0")
            .expect("klein");
        let components = profile.components(variant);
        let path_of = |role| {
            components
                .iter()
                .find(|component| component.role == role)
                .map(|component| paths.component_path(component).display().to_string())
        };
        let binding = StableDiffusionCppBinding {
            profile_id: Some(profile.id.clone()),
            variant_id: Some(variant.id.clone()),
            text_encoder_path: path_of(crate::DiffusionComponentRole::TextEncoder),
            vae_path: path_of(crate::DiffusionComponentRole::Vae),
            supports_image_edit: Some(true),
            ..StableDiffusionCppBinding::default()
        };
        let request = ProviderImageRequest {
            job_id: JobId::new(),
            model_profile_id: ModelProfileId::new(),
            account: ProviderAccount {
                id: ProviderAccountId::new(),
                secret_owner_id: SecretOwnerId::new(),
                provider_kind: "sdcpp".to_owned(),
                protocol: ProviderProtocol::StableDiffusion,
                label: "stable-diffusion.cpp".to_owned(),
                endpoint: None,
                enabled: true,
                streaming_enabled: false,
                allow_invalid_tls: false,
                api_key_ref: None,
                secret_headers: Vec::new(),
                config: ProviderConfig::Standard,
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(1),
                updated_at: TimestampMillis::new(1),
            },
            external_model_id: path_of(crate::DiffusionComponentRole::DiffusionModel)
                .expect("diffusion"),
            model_display_name: profile.installed_display_name(variant),
            prompt: "a red lighthouse on a cliff at dusk, painterly".to_owned(),
            settings: StableDiffusionSettings {
                cpp: binding,
                seed: Some(7),
                ..StableDiffusionSettings::default()
            },
            loras: vec![StableDiffusionLora {
                path: "Flux-Klein-4B-Art_10.safetensors".to_owned(),
                multiplier: 0.8,
                is_high_noise: false,
                keywords: Vec::new(),
            }],
            input_images: Vec::new(),
            mask_image: None,
            size: Some("512x512".to_owned()),
            quality: None,
            style: None,
            count: 1,
            text_output: false,
            cancellation: CancellationToken::new(),
        };
        let output = engine.generate(request).await.expect("generation");
        assert_eq!(output.images.len(), 1);
        assert_eq!(png_dimensions(&output.images[0].bytes), (512, 512));
        assert_eq!(*host.unloads.lock().expect("unloads"), 1);
        let events = progress.0.lock().expect("progress").clone();
        assert_eq!(events.first(), Some(&GenerationProgress::Starting));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, GenerationProgress::Sampling { .. }))
        );
        let cached = std::fs::read_dir(paths.loras.join(".sdcpp-compat"))
            .expect("compat cache")
            .flatten()
            .map(|entry| entry.path())
            .next()
            .expect("a rewritten LoRA");
        if let Some(legacy) = std::fs::read_dir(app_dir.join("models/loras/.sdcpp-compat"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("Flux-Klein-4B-Art_10-"))
            })
        {
            assert!(
                std::fs::read(&cached).expect("cached") == std::fs::read(legacy).expect("legacy"),
                "the rewritten LoRA must equal legacy's cache byte for byte"
            );
        }
        std::fs::write(
            std::env::temp_dir().join("lettuce-sd-e2e.png"),
            &output.images[0].bytes,
        )
        .ok();
        engine.shutdown().await;
        std::fs::remove_dir_all(root).ok();
    }
}
