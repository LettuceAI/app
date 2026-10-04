//! The local stable-diffusion.cpp engine settings: the model catalog, engine
//! builds, installed models, compute policy, upscalers and the runnability
//! of a model on this machine.

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_image_generation::sd_runtime::fit::{
    FitComponent, FitDevice, FitEstimate, FitPlacement, PlanMode,
};
use lettuce_image_generation::sd_runtime::inventory::{BundleProfileView, ComputePolicyInfo};
use lettuce_image_generation::sd_runtime::layout::runtime_is_installed;
use lettuce_image_generation::sd_runtime::policy::ComputePolicy;
use lettuce_image_generation::sd_runtime::releases::{RuntimeAsset, RuntimeRelease};
use lettuce_image_generation::{
    DiffusionComponentRole, ImageError, ImageFailureKind, diffusion_catalog,
};

use super::{ApiContext, engine, image_error, internal};
use crate::api::error::{api_error, invalid_field};
use crate::api::jobs::{ImageToolDetail, admit_install, admit_tool};
use crate::api::local_models::{local_path, models_root};

pub(super) const fn role_dto(role: DiffusionComponentRole) -> dto::ImageComponentRole {
    match role {
        DiffusionComponentRole::DiffusionModel => dto::ImageComponentRole::DiffusionModel,
        DiffusionComponentRole::TextEncoder => dto::ImageComponentRole::TextEncoder,
        DiffusionComponentRole::Vae => dto::ImageComponentRole::Vae,
        DiffusionComponentRole::VisionEncoder => dto::ImageComponentRole::VisionEncoder,
    }
}

pub(super) const fn role_of(role: dto::ImageComponentRole) -> DiffusionComponentRole {
    match role {
        dto::ImageComponentRole::DiffusionModel => DiffusionComponentRole::DiffusionModel,
        dto::ImageComponentRole::TextEncoder => DiffusionComponentRole::TextEncoder,
        dto::ImageComponentRole::Vae => DiffusionComponentRole::Vae,
        dto::ImageComponentRole::VisionEncoder => DiffusionComponentRole::VisionEncoder,
    }
}

