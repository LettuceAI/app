//! Whether a catalog model or a downloadable bundle runs on this machine:
//! a placement estimate before the model is installed, a real execution
//! probe once it is.

use std::io::Cursor;
use std::time::{Duration, Instant};

use base64::Engine as _;
use lettuce_models::{StableDiffusionCppBinding, StableDiffusionLora};
use lettuce_network::JsonAuth;
use serde::Serialize;
use serde_json::{Value, json};

use super::fit::{
    DeviceSource, FitEstimate, PlanMode, catalog_components, compute_auto_fit_estimate,
    file_components,
};
use super::layout::{ActiveRuntime, load_compute_policy, runtime_is_installed};
use super::loras::normalize_loras;
use super::server::{DESKTOP_ONLY_MESSAGE, EngineModel, LocalDiffusionEngine, split_endpoint};
use crate::{DiffusionComponentRole, DiffusionProfile, diffusion_catalog};

const PROBE_POLL_ATTEMPTS: u32 = 1_200;
const PROBE_POLL_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RunnabilityStatus {
    IncompatibleRuntime,
    NotInstalled,
    EstimatedRunnable,
    CpuFallback,
    Inconclusive,
    Passed,
    Failed,
}

/// A runnability verdict: how it was reached, how exact it is, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Runnability {
    pub status: RunnabilityStatus,
    pub method: &'static str,
    pub exact: bool,
    pub scope: &'static str,
    pub placement_policy: &'static str,
    pub elapsed_ms: Option<u64>,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimate: Option<FitEstimate>,
}

/// A fit test of a catalog variant on an engine build; unset fields take the
/// profile defaults.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CatalogRunnabilityRequest {
    pub profile_id: String,
    pub variant_id: String,
    pub runtime_release: String,
    pub runtime_asset: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub reference_image_count: Option<u8>,
    pub reference_images: Vec<String>,
    pub loras: Vec<StableDiffusionLora>,
    pub prompt: Option<String>,
    pub negative_prompt: Option<String>,
    pub sample_steps: Option<u32>,
    pub cfg_scale: Option<f64>,
    pub seed: Option<i64>,
    pub sample_method: Option<String>,
    pub batch_count: Option<u32>,
    /// Runs every sample step instead of a one-step probe.
    pub full_execution: bool,
}

/// A bundle about to be downloaded, by its file sizes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RemoteBundleRunnabilityRequest {
    pub profile_id: String,
    pub runtime_release: String,
    pub runtime_asset: String,
    pub diffusion_bytes: u64,
    pub text_encoder_bytes: u64,
    pub vae_bytes: u64,
    pub vision_encoder_bytes: u64,
}

enum ProbeJobError {
    Execution(String),
    Infrastructure(String),
}

fn elapsed_ms(started: Instant) -> Option<u64> {
    Some(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX))
}

fn engine_unavailable(
    profile: &DiffusionProfile,
    release: &str,
    asset: &str,
    engine: &LocalDiffusionEngine,
    not_installed_reason: &str,
) -> Option<Runnability> {
    if let Err(error) = profile.check_runtime(release) {
        return Some(Runnability {
            status: RunnabilityStatus::IncompatibleRuntime,
            method: "stableDiffusionCppRuntimeCompatibility",
            exact: true,
            scope: "engineUnavailable",
            placement_policy: "notRun",
            elapsed_ms: None,
            reason: error.to_string(),
            estimate: None,
        });
    }
    if !runtime_is_installed(engine.paths(), release, asset) {
        return Some(Runnability {
            status: RunnabilityStatus::NotInstalled,
            method: "stableDiffusionCppAutoFitEstimate",
            exact: false,
            scope: "engineUnavailable",
            placement_policy: "notRun",
            elapsed_ms: None,
            reason: not_installed_reason.to_owned(),
            estimate: None,
        });
    }
    None
}

fn estimated(
    status: RunnabilityStatus,
    started: Instant,
    reason: &str,
    estimate: FitEstimate,
) -> Runnability {
    Runnability {
        status,
        method: "stableDiffusionCppConfiguredPlacementEstimate",
        exact: false,
        scope: "preInstallEstimate",
        placement_policy: "sdCppConfiguredPolicyEstimate",
        elapsed_ms: elapsed_ms(started),
        reason: reason.to_owned(),
        estimate: Some(estimate),
    }
}

