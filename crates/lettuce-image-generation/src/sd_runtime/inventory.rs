//! What the local image settings page shows and changes: the catalog with
//! install state, engine builds, the active build, compute policies, model
//! file detection and disk usage.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::fit::FitDevice;
use super::layout::{
    ActiveRuntime, InstalledRuntime, clear_active_runtime, load_compute_policy,
    runtime_is_installed, save_active_runtime, save_compute_policy, saved_active_runtime,
};
use super::policy::ComputePolicy;
use super::releases::{RuntimeRelease, runtime_backend};
use super::server::LocalDiffusionEngine;
use crate::{DiffusionComponentRole, DiffusionProfile, DiffusionVariant, diffusion_catalog};

const NOT_INSTALLED_MESSAGE: &str =
    "The selected stable-diffusion.cpp engine build is not installed.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogVariantView {
    pub id: String,
    pub label: String,
    pub description: String,
    pub download_bytes: u64,
    pub installed: bool,
    pub recommended: bool,
    pub smaller: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogProfileView {
    pub id: String,
    pub display_name: String,
    pub family: String,
    pub description: String,
    pub license: String,
    pub source_url: String,
    pub supports_text_to_image: bool,
    pub supports_image_edit: bool,
    pub supports_lora: bool,
    pub max_reference_images: Option<u8>,
    pub requires_reference_image: bool,
    pub recommended_for_scenes: bool,
    pub default_width: u32,
    pub default_height: u32,
    pub default_steps: u16,
    pub default_cfg: f32,
    pub minimum_runtime_build: Option<u32>,
    pub variants: Vec<CatalogVariantView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogView {
    pub runtime_supported: bool,
    pub unsupported_reason: Option<String>,
    pub runtime_releases: Vec<RuntimeRelease>,
    pub profiles: Vec<CatalogProfileView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInventory {
    pub installed: Vec<InstalledRuntime>,
    pub active: Option<ActiveRuntime>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePolicyInfo {
    pub runtime_release: String,
    pub runtime_asset: String,
    pub backend: String,
    pub supports_row_split: bool,
    pub policy: ComputePolicy,
    pub devices: Vec<FitDevice>,
}

/// The bundle-assembly view of a catalog profile.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleProfileView {
    pub id: String,
    pub display_name: String,
    pub family: String,
    pub description: String,
    pub minimum_runtime_build: Option<u32>,
    pub required_roles: Vec<DiffusionComponentRole>,
    pub diffusion_markers: Vec<String>,
    pub encoder_markers: Vec<String>,
    pub encoder_parameter_billions: f32,
    pub recommended_repositories: std::collections::BTreeMap<DiffusionComponentRole, String>,
    pub supports_text_to_image: bool,
    pub supports_image_edit: bool,
    pub max_reference_images: Option<u8>,
    pub requires_reference_image: bool,
    pub recommended_for_scenes: bool,
    pub default_width: u32,
    pub default_height: u32,
    pub default_steps: u16,
    pub default_cfg: f32,
}

impl From<&DiffusionProfile> for BundleProfileView {
    fn from(profile: &DiffusionProfile) -> Self {
        Self {
            id: profile.id.clone(),
            display_name: profile.display_name.clone(),
            family: profile.family.clone(),
            description: profile.description.clone(),
            minimum_runtime_build: profile.minimum_runtime_build,
            required_roles: profile.required_roles(),
            diffusion_markers: profile.diffusion_markers.clone(),
            encoder_markers: profile.encoder_markers.clone(),
            encoder_parameter_billions: profile.encoder_parameter_billions,
            recommended_repositories: profile
                .recommended_repositories()
                .into_iter()
                .map(|(role, repository)| (role, repository.to_owned()))
                .collect(),
            supports_text_to_image: profile.supports_text_to_image,
            supports_image_edit: profile.supports_image_edit,
            max_reference_images: profile.max_reference_images,
            requires_reference_image: profile.requires_reference_image,
            recommended_for_scenes: profile.recommended_for_scenes,
            default_width: profile.default_width,
            default_height: profile.default_height,
            default_steps: profile.default_steps,
            default_cfg: profile.default_cfg,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedModelFile {
    pub exists: bool,
    pub profile: Option<BundleProfileView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskUsage {
    pub components_bytes: u64,
    pub runtimes_bytes: u64,
    pub loras_bytes: u64,
    pub total_bytes: u64,
    pub has_engine: bool,
    pub engine_release: Option<String>,
    pub engine_backend: Option<String>,
}

fn directory_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => directory_size(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map_or(0, |metadata| metadata.len()),
            _ => 0,
        })
        .sum()
}

impl LocalDiffusionEngine {
    /// Whether every file of a variant is on disk at its expected size and an
    /// engine build is installed.
    #[must_use]
    pub fn variant_installed(
        &self,
        profile: &DiffusionProfile,
        variant: &DiffusionVariant,
    ) -> bool {
        !self.installed_runtimes().is_empty()
            && profile.components(variant).iter().all(|component| {
                std::fs::metadata(self.paths().component_path(component))
                    .is_ok_and(|metadata| metadata.len() == component.bytes)
            })
    }

    /// The catalog with install state; `releases` are the platform's engine
    /// builds on GitHub.
    #[must_use]
    pub fn catalog_view(&self, releases: Vec<RuntimeRelease>) -> CatalogView {
        let supported = !releases.is_empty();
        CatalogView {
            runtime_supported: supported,
            unsupported_reason: (!supported).then(|| {
                "No stable-diffusion.cpp release assets match this operating system and architecture."
                    .to_owned()
            }),
            runtime_releases: releases,
            profiles: diffusion_catalog()
                .profiles
                .iter()
                .map(|profile| CatalogProfileView {
                    id: profile.id.clone(),
                    display_name: profile.display_name.clone(),
                    family: profile.family.clone(),
                    description: profile.description.clone(),
                    license: profile.license.clone(),
                    source_url: profile.source_url.clone(),
                    supports_text_to_image: profile.supports_text_to_image,
                    supports_image_edit: profile.supports_image_edit,
                    supports_lora: true,
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
                        .iter()
                        .map(|variant| CatalogVariantView {
                            id: variant.id.clone(),
                            label: variant.label.clone(),
                            description: variant.description.clone(),
                            download_bytes: profile.download_bytes(variant),
                            installed: self.variant_installed(profile, variant),
                            recommended: variant.recommended,
                            smaller: variant.smaller,
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    /// Installed builds and the active one; an implicit choice is saved.
    pub fn runtime_inventory(&self) -> Result<RuntimeInventory, String> {
        let mut installed = self.installed_runtimes();
        let saved = saved_active_runtime(self.paths());
        let active = saved.clone().or_else(|| {
            installed.first().map(|runtime| ActiveRuntime {
                release: runtime.release.clone(),
                asset: runtime.asset.clone(),
            })
        });
        if saved.is_none()
            && let Some(selected) = &active
        {
            save_active_runtime(self.paths(), selected)
                .map_err(|error| format!("Failed to save the active engine selection: {error}"))?;
        }
        if let Some(selected) = &active {
            for runtime in &mut installed {
                runtime.active =
                    runtime.release == selected.release && runtime.asset == selected.asset;
            }
        }
        Ok(RuntimeInventory { installed, active })
    }

    pub async fn switch_runtime(&self, release: &str, asset: &str) -> Result<(), String> {
        if !runtime_is_installed(self.paths(), release, asset) {
            return Err(NOT_INSTALLED_MESSAGE.to_owned());
        }
        save_active_runtime(
            self.paths(),
            &ActiveRuntime {
                release: release.to_owned(),
                asset: asset.to_owned(),
            },
        )
        .map_err(|error| format!("Failed to save the active engine selection: {error}"))?;
        self.stop().await;
        Ok(())
    }

    /// Deletes a build; deleting the active build selects the next one.
    pub async fn delete_runtime(&self, release: &str, asset: &str) -> Result<(), String> {
        if !runtime_is_installed(self.paths(), release, asset) {
            return Err(NOT_INSTALLED_MESSAGE.to_owned());
        }
        self.stop().await;
        let removed_active = self
            .effective_runtime()
            .is_some_and(|active| active.release == release && active.asset == asset);
        let root = self.paths().runtime_root(release, asset);
        std::fs::remove_dir_all(&root).map_err(|error| {
            format!("Failed to delete the stable-diffusion.cpp engine build: {error}")
        })?;
        if let Some(parent) = root.parent() {
            std::fs::remove_dir(parent).ok();
        }
        if removed_active {
            match self.installed_runtimes().first() {
                Some(next) => save_active_runtime(
                    self.paths(),
                    &ActiveRuntime {
                        release: next.release.clone(),
                        asset: next.asset.clone(),
                    },
                )
                .map_err(|error| format!("Failed to save the active engine selection: {error}"))?,
                None => clear_active_runtime(self.paths()).map_err(|error| {
                    format!("Failed to clear the active engine selection: {error}")
                })?,
            }
        }
        Ok(())
    }

    async fn compute_policy_info(
        &self,
        release: &str,
        asset: &str,
        policy: ComputePolicy,
    ) -> Result<ComputePolicyInfo, String> {
        if !runtime_is_installed(self.paths(), release, asset) {
            return Err(NOT_INSTALLED_MESSAGE.to_owned());
        }
        let resolved = self.resolve_compute_policy(release, asset, &policy).await?;
        Ok(ComputePolicyInfo {
            runtime_release: release.to_owned(),
            runtime_asset: asset.to_owned(),
            supports_row_split: resolved.backend == "cuda",
            backend: resolved.backend,
            policy,
            devices: resolved.available_devices,
        })
    }

    pub async fn compute_policy(
        &self,
        release: &str,
        asset: &str,
    ) -> Result<ComputePolicyInfo, String> {
        let policy = load_compute_policy(self.paths(), release, asset);
        self.compute_policy_info(release, asset, policy).await
    }

    /// Validates and saves a build's policy, then stops the server so the
    /// next generation uses it.
    pub async fn save_compute_policy(
        &self,
        release: &str,
        asset: &str,
        policy: ComputePolicy,
    ) -> Result<ComputePolicyInfo, String> {
        let info = self.compute_policy_info(release, asset, policy).await?;
        save_compute_policy(self.paths(), release, asset, &info.policy)
            .map_err(|error| format!("Failed to write the compute policy: {error}"))?;
        self.stop().await;
        Ok(info)
    }

    /// Recognises a user-picked diffusion file by its name and folder.
    #[must_use]
    pub fn detect_model_file(path: &str) -> DetectedModelFile {
        DetectedModelFile {
            exists: Path::new(path.trim()).is_file(),
            profile: diffusion_catalog()
                .detect_profile(path)
                .map(BundleProfileView::from),
        }
    }

    #[must_use]
    pub fn bundle_profiles() -> Vec<BundleProfileView> {
        diffusion_catalog()
            .profiles
            .iter()
            .map(BundleProfileView::from)
            .collect()
    }

    #[must_use]
    pub fn disk_usage(&self) -> DiskUsage {
        let components_bytes = directory_size(&self.paths().image_root.join("components"));
        let runtimes_bytes = directory_size(&self.paths().runtimes);
        let loras_bytes = directory_size(&self.paths().loras);
        let engine = self.installed_runtimes().into_iter().next();
        DiskUsage {
            components_bytes,
            runtimes_bytes,
            loras_bytes,
            total_bytes: components_bytes + runtimes_bytes + loras_bytes,
            has_engine: engine.is_some(),
            engine_backend: engine
                .as_ref()
                .and_then(|runtime| runtime_backend(&runtime.asset, self.platform()))
                .map(str::to_owned),
            engine_release: engine.map(|runtime| runtime.release),
        }
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
            None
        }

        async fn unload_local_llm(&self) -> Result<(), String> {
            Ok(())
        }
    }

    struct Silent;

    impl GenerationProgressSink for Silent {
        fn progress(&self, _: GenerationProgress) {}
    }

    fn engine() -> (std::path::PathBuf, LocalDiffusionEngine) {
        let root = std::env::temp_dir().join(format!("sd-inventory-{}", OperationId::new()));
        let paths = DiffusionPaths::legacy_layout(&root, root.join("models").join("image"));
        let engine = LocalDiffusionEngine::new(
            paths,
            BulkHttpClient::new().expect("client"),
            Arc::new(Host),
            Arc::new(Silent),
        );
        (root, engine)
    }

    fn install_build(engine: &LocalDiffusionEngine, release: &str, asset: &str) {
        let build = engine.paths().runtime_root(release, asset);
        std::fs::create_dir_all(&build).expect("build");
        std::fs::write(build.join(server_executable_name()), b"server").expect("server");
    }

    #[tokio::test]
    async fn builds_are_listed_selected_switched_and_deleted_like_legacy() {
        let (root, engine) = engine();
        assert_eq!(
            engine.runtime_inventory().expect("empty"),
            RuntimeInventory {
                installed: Vec::new(),
                active: None
            }
        );
        let cpu = "sd-master-bin-Linux-Ubuntu-24.04-x86_64.zip";
        let vulkan = "sd-master-bin-Linux-Ubuntu-24.04-x86_64-vulkan.zip";
        install_build(&engine, "master-700-a", cpu);
        install_build(&engine, "master-778-b", vulkan);
        let inventory = engine.runtime_inventory().expect("inventory");
        assert_eq!(
            inventory.active,
            Some(ActiveRuntime {
                release: "master-778-b".to_owned(),
                asset: vulkan.to_owned(),
            })
        );
        assert!(inventory.installed[0].active);
        assert!(
            saved_active_runtime(engine.paths()).is_some(),
            "the implicit choice is saved"
        );
        engine
            .switch_runtime("master-700-a", cpu)
            .await
            .expect("switch");
        assert_eq!(
            engine.effective_runtime().map(|active| active.release),
            Some("master-700-a".to_owned())
        );
        assert_eq!(
            engine.switch_runtime("master-1-x", cpu).await,
            Err("The selected stable-diffusion.cpp engine build is not installed.".to_owned())
        );
        engine
            .delete_runtime("master-700-a", cpu)
            .await
            .expect("delete");
        assert_eq!(
            saved_active_runtime(engine.paths()).map(|active| active.release),
            Some("master-778-b".to_owned())
        );
        engine
            .delete_runtime("master-778-b", vulkan)
            .await
            .expect("delete last");
        assert_eq!(saved_active_runtime(engine.paths()), None);
        assert!(!engine.paths().runtimes.join("master-778-b").exists());
        let usage = engine.disk_usage();
        assert!(!usage.has_engine);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn the_catalog_view_reports_install_state_and_the_unsupported_reason() {
        let (root, engine) = engine();
        let view = engine.catalog_view(Vec::new());
        assert!(!view.runtime_supported);
        assert_eq!(
            view.unsupported_reason.as_deref(),
            Some(
                "No stable-diffusion.cpp release assets match this operating system and architecture."
            )
        );
        assert_eq!(view.profiles.len(), 8);
        assert!(view.profiles.iter().all(|profile| profile.supports_lora));
        assert!(
            view.profiles
                .iter()
                .flat_map(|profile| &profile.variants)
                .all(|variant| !variant.installed)
        );
        let detected = LocalDiffusionEngine::detect_model_file("/nowhere/z_image_turbo-Q4_K.gguf");
        assert!(!detected.exists);
        assert_eq!(
            detected.profile.map(|profile| profile.id),
            Some("z-image-turbo".to_owned())
        );
        assert_eq!(LocalDiffusionEngine::bundle_profiles().len(), 8);
        std::fs::remove_dir_all(root).ok();
    }
}