fn narrow(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn fit_device(device: FitDevice) -> dto::SdFitDevice {
    dto::SdFitDevice {
        id: narrow(device.id),
        name: device.name,
        description: device.description,
        total_bytes: device.total_bytes,
        free_bytes: device.free_bytes,
        budget_bytes: device.budget_bytes,
    }
}

pub(crate) fn fit_estimate(estimate: FitEstimate) -> dto::SdFitEstimate {
    dto::SdFitEstimate {
        model_bytes: estimate.model_bytes,
        available_ram_bytes: estimate.available_ram_bytes,
        plan_mode: match estimate.plan_mode {
            PlanMode::DefaultBackend => dto::SdPlanMode::DefaultBackend,
            PlanMode::Concurrent => dto::SdPlanMode::Concurrent,
            PlanMode::TimeShare => dto::SdPlanMode::TimeShare,
        },
        devices: estimate.devices.into_iter().map(fit_device).collect(),
        placements: estimate
            .placements
            .into_iter()
            .map(|placement: FitPlacement| dto::SdFitPlacement {
                component: match placement.component {
                    FitComponent::Dit => dto::SdFitComponent::Dit,
                    FitComponent::Vae => dto::SdFitComponent::Vae,
                    FitComponent::Conditioner => dto::SdFitComponent::Conditioner,
                },
                params_bytes: placement.params_bytes,
                compute_reserve_bytes: placement.compute_reserve_bytes,
                targets: placement.targets,
                cpu: placement.cpu,
                split: placement.split,
            })
            .collect(),
    }
}

fn runtime_release(release: RuntimeRelease) -> dto::SdRuntimeRelease {
    dto::SdRuntimeRelease {
        tag: release.tag,
        name: release.name,
        published_at: release.published_at,
        prerelease: release.prerelease,
        assets: release
            .assets
            .into_iter()
            .map(|asset| dto::SdRuntimeAsset {
                name: asset.name,
                backend: asset.backend,
                bytes: asset.bytes,
                sha256: asset.sha256,
                download_url: asset.download_url,
                dependencies: asset
                    .dependencies
                    .into_iter()
                    .map(|dependency| dto::SdRuntimeDependency {
                        name: dependency.name,
                        bytes: dependency.bytes,
                        sha256: dependency.sha256,
                        download_url: dependency.download_url,
                    })
                    .collect(),
            })
            .collect(),
    }
}

pub(crate) fn bundle_profile(view: BundleProfileView) -> dto::ImageBundleProfile {
    dto::ImageBundleProfile {
        id: view.id,
        display_name: view.display_name,
        family: view.family,
        description: view.description,
        minimum_runtime_build: view.minimum_runtime_build,
        required_roles: view.required_roles.into_iter().map(role_dto).collect(),
        diffusion_markers: view.diffusion_markers,
        encoder_markers: view.encoder_markers,
        encoder_parameter_billions: view.encoder_parameter_billions,
        recommended_repositories: view
            .recommended_repositories
            .into_iter()
            .map(|(role, repository)| dto::RecommendedRepository {
                role: role_dto(role),
                repository,
            })
            .collect(),
        supports_text_to_image: view.supports_text_to_image,
        supports_image_edit: view.supports_image_edit,
        max_reference_images: view.max_reference_images,
        requires_reference_image: view.requires_reference_image,
        recommended_for_scenes: view.recommended_for_scenes,
        default_width: view.default_width,
        default_height: view.default_height,
        default_steps: view.default_steps,
        default_cfg: view.default_cfg,
    }
}

fn policy_dto(policy: ComputePolicy) -> dto::SdComputePolicy {
    dto::SdComputePolicy {
        multi_gpu_enabled: policy.multi_gpu_enabled,
        gpu_device_ids: policy.gpu_device_ids.into_iter().map(narrow).collect(),
        single_gpu_device_id: policy.single_gpu_device_id.map(narrow),
        device_budgets_gib: policy
            .device_budgets_gib
            .into_iter()
            .map(|(device, gib)| dto::SdDeviceBudget {
                device_id: narrow(device),
                gib,
            })
            .collect(),
        split_mode: policy.split_mode,
    }
}

fn policy_of(policy: dto::SdComputePolicy) -> ComputePolicy {
    ComputePolicy {
        multi_gpu_enabled: policy.multi_gpu_enabled,
        gpu_device_ids: policy
            .gpu_device_ids
            .into_iter()
            .map(|id| id as usize)
            .collect(),
        single_gpu_device_id: policy.single_gpu_device_id.map(|id| id as usize),
        device_budgets_gib: policy
            .device_budgets_gib
            .into_iter()
            .map(|budget| (budget.device_id as usize, budget.gib))
            .collect(),
        split_mode: policy.split_mode,
    }
}

fn policy_info(info: ComputePolicyInfo) -> dto::SdComputePolicyInfo {
    dto::SdComputePolicyInfo {
        runtime_release: info.runtime_release,
        runtime_asset: info.runtime_asset,
        backend: info.backend,
        supports_row_split: info.supports_row_split,
        policy: policy_dto(info.policy),
        devices: info.devices.into_iter().map(fit_device).collect(),
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
async fn fetch_releases(context: &ApiContext) -> Result<Vec<RuntimeRelease>, ApiError> {
    let tls = context
        .backend()
        .tls_policy()
        .map_err(|error| internal(error))?;
    let client = lettuce_network::JsonClient::with_tls(&tls)
        .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?;
    crate::fetch_runtime_releases(&client)
        .await
        .map_err(|error| match error {
            crate::LocalDiffusionInstallError::Message(message) => {
                api_error(ApiErrorCode::Unavailable, message)
            }
            error @ crate::LocalDiffusionInstallError::Models(_) => internal(error),
        })
}

/// The model catalog with its install state, and the engine builds offered
/// for this platform.
pub async fn sd_catalog(context: &ApiContext) -> Result<dto::SdCatalog, ApiError> {
    let engine = engine(context)?;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let releases = fetch_releases(context).await?;
        let view = engine.catalog_view(releases);
        Ok(dto::SdCatalog {
            runtime_supported: view.runtime_supported,
            unsupported_reason: view.unsupported_reason,
            runtime_releases: view
                .runtime_releases
                .into_iter()
                .map(runtime_release)
                .collect(),
            profiles: view
                .profiles
                .into_iter()
                .map(|profile| dto::SdCatalogProfile {
                    id: profile.id,
                    display_name: profile.display_name,
                    family: profile.family,
                    description: profile.description,
                    license: profile.license,
                    source_url: profile.source_url,
                    supports_text_to_image: profile.supports_text_to_image,
                    supports_image_edit: profile.supports_image_edit,
                    supports_lora: profile.supports_lora,
                    max_reference_images: profile.max_reference_images,
                    requires_reference_image: profile.requires_reference_image,
                    recommended_for_scenes: profile.recommended_for_scenes,
                    default_width: profile.default_width,
                    default_height: profile.default_height,
                    default_steps: profile.default_steps,
                    default_cfg: profile.default_cfg,
                    minimum_runtime_build: profile.minimum_runtime_build,
                    variants: profile
                        .variants
                        .into_iter()
                        .map(|variant| dto::SdCatalogVariant {
                            id: variant.id,
                            label: variant.label,
                            description: variant.description,
                            download_bytes: variant.download_bytes,
                            installed: variant.installed,
                            recommended: variant.recommended,
                            smaller: variant.smaller,
                        })
                        .collect(),
                })
                .collect(),
        })
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = engine;
        Err(super::unsupported())
    }
}

/// The engine builds on GitHub that run on this platform.
pub async fn sd_runtime_releases(context: &ApiContext) -> Result<dto::SdRuntimeReleases, ApiError> {
    engine(context)?;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        Ok(dto::SdRuntimeReleases {
            releases: fetch_releases(context)
                .await?
                .into_iter()
                .map(runtime_release)
                .collect(),
        })
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    Err(super::unsupported())
}

/// Downloads and extracts an engine build; with `then_register`, a catalog
/// variant is registered once its files are in too. A build already
/// installed is `Conflict`.
pub async fn sd_runtime_install(
    context: &ApiContext,
    request: dto::SdRuntimeInstallRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let engine = engine(context)?;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let paths = engine.paths();
        if runtime_is_installed(&paths, &request.release, &request.asset) {
            return Err(api_error(
                ApiErrorCode::Conflict,
                "This stable-diffusion.cpp engine build is already installed.",
            ));
        }
        let releases = fetch_releases(context).await?;
        let release = releases
            .iter()
            .find(|release| release.tag == request.release)
            .ok_or_else(|| {
                api_error(ApiErrorCode::NotFound, "that engine release is not offered")
            })?;
        let asset: RuntimeAsset = release
            .assets
            .iter()
            .find(|asset| asset.name == request.asset)
            .cloned()
            .ok_or_else(|| api_error(ApiErrorCode::NotFound, "that engine build is not offered"))?;
        let plan = crate::runtime_install_plan(&paths, release, &asset).map_err(install_error)?;
        let variant = request
            .then_register
            .map(|variant| crate::api::CatalogVariant {
                profile_id: variant.profile_id,
                variant_id: variant.variant_id,
                runtime_release: release.tag.clone(),
                runtime_asset: asset.name.clone(),
            });
        admit_install(
            context,
            crate::api::InstallWork::Artifact {
                plan,
                finish: Box::new(crate::api::InstallFinish::StableDiffusionRuntime {
                    paths: (*paths).clone(),
                    release: release.tag.clone(),
                    asset,
                    variant,
                }),
            },
        )
        .await
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = (engine, request);
        Err(super::unsupported())
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn install_error(error: crate::LocalDiffusionInstallError) -> ApiError {
    match error {
        crate::LocalDiffusionInstallError::Message(message) => {
            api_error(ApiErrorCode::Unavailable, message)
        }
        error @ crate::LocalDiffusionInstallError::Models(_) => internal(error),
    }
}

pub async fn sd_runtime_inventory(
    context: &ApiContext,
) -> Result<dto::SdRuntimeInventory, ApiError> {
    let engine = engine(context)?;
    let inventory = context
        .blocking(move |_| engine.runtime_inventory().map_err(image_error))
        .await?;
    Ok(dto::SdRuntimeInventory {
        installed: inventory
            .installed
            .into_iter()
            .map(|runtime| dto::SdInstalledRuntime {
                release: runtime.release,
                asset: runtime.asset,
                backend: runtime.backend,
                size_bytes: runtime.size_bytes,
                active: runtime.active,
            })
            .collect(),
        active: inventory.active.map(|active| dto::SdRuntimeRef {
            release: active.release,
            asset: active.asset,
        }),
    })
}

/// Selects an installed build and stops the running server.
pub async fn sd_runtime_switch(
    context: &ApiContext,
    request: dto::SdRuntimeRef,
) -> Result<(), ApiError> {
    engine(context)?
        .switch_runtime(&request.release, &request.asset)
        .await
        .map_err(image_error)
}

/// Deletes an installed build; deleting the active one selects the next.
pub async fn sd_runtime_delete(
    context: &ApiContext,
    request: dto::SdRuntimeRef,
) -> Result<(), ApiError> {
    engine(context)?
        .delete_runtime(&request.release, &request.asset)
        .await
        .map_err(image_error)
}

fn variant_of(
    profile_id: &str,
    variant_id: &str,
) -> Result<
    (
        &'static lettuce_image_generation::DiffusionProfile,
        &'static lettuce_image_generation::DiffusionVariant,
    ),
    ApiError,
> {
    diffusion_catalog()
        .find_variant(profile_id, variant_id)
        .map_err(|error| invalid_field("variant_id", error.to_string()))
}

/// Downloads the files of a catalog variant and registers the model. When
/// every file is already on disk the model is registered at once and no job
/// is started.
pub async fn sd_model_install(
    context: &ApiContext,
    request: dto::SdModelInstallRequest,
) -> Result<dto::SdModelInstallStarted, ApiError> {
    let engine = engine(context)?;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let (profile, _) = variant_of(&request.profile_id, &request.variant_id)?;
        let paths = engine.paths();
        if !runtime_is_installed(&paths, &request.release, &request.asset) {
            return Err(image_error(ImageError::new(
                ImageFailureKind::RuntimeNotInstalled,
                "Install a stable-diffusion.cpp engine build before downloading a model.",
            )));
        }
        profile.check_runtime(&request.release).map_err(|error| {
            image_error(ImageError::new(
                ImageFailureKind::RuntimeIncompatible,
                error.to_string(),
            ))
        })?;
        let plan = crate::variant_install_plan(
            &paths,
            &request.profile_id,
            &request.variant_id,
            &request.release,
            &request.asset,
        )
        .map_err(install_error)?;
        let variant = crate::api::CatalogVariant {
            profile_id: request.profile_id,
            variant_id: request.variant_id,
            runtime_release: request.release,
            runtime_asset: request.asset,
        };
        if plan.artifacts.is_empty() {
            let now = context.now();
            let registered = context
                .blocking(move |context| {
                    crate::register_catalog_model(
                        context.backend().database(),
                        &paths,
                        &variant.profile_id,
                        &variant.variant_id,
                        &variant.runtime_release,
                        &variant.runtime_asset,
                        now,
                    )
                    .map(|model| model.id.to_string())
                    .map_err(|error| internal(error))
                })
                .await?;
            return Ok(dto::SdModelInstallStarted {
                job_id: None,
                model_id: Some(registered),
            });
        }
        let accepted = admit_install(
            context,
            crate::api::InstallWork::Artifact {
                plan,
                finish: Box::new(crate::api::InstallFinish::StableDiffusionVariant {
                    paths: (*paths).clone(),
                    variant,
                }),
            },
        )
        .await?;
        Ok(dto::SdModelInstallStarted {
            job_id: Some(accepted.job_id),
            model_id: None,
        })
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = (engine, request);
        Err(super::unsupported())
    }
}

/// The installed catalog variants with the models registered for them.
pub async fn sd_models_installed(context: &ApiContext) -> Result<dto::SdInstalledModels, ApiError> {
    engine(context)?;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let installed = context
            .blocking(|context| {
                context
                    .backend()
                    .installed_local_image_models()
                    .map_err(|message| internal(message))
            })
            .await?;
        Ok(dto::SdInstalledModels {
            models: installed
                .into_iter()
                .map(|model| dto::SdInstalledModel {
                    profile_id: model.profile_id,
                    variant_id: model.variant_id,
                    display_name: model.display_name,
                    runtime_release: model.runtime_release,
                    runtime_asset: model.runtime_asset,
                    runtime_backend: model.runtime_backend,
                    component_bytes_on_disk: model.component_bytes_on_disk,
                    model_id: model.model_id.map(|id| id.to_string()),
                    supports_text_to_image: model.supports_text_to_image,
                    supports_image_edit: model.supports_image_edit,
                    recommended_for_scenes: model.recommended_for_scenes,
                    requires_reference_image: model.requires_reference_image,
                    default_width: model.default_width,
                    default_height: model.default_height,
                    default_steps: model.default_steps,
                    default_cfg: model.default_cfg,
                    model_path: model.model_path,
                    components: model
                        .components
                        .into_iter()
                        .map(|component| dto::SdInstalledComponent {
                            role: role_dto(component.role),
                            filename: component.filename,
                            path: component.path,
                            bytes_on_disk: component.bytes_on_disk,
                        })
                        .collect(),
                })
                .collect(),
        })
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    Err(super::unsupported())
}

/// Removes a catalog variant's model and the files nothing else uses; a file
/// that cannot be removed is listed, not hidden.
pub async fn sd_model_uninstall(
    context: &ApiContext,
    request: dto::SdModelUninstallRequest,
) -> Result<dto::SdUninstallOutcome, ApiError> {
    engine(context)?;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        variant_of(&request.profile_id, &request.variant_id)?;
        let left_behind = context
            .backend()
            .uninstall_local_image_model(
                &request.profile_id,
                &request.variant_id,
                request.also_remove_engine_if_unused,
            )
            .await
            .map_err(|message| internal(message))?;
        Ok(dto::SdUninstallOutcome { left_behind })
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = request;
        Err(super::unsupported())
    }
}

/// Registers an installed variant again with the first complete engine
/// build.
pub async fn sd_model_repair(
    context: &ApiContext,
    request: dto::SdModelRepairRequest,
) -> Result<dto::SdModelRepaired, ApiError> {
    let engine = engine(context)?;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let (profile, variant) = variant_of(&request.profile_id, &request.variant_id)?;
        let installed = engine.installed_runtimes();
        if installed.is_empty() {
            return Err(image_error(ImageError::new(
                ImageFailureKind::RuntimeNotInstalled,
                "No complete stable-diffusion.cpp engine build is installed.",
            )));
        }
        if !engine.variant_installed(profile, variant) {
            return Err(image_error(ImageError::new(
                ImageFailureKind::ModelFileMissing,
                "The local image model is incomplete. Retry the installation first.",
            )));
        }
        let model_id = context
            .backend()
            .repair_local_image_registration(
                &request.profile_id,
                &request.variant_id,
                context.now(),
            )
            .map_err(|message| internal(message))?;
        Ok(dto::SdModelRepaired {
            model_id: model_id.to_string(),
        })
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = (engine, request);
        Err(super::unsupported())
    }
}