fn estimate_failed(started: Instant, reason: String) -> Runnability {
    Runnability {
        status: RunnabilityStatus::Inconclusive,
        method: "stableDiffusionCppConfiguredPlacementEstimate",
        exact: false,
        scope: "preInstallEstimate",
        placement_policy: "sdCppConfiguredPolicyEstimate",
        elapsed_ms: elapsed_ms(started),
        reason,
        estimate: None,
    }
}

/// Which placement the estimate settled on, as the reason texts name it.
enum Placement {
    CpuFallback,
    Split,
    TimeShare,
    Concurrent,
}

fn placement(estimate: &FitEstimate) -> Placement {
    if estimate.placements.iter().any(|placement| placement.cpu) {
        Placement::CpuFallback
    } else if estimate.placements.iter().any(|placement| placement.split) {
        Placement::Split
    } else if estimate.plan_mode == PlanMode::TimeShare {
        Placement::TimeShare
    } else {
        Placement::Concurrent
    }
}

fn blank_reference_data_url(width: u32, height: u32) -> Result<String, String> {
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(width, height)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|error| format!("Failed to create probe reference image: {error}"))?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes.into_inner())
    ))
}

impl LocalDiffusionEngine {
    fn variant_installed_for(
        &self,
        profile: &DiffusionProfile,
        variant: &crate::DiffusionVariant,
        release: &str,
        asset: &str,
    ) -> bool {
        runtime_is_installed(self.paths(), release, asset)
            && profile.components(variant).iter().all(|component| {
                std::fs::metadata(self.paths().component_path(component))
                    .is_ok_and(|metadata| metadata.len() == component.bytes)
            })
    }