/// Tests whether a catalog variant runs on an engine build, as a job: an
/// estimate before the variant is installed, a real probe after (which
/// starts the engine).
pub async fn sd_runnability(
    context: &ApiContext,
    request: dto::SdRunnabilityRequest,
) -> Result<dto::JobAccepted, ApiError> {
    engine(context)?;
    variant_of(&request.profile_id, &request.variant_id)?;
    let operation = request.client_operation_id.clone();
    let mut stored = request;
    stored.client_operation_id.clear();
    context
        .blocking(move |context| {
            admit_tool(
                context,
                "sd_runnability",
                &operation,
                ImageToolDetail::Runnability { request: stored },
                || Ok(()),
            )
        })
        .await
}

/// Estimates how a bundle of files would run, as a job.
pub async fn sd_bundle_runnability(
    context: &ApiContext,
    request: dto::SdBundleRunnabilityRequest,
) -> Result<dto::JobAccepted, ApiError> {
    engine(context)?;
    diffusion_catalog()
        .profile(&request.profile_id)
        .map_err(|error| invalid_field("profile_id", error.to_string()))?;
    let operation = request.client_operation_id.clone();
    let mut stored = request;
    stored.client_operation_id.clear();
    context
        .blocking(move |context| {
            admit_tool(
                context,
                "sd_bundle_runnability",
                &operation,
                ImageToolDetail::BundleRunnability { request: stored },
                || Ok(()),
            )
        })
        .await
}