    async fn run_probe_job(&self, base_url: &str, payload: &Value) -> Result<(), ProbeJobError> {
        let response = self
            .http
            .post_json(
                base_url,
                "/sdcpp/v1/img_gen",
                &[],
                payload.to_string().into_bytes(),
                &[],
                JsonAuth::None,
                Vec::new(),
                false,
            )
            .await
            .map_err(|error| {
                ProbeJobError::Infrastructure(format!(
                    "Failed to submit stable-diffusion.cpp fit test: {error}"
                ))
            })?;
        if !(200..300).contains(&response.status) {
            return Err(ProbeJobError::Execution(format!(
                "Fit test was rejected ({}): {}",
                lettuce_network::status_text(response.status),
                String::from_utf8_lossy(&response.body)
            )));
        }
        let accepted = serde_json::from_slice::<Value>(&response.body).map_err(|error| {
            ProbeJobError::Infrastructure(format!("Failed to parse fit-test response: {error}"))
        })?;
        let poll_path = accepted
            .get("poll_url")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ProbeJobError::Infrastructure(
                    "Fit-test response did not include a poll URL".to_owned(),
                )
            })?;
        let poll_url = if poll_path.starts_with("http://") || poll_path.starts_with("https://") {
            poll_path.to_owned()
        } else {
            format!("{base_url}{poll_path}")
        };
        let (poll_endpoint, poll_path) = split_endpoint(&poll_url).ok_or_else(|| {
            ProbeJobError::Infrastructure("Fit-test response did not include a poll URL".to_owned())
        })?;
        for _ in 0..PROBE_POLL_ATTEMPTS {
            tokio::time::sleep(PROBE_POLL_INTERVAL).await;
            let response = self
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
                .map_err(|error| {
                    ProbeJobError::Infrastructure(format!(
                        "Failed to poll stable-diffusion.cpp fit test: {error}"
                    ))
                })?;
            let job = serde_json::from_slice::<Value>(&response.body).map_err(|error| {
                ProbeJobError::Infrastructure(format!(
                    "Failed to parse stable-diffusion.cpp fit test: {error}"
                ))
            })?;
            match job.get("status").and_then(Value::as_str) {
                Some("queued" | "generating" | "running") => {}
                Some("completed") => return Ok(()),
                Some("failed" | "cancelled") => {
                    return Err(ProbeJobError::Execution(
                        job.pointer("/error/message")
                            .and_then(Value::as_str)
                            .unwrap_or("stable-diffusion.cpp fit test failed")
                            .to_owned(),
                    ));
                }
                status => {
                    return Err(ProbeJobError::Infrastructure(format!(
                        "Unknown fit-test job status: {status:?}"
                    )));
                }
            }
        }
        Err(ProbeJobError::Infrastructure(
            "stable-diffusion.cpp fit test timed out after ten minutes".to_owned(),
        ))
    }

    /// Whether a catalog variant runs on an engine build: an estimate from
    /// the configured compute policy before the variant is installed, a real
    /// generation probe after (which starts the image server).
    #[expect(
        clippy::too_many_lines,
        reason = "one verdict per branch of the legacy fit test, kept in order"
    )]
    pub async fn catalog_runnability(
        &self,
        request: CatalogRunnabilityRequest,
    ) -> Result<Runnability, String> {
        if cfg!(any(target_os = "android", target_os = "ios")) {
            return Err(DESKTOP_ONLY_MESSAGE.to_owned());
        }
        let (profile, variant) = diffusion_catalog()
            .find_variant(&request.profile_id, &request.variant_id)
            .map_err(|error| error.to_string())?;
        let (release, asset) = (&request.runtime_release, &request.runtime_asset);
        if let Some(unavailable) = engine_unavailable(
            profile,
            release,
            asset,
            self,
            "Install the selected stable-diffusion.cpp engine build before checking model runnability. The model itself does not need to be installed.",
        ) {
            return Ok(unavailable);
        }
        let policy = load_compute_policy(self.paths(), release, asset);
        let installed = self.variant_installed_for(profile, variant, release, asset);
        let refs = if request.reference_images.is_empty() {
            usize::from(request.reference_image_count.unwrap_or(0))
        } else {
            if installed
                && request
                    .reference_image_count
                    .is_some_and(|count| usize::from(count) != request.reference_images.len())
            {
                return Err(
                    "referenceImageCount must match the number of supplied referenceImages."
                        .to_owned(),
                );
            }
            request.reference_images.len()
        };
        if installed
            && let Some(maximum) = profile.max_reference_images
            && refs > usize::from(maximum)
        {
            return Err(format!(
                "{} accepts at most {maximum} reference images.",
                profile.display_name
            ));
        }
        if installed && profile.requires_reference_image && refs == 0 {
            return Err(format!(
                "{} requires at least one reference image.",
                profile.display_name
            ));
        }
        let width = request.width.unwrap_or(profile.default_width);
        let height = request.height.unwrap_or(profile.default_height);
        if installed && (width == 0 || height == 0) {
            return Err("Fit-test width and height must be greater than zero.".to_owned());
        }
        let requested_steps = request
            .sample_steps
            .unwrap_or(u32::from(profile.default_steps));
        if installed && requested_steps == 0 {
            return Err("Fit-test sampleSteps must be greater than zero.".to_owned());
        }
        let batch_count = request.batch_count.unwrap_or(1);
        if installed && batch_count == 0 {
            return Err("Fit-test batchCount must be greater than zero.".to_owned());
        }
        if !installed {
            let started = Instant::now();
            return Ok(
                match self.resolve_compute_policy(release, asset, &policy).await {
                    Ok(resolved) => {
                        let estimate = compute_auto_fit_estimate(
                            &catalog_components(profile, variant),
                            resolved.effective_devices,
                            self.host.available_memory_bytes(),
                            DeviceSource::ConfiguredEnginePolicy,
                        );
                        let reason = match placement(&estimate) {
                            Placement::CpuFallback => {
                                "Estimated runnable using the configured Stable Diffusion compute policy with CPU fallback for at least one model component. This is not a speed guarantee; install the model to run the exact execution probe."
                            }
                            Placement::Split => {
                                "Estimated to fit by splitting at least one model component across the GPUs selected by the configured compute policy. Install the model to verify with a real execution probe."
                            }
                            Placement::TimeShare => {
                                "Estimated to fit on the configured GPU devices by loading model components per phase. Install the model to verify with a real execution probe."
                            }
                            Placement::Concurrent => {
                                "Estimated to fit concurrently on the GPU devices selected by the configured compute policy. Install the model to verify with a real execution probe."
                            }
                        };
                        estimated(
                            RunnabilityStatus::EstimatedRunnable,
                            started,
                            reason,
                            estimate,
                        )
                    }
                    Err(error) => estimate_failed(
                        started,
                        format!(
                            "The pre-install runnability estimate could not be completed: {error}"
                        ),
                    ),
                },
            );
        }
        let path_of = |role| {
            profile
                .components(variant)
                .into_iter()
                .find(|component| component.role == role)
                .map(|component| self.paths().component_path(component).display().to_string())
        };
        let diffusion = path_of(DiffusionComponentRole::DiffusionModel).ok_or_else(|| {
            format!(
                "{} does not define a diffusion_model component",
                profile.display_name
            )
        })?;
        let model = EngineModel {
            display_name: profile.installed_display_name(variant),
            diffusion_model_path: diffusion,
            binding: StableDiffusionCppBinding {
                profile_id: Some(profile.id.clone()),
                variant_id: Some(variant.id.clone()),
                text_encoder_path: path_of(DiffusionComponentRole::TextEncoder),
                vae_path: path_of(DiffusionComponentRole::Vae),
                vision_encoder_path: path_of(DiffusionComponentRole::VisionEncoder),
                runtime_release: Some(release.clone()),
                runtime_asset: Some(asset.clone()),
                max_reference_images: profile.max_reference_images.map(u32::from),
                supports_image_edit: Some(profile.supports_image_edit),
                requires_reference_image: Some(profile.requires_reference_image),
                ..StableDiffusionCppBinding::default()
            },
        };
        let placement_policy = if policy.is_automatic() {
            "sdCppAutoFit"
        } else {
            "sdCppConfiguredPolicy"
        };
        let started = Instant::now();
        let runtime = ActiveRuntime {
            release: release.clone(),
            asset: asset.clone(),
        };
        let base_url = match self.ensure_server(&model, &runtime, false).await {
            Ok(base_url) => base_url,
            Err(error) => {
                return Ok(Runnability {
                    status: RunnabilityStatus::Inconclusive,
                    method: "stableDiffusionCppExecutionProbe",
                    exact: false,
                    scope: "serverStartup",
                    placement_policy,
                    elapsed_ms: elapsed_ms(started),
                    reason: format!(
                        "The sd.cpp server could not be prepared, so no runnability verdict was made: {error}"
                    ),
                    estimate: None,
                });
            }
        };
        let supplied_references = !request.reference_images.is_empty();
        let references = if supplied_references {
            request.reference_images
        } else if refs > 0 {
            vec![blank_reference_data_url(width, height)?; refs]
        } else {
            Vec::new()
        };
        let loras = normalize_loras(&self.paths().loras, &request.loras, Some(&profile.id))
            .map_err(|error| error.to_string())?;
        let supplied_prompt = request.prompt.is_some();
        let mut sample_params = json!({
            "sample_steps": if request.full_execution { requested_steps } else { 1 },
            "guidance": { "txt_cfg": request.cfg_scale.unwrap_or(f64::from(profile.default_cfg)) },
        });
        if let Some(method) = &request.sample_method {
            sample_params["sample_method"] = json!(method);
        }
        let payload = json!({
            "prompt": request.prompt.as_deref().unwrap_or("runnability probe"),
            "negative_prompt": request.negative_prompt.as_deref().unwrap_or(""),
            "width": width,
            "height": height,
            "seed": request.seed.unwrap_or(-1),
            "batch_count": batch_count,
            "auto_resize_ref_image": true,
            "increase_ref_index": false,
            "ref_images": references,
            "sample_params": sample_params,
            "lora": loras,
            "vae_tiling_params": { "enabled": true },
            "hires": { "enabled": false },
            "output_format": "png",
            "output_compression": 100
        });
        let result = self.run_probe_job(&base_url, &payload).await;
        let request_matched =
            request.full_execution && supplied_prompt && (refs == 0 || supplied_references);
        let scope = if request_matched {
            "fullRequest"
        } else {
            "executionProbe"
        };
        Ok(match result {
            Ok(()) => Runnability {
                status: RunnabilityStatus::Passed,
                method: "stableDiffusionCppExecutionProbe",
                exact: request_matched,
                scope,
                placement_policy,
                elapsed_ms: elapsed_ms(started),
                reason: if request_matched {
                    if policy.is_automatic() {
                        "stable-diffusion.cpp completed the full supplied generation request using its real auto-fit placement."
                    } else {
                        "stable-diffusion.cpp completed the full supplied generation request using the configured GPU placement policy."
                    }
                } else if request.full_execution {
                    "stable-diffusion.cpp completed a full representative generation, but generated placeholders were used for request data that was not supplied."
                } else {
                    "stable-diffusion.cpp completed a one-step execution probe at the requested shape. This proves the tested graph ran, but it is not a full-request guarantee."
                }
                .to_owned(),
                estimate: None,
            },
            Err(ProbeJobError::Execution(error)) => Runnability {
                status: RunnabilityStatus::Failed,
                method: "stableDiffusionCppExecutionProbe",
                exact: request_matched,
                scope,
                placement_policy,
                elapsed_ms: elapsed_ms(started),
                reason: error,
                estimate: None,
            },
            Err(ProbeJobError::Infrastructure(error)) => Runnability {
                status: RunnabilityStatus::Inconclusive,
                method: "stableDiffusionCppExecutionProbe",
                exact: false,
                scope: "probeInfrastructure",
                placement_policy,
                elapsed_ms: elapsed_ms(started),
                reason: format!("The execution probe could not produce a runnability verdict: {error}"),
                estimate: None,
            },
        })
    }

    /// The placement estimate for a bundle before it is downloaded.
    pub async fn remote_bundle_runnability(
        &self,
        request: &RemoteBundleRunnabilityRequest,
    ) -> Result<Runnability, String> {
        if cfg!(any(target_os = "android", target_os = "ios")) {
            return Err(DESKTOP_ONLY_MESSAGE.to_owned());
        }
        let profile = diffusion_catalog()
            .profile(&request.profile_id)
            .map_err(|_| format!("Unknown local image architecture: {}", request.profile_id))?;
        let (release, asset) = (&request.runtime_release, &request.runtime_asset);
        if let Some(unavailable) = engine_unavailable(
            profile,
            release,
            asset,
            self,
            "Install a compatible stable-diffusion.cpp engine before downloading this image bundle.",
        ) {
            return Ok(unavailable);
        }
        let started = Instant::now();
        let policy = load_compute_policy(self.paths(), release, asset);
        let resolved = match self.resolve_compute_policy(release, asset, &policy).await {
            Ok(resolved) => resolved,
            Err(error) => {
                return Ok(estimate_failed(
                    started,
                    format!("The pre-download estimate could not be completed: {error}"),
                ));
            }
        };
        let estimate = compute_auto_fit_estimate(
            &file_components(
                request.diffusion_bytes,
                request.text_encoder_bytes,
                request.vae_bytes,
                request.vision_encoder_bytes,
            ),
            resolved.effective_devices,
            self.host.available_memory_bytes(),
            DeviceSource::ConfiguredEnginePolicy,
        );
        let (status, reason) = match placement(&estimate) {
            Placement::CpuFallback => (
                RunnabilityStatus::CpuFallback,
                "Pre-download estimate: one or more components will use CPU fallback. A smaller quantization is recommended.",
            ),
            Placement::Split => (
                RunnabilityStatus::EstimatedRunnable,
                "Pre-download estimate: the selected bundle fits by splitting components across configured GPUs.",
            ),
            Placement::TimeShare => (
                RunnabilityStatus::EstimatedRunnable,
                "Pre-download estimate: the selected bundle fits with phased GPU placement.",
            ),
            Placement::Concurrent => (
                RunnabilityStatus::EstimatedRunnable,
                "Pre-download estimate: the selected bundle fits concurrently on the configured GPU devices.",
            ),
        };
        Ok(estimated(status, started, reason, estimate))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use lettuce_network::BulkHttpClient;
    use lettuce_types::OperationId;

    use super::super::layout::{DiffusionPaths, server_executable_name};
    use super::super::output::{GenerationProgress, GenerationProgressSink};
    use super::super::policy::HardwareGpu;
    use super::super::server::EngineHost;
    use super::*;

    struct Host;

    #[async_trait]
    impl EngineHost for Host {
        async fn gpu_devices(&self) -> Result<Vec<HardwareGpu>, String> {
            Ok(Vec::new())
        }

        fn available_memory_bytes(&self) -> Option<u64> {
            Some(64 * 1024 * 1024 * 1024)
        }

        async fn unload_local_llm(&self) -> Result<(), String> {
            Ok(())
        }
    }

    struct Silent;

    impl GenerationProgressSink for Silent {
        fn progress(&self, _: GenerationProgress) {}
    }

    const CPU_BUILD: &str = "sd-master-bin-Linux-Ubuntu-24.04-x86_64.zip";

    fn engine() -> (std::path::PathBuf, LocalDiffusionEngine) {
        let root = std::env::temp_dir().join(format!("sd-runnability-{}", OperationId::new()));
        let paths = DiffusionPaths::legacy_layout(&root, root.join("models").join("image"));
        let engine = LocalDiffusionEngine::new(
            paths,
            BulkHttpClient::new().expect("client"),
            Arc::new(Host),
            Arc::new(Silent),
        );
        (root, engine)
    }

    #[tokio::test]
    async fn engines_that_are_missing_or_too_old_stop_before_any_estimate() {
        let (root, engine) = engine();
        let bundle = RemoteBundleRunnabilityRequest {
            profile_id: "krea-2-turbo".to_owned(),
            runtime_release: "master-700-a".to_owned(),
            runtime_asset: CPU_BUILD.to_owned(),
            diffusion_bytes: 1,
            ..RemoteBundleRunnabilityRequest::default()
        };
        let old = engine
            .remote_bundle_runnability(&bundle)
            .await
            .expect("old");
        assert_eq!(old.status, RunnabilityStatus::IncompatibleRuntime);
        assert!(old.exact);
        let missing = engine
            .remote_bundle_runnability(&RemoteBundleRunnabilityRequest {
                profile_id: "z-image-turbo".to_owned(),
                ..bundle.clone()
            })
            .await
            .expect("missing");
        assert_eq!(missing.status, RunnabilityStatus::NotInstalled);
        assert_eq!(
            missing.reason,
            "Install a compatible stable-diffusion.cpp engine before downloading this image bundle."
        );
        assert_eq!(
            engine
                .remote_bundle_runnability(&RemoteBundleRunnabilityRequest {
                    profile_id: "nope".to_owned(),
                    ..bundle
                })
                .await,
            Err("Unknown local image architecture: nope".to_owned())
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn without_gpus_an_uninstalled_variant_is_estimated_on_the_cpu() {
        let (root, engine) = engine();
        let build = engine.paths().runtime_root("master-778-b", CPU_BUILD);
        std::fs::create_dir_all(&build).expect("build");
        std::fs::write(build.join(server_executable_name()), b"server").expect("server");
        let verdict = engine
            .catalog_runnability(CatalogRunnabilityRequest {
                profile_id: "z-image-turbo".to_owned(),
                variant_id: "q4-k".to_owned(),
                runtime_release: "master-778-b".to_owned(),
                runtime_asset: CPU_BUILD.to_owned(),
                ..CatalogRunnabilityRequest::default()
            })
            .await
            .expect("verdict");
        assert_eq!(verdict.scope, "preInstallEstimate");
        assert!(
            verdict.status == RunnabilityStatus::EstimatedRunnable
                || verdict.status == RunnabilityStatus::Inconclusive,
            "{verdict:?}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_blank_probe_reference_is_a_png_data_url() {
        let url = blank_reference_data_url(8, 4).expect("reference");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(url.strip_prefix("data:image/png;base64,").expect("prefix"))
            .expect("base64");
        let image = image::load_from_memory(&bytes).expect("png");
        assert_eq!((image.width(), image.height()), (8, 4));
    }
}