pub async fn sd_compute_policy_get(
    context: &ApiContext,
    request: dto::SdRuntimeRef,
) -> Result<dto::SdComputePolicyInfo, ApiError> {
    engine(context)?
        .compute_policy(&request.release, &request.asset)
        .await
        .map(policy_info)
        .map_err(image_error)
}

/// Validates and saves a build's compute policy and stops the server, so
/// the next generation uses it.
pub async fn sd_compute_policy_save(
    context: &ApiContext,
    request: dto::SdComputePolicySaveRequest,
) -> Result<dto::SdComputePolicyInfo, ApiError> {
    engine(context)?
        .save_compute_policy(&request.release, &request.asset, policy_of(request.policy))
        .await
        .map(policy_info)
        .map_err(image_error)
}

/// Recognises a picked diffusion file by its name and folder.
pub async fn sd_detect_model_file(
    context: &ApiContext,
    request: dto::SdDetectModelFileRequest,
) -> Result<dto::SdDetectedModelFile, ApiError> {
    engine(context)?;
    let path = local_path(&request.source, "source")?;
    let detected =
        lettuce_image_generation::sd_runtime::server::LocalDiffusionEngine::detect_model_file(
            &path,
        );
    Ok(dto::SdDetectedModelFile {
        exists: detected.exists,
        profile: detected.profile.map(bundle_profile),
    })
}

pub async fn sd_disk_usage(context: &ApiContext) -> Result<dto::SdDiskUsage, ApiError> {
    let engine = engine(context)?;
    let usage = context.blocking(move |_| Ok(engine.disk_usage())).await?;
    Ok(dto::SdDiskUsage {
        components_bytes: usage.components_bytes,
        runtimes_bytes: usage.runtimes_bytes,
        loras_bytes: usage.loras_bytes,
        total_bytes: usage.total_bytes,
        has_engine: usage.has_engine,
        engine_release: usage.engine_release,
        engine_backend: usage.engine_backend,
    })
}

/// The encoders and VAEs already on disk that an image model can reuse.
pub async fn sd_component_library(
    context: &ApiContext,
) -> Result<dto::SdComponentLibrary, ApiError> {
    let engine = engine(context)?;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        context
            .blocking(move |context| {
                let llm_root = models_root(context)?;
                let entries = crate::component_library(&engine.paths(), &llm_root)
                    .map_err(|message| internal(message))?;
                Ok(dto::SdComponentLibrary {
                    files: entries
                        .into_iter()
                        .map(|entry| dto::SdComponentFile {
                            path: entry.path,
                            filename: entry.filename,
                            bytes: entry.bytes,
                            role: entry.role.map(role_dto),
                            source: match entry.source {
                                crate::ComponentSource::ImageComponents => {
                                    dto::ComponentSource::ImageComponents
                                }
                                crate::ComponentSource::LlmLibrary => {
                                    dto::ComponentSource::LlmLibrary
                                }
                                crate::ComponentSource::ImageDownloads => {
                                    dto::ComponentSource::ImageDownloads
                                }
                            },
                        })
                        .collect(),
                })
            })
            .await
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = engine;
        Err(super::unsupported())
    }
}

fn upscaler_inventory(
    inventory: lettuce_image_generation::sd_runtime::upscale::UpscalerInventory,
) -> dto::SdUpscalerInventory {
    dto::SdUpscalerInventory {
        models: inventory.models,
        hires_upscaler_names: inventory.hires_upscaler_names,
        recommended_filename: inventory.recommended_filename,
        recommended_bytes: inventory.recommended_bytes,
        recommended_installed: inventory.recommended_installed,
    }
}

pub async fn sd_upscalers_list(context: &ApiContext) -> Result<dto::SdUpscalerInventory, ApiError> {
    Ok(upscaler_inventory(engine(context)?.upscaler_inventory()))
}

/// Downloads the recommended upscaler; one already installed is `Conflict`.
pub async fn sd_upscalers_install(context: &ApiContext) -> Result<dto::JobAccepted, ApiError> {
    let engine = engine(context)?;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        if engine.upscaler_inventory().recommended_installed {
            return Err(api_error(
                ApiErrorCode::Conflict,
                "The recommended upscaler is already installed.",
            ));
        }
        admit_install(
            context,
            crate::api::InstallWork::Artifact {
                plan: crate::upscaler_install_plan(&engine.paths()),
                finish: Box::new(crate::api::InstallFinish::Files),
            },
        )
        .await
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = engine;
        Err(super::unsupported())
    }
}

pub async fn sd_upscalers_remove(
    context: &ApiContext,
    request: dto::SdUpscalerRemoveRequest,
) -> Result<dto::SdUpscalerInventory, ApiError> {
    let engine = engine(context)?;
    context
        .blocking(move |_| {
            engine
                .remove_upscaler(&request.filename)
                .map(upscaler_inventory)
                .map_err(|message| {
                    if message == "Invalid upscaler file name." {
                        invalid_field("filename", message)
                    } else {
                        internal(message)
                    }
                })
        })
        .await
}
