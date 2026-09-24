//! The embedded stable-diffusion.cpp engine composed with the llama.cpp
//! runtime: each gives way to the other like legacy (the image server
//! unloads llama.cpp before it starts, llama.cpp stops the image server
//! before it runs).

use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use lettuce_image_generation::LOCAL_DIFFUSION_PROVIDER_KIND;
use lettuce_image_generation::sd_runtime::layout::DiffusionPaths;
use lettuce_image_generation::sd_runtime::output::GenerationProgressSink;
use lettuce_image_generation::sd_runtime::policy::HardwareGpu;
use lettuce_image_generation::sd_runtime::server::{EngineHost, LocalDiffusionEngine};
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

impl crate::AppBackend {
    /// The embedded stable-diffusion.cpp engine, when the host configured
    /// its folders.
    #[must_use]
    pub fn local_diffusion(&self) -> Option<&Arc<LocalDiffusionEngine>> {
        self.local_diffusion.as_ref()
    }
}

/// One installed catalog variant as the settings page lists it (legacy
/// `sdcpp_installed`).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledLocalImageModel {
    pub profile_id: String,
    pub variant_id: String,
    pub display_name: String,
    pub runtime_release: Option<String>,
    pub runtime_asset: Option<String>,
    pub runtime_backend: Option<String>,
    pub component_bytes_on_disk: u64,
    pub model_id: Option<lettuce_types::ModelProfileId>,
    pub supports_text_to_image: bool,
    pub supports_image_edit: bool,
    pub recommended_for_scenes: bool,
    pub requires_reference_image: bool,
    pub default_width: u32,
    pub default_height: u32,
    pub default_steps: u16,
    pub default_cfg: f32,
    pub model_path: String,
    pub components: Vec<InstalledLocalImageComponent>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledLocalImageComponent {
    pub role: lettuce_image_generation::DiffusionComponentRole,
    pub filename: String,
    pub path: String,
    pub bytes_on_disk: u64,
}

const DESKTOP_ENGINE_MISSING: &str = "Local stable-diffusion.cpp image generation is desktop-only.";

impl crate::AppBackend {
    fn engine(&self) -> Result<&Arc<LocalDiffusionEngine>, String> {
        self.local_diffusion
            .as_ref()
            .ok_or_else(|| DESKTOP_ENGINE_MISSING.to_owned())
    }

    fn local_image_model(
        &self,
        diffusion_path: &str,
        legacy_name: Option<(&str, &str)>,
    ) -> Result<Option<lettuce_models::ModelProfile>, String> {
        use lettuce_models::ModelLookup;
        let Some(account) = self
            .database()
            .account_by_kind_and_label(
                LOCAL_DIFFUSION_PROVIDER_KIND,
                crate::LOCAL_DIFFUSION_PROVIDER_LABEL,
            )
            .map_err(|error| error.to_string())?
        else {
            return Ok(None);
        };
        match (
            self.database()
                .profile_by_external_id(account.id, diffusion_path)
                .map_err(|error| error.to_string())?,
            legacy_name,
        ) {
            (Some(profile), _) => Ok(Some(profile)),
            (None, Some((profile_id, variant_id))) => self
                .database()
                .profile_by_external_id(account.id, &format!("sdcpp:{profile_id}:{variant_id}"))
                .map_err(|error| error.to_string()),
            (None, None) => Ok(None),
        }
    }

    /// Installed catalog variants with their registered models.
    pub fn installed_local_image_models(&self) -> Result<Vec<InstalledLocalImageModel>, String> {
        let engine = self.engine()?;
        let paths = engine.paths();
        let mut installed = Vec::new();
        for profile in &lettuce_image_generation::diffusion_catalog().profiles {
            for variant in &profile.variants {
                if !engine.variant_installed(profile, variant) {
                    continue;
                }
                let components = profile
                    .components(variant)
                    .into_iter()
                    .filter_map(|component| {
                        let path = paths.component_path(component);
                        let bytes_on_disk = std::fs::metadata(&path).ok()?.len();
                        Some(InstalledLocalImageComponent {
                            role: component.role,
                            filename: component.filename.clone(),
                            path: path.display().to_string(),
                            bytes_on_disk,
                        })
                    })
                    .collect::<Vec<_>>();
                let model_path = components
                    .iter()
                    .find(|component| {
                        component.role
                            == lettuce_image_generation::DiffusionComponentRole::DiffusionModel
                    })
                    .map(|component| component.path.clone())
                    .unwrap_or_default();
                let model = self.local_image_model(&model_path, None)?;
                let binding = model
                    .as_ref()
                    .map(|model| model.config.stable_diffusion.cpp.clone());
                let runtime_asset = binding
                    .as_ref()
                    .and_then(|binding| binding.runtime_asset.clone());
                installed.push(InstalledLocalImageModel {
                    profile_id: profile.id.clone(),
                    variant_id: variant.id.clone(),
                    display_name: profile.installed_display_name(variant),
                    runtime_release: binding.and_then(|binding| binding.runtime_release),
                    runtime_backend: runtime_asset.as_deref().and_then(|asset| {
                        lettuce_image_generation::sd_runtime::releases::runtime_backend(
                            asset,
                            engine.platform(),
                        )
                        .map(str::to_owned)
                    }),
                    runtime_asset,
                    component_bytes_on_disk: components
                        .iter()
                        .map(|component| component.bytes_on_disk)
                        .sum(),
                    model_id: model.map(|model| model.id),
                    supports_text_to_image: profile.supports_text_to_image,
                    supports_image_edit: profile.supports_image_edit,
                    recommended_for_scenes: profile.recommended_for_scenes,
                    requires_reference_image: profile.requires_reference_image,
                    default_width: profile.default_width,
                    default_height: profile.default_height,
                    default_steps: profile.default_steps,
                    default_cfg: profile.default_cfg,
                    model_path,
                    components,
                });
            }
        }
        Ok(installed)
    }

    /// Removes a catalog variant: the files no other installed variant uses,
    /// its model, and optionally an engine build no remaining model names
    /// (legacy `sdcpp_uninstall`; the build check reads each model's stored
    /// build instead of legacy's active build).
    pub async fn uninstall_local_image_model(
        &self,
        profile_id: &str,
        variant_id: &str,
        also_remove_engine_if_unused: bool,
    ) -> Result<(), String> {
        use lettuce_models::ModelProfileRepository;

        let engine = self.engine()?;
        let catalog = lettuce_image_generation::diffusion_catalog();
        let (profile, variant) = catalog
            .find_variant(profile_id, variant_id)
            .map_err(|error| error.to_string())?;
        let paths = engine.paths();
        let survivors = catalog
            .profiles
            .iter()
            .flat_map(|candidate| {
                candidate
                    .variants
                    .iter()
                    .map(move |other| (candidate, other))
            })
            .filter(|(candidate, other)| !(candidate.id == profile.id && other.id == variant.id))
            .filter(|(candidate, other)| engine.variant_installed(candidate, other))
            .collect::<Vec<_>>();
        let keep = survivors
            .iter()
            .flat_map(|(candidate, other)| candidate.components(other))
            .map(|component| component.sha256.clone())
            .collect::<std::collections::HashSet<_>>();
        for component in profile.components(variant) {
            if keep.contains(&component.sha256) {
                continue;
            }
            let path = paths.component_path(component);
            std::fs::remove_file(&path).ok();
            if let Some(parent) = path.parent() {
                std::fs::remove_dir(parent).ok();
            }
        }
        let diffusion = paths
            .component_path(&variant.diffusion)
            .display()
            .to_string();
        let model = self.local_image_model(&diffusion, Some((&profile.id, &variant.id)))?;
        let target = model
            .as_ref()
            .map(|model| {
                (
                    model.config.stable_diffusion.cpp.runtime_release.clone(),
                    model.config.stable_diffusion.cpp.runtime_asset.clone(),
                )
            })
            .unwrap_or_else(|| {
                let active = engine.installed_runtimes().into_iter().next();
                (
                    active.as_ref().map(|runtime| runtime.release.clone()),
                    active.map(|runtime| runtime.asset),
                )
            });
        if let Some(model) = model {
            self.database()
                .delete_and_clear_default(model.id)
                .map_err(|error| error.to_string())?;
        }
        if also_remove_engine_if_unused && let (Some(release), Some(asset)) = target {
            let still_used = survivors.iter().any(|(_, other)| {
                let path = paths.component_path(&other.diffusion).display().to_string();
                self.local_image_model(&path, None)
                    .ok()
                    .flatten()
                    .is_some_and(|model| {
                        let binding = &model.config.stable_diffusion.cpp;
                        binding.runtime_release.as_deref() == Some(release.as_str())
                            && binding.runtime_asset.as_deref() == Some(asset.as_str())
                    })
            });
            if !still_used {
                engine.stop().await;
                std::fs::remove_dir_all(paths.runtime_root(&release, &asset)).ok();
            }
        }
        Ok(())
    }

    /// Registers an installed variant again with the first complete engine
    /// build (legacy `sdcpp_repair_registration`).
    pub fn repair_local_image_registration(
        &self,
        profile_id: &str,
        variant_id: &str,
        now: lettuce_types::TimestampMillis,
    ) -> Result<lettuce_types::ModelProfileId, String> {
        let engine = self.engine()?;
        let (profile, variant) = lettuce_image_generation::diffusion_catalog()
            .find_variant(profile_id, variant_id)
            .map_err(|error| error.to_string())?;
        if !engine.variant_installed(profile, variant) {
            return Err(
                "The local image model is incomplete. Retry the installation first.".to_owned(),
            );
        }
        let runtime = engine
            .installed_runtimes()
            .into_iter()
            .next()
            .ok_or_else(|| {
                "No complete stable-diffusion.cpp engine build is installed.".to_owned()
            })?;
        crate::register_catalog_model(
            self.database(),
            engine.paths(),
            profile_id,
            variant_id,
            &runtime.release,
            &runtime.asset,
            now,
        )
        .map(|model| model.id)
        .map_err(|error| error.to_string())
    }

    /// The LoRA library below the engine's LoRA folder.
    pub fn lora_library(
        &self,
    ) -> Result<
        lettuce_image_generation::sd_runtime::lora_library::LoraLibrary<
            '_,
            lettuce_database::Database,
        >,
        String,
    > {
        Ok(
            lettuce_image_generation::sd_runtime::lora_library::LoraLibrary::new(
                &self.engine()?.paths().loras,
                self.database(),
            ),
        )
    }

    /// Upscales a stored image into a new asset (legacy
    /// `sdcpp_upscale_image`, which took the image bytes).
    pub async fn upscale_image<D: lettuce_image_generation::ImageMedia + ?Sized>(
        &self,
        media: &D,
        asset_id: lettuce_types::AssetId,
    ) -> Result<lettuce_image_generation::GeneratedImage, String> {
        let engine = self.engine()?;
        engine.check_upscale_ready()?;
        let input = media
            .load_input(asset_id)
            .map_err(|error| error.to_string())?;
        let upscaled = engine.upscale(&input.bytes).await?;
        media
            .ingest_derived(
                "upscale",
                lettuce_image_generation::ProviderImage {
                    bytes: upscaled,
                    declared_mime_type: Some("image/png".to_owned()),
                    text: None,
                },
            )
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use lettuce_image_generation::sd_runtime::layout::server_executable_name;
    use lettuce_image_generation::sd_runtime::lora_library::LoraKeywordSource;
    use lettuce_image_generation::sd_runtime::output::GenerationProgress;
    use lettuce_types::{OperationId, TimestampMillis};

    use super::*;

    struct Silent;

    impl GenerationProgressSink for Silent {
        fn progress(&self, _: GenerationProgress) {}
    }

    fn backend(root: &std::path::Path) -> crate::AppBackend {
        let paths = DiffusionPaths::legacy_layout(root, root.join("models").join("image"));
        crate::AppBackend::open(root.join("app.sqlite3"), TimestampMillis::new(1))
            .expect("backend")
            .with_local_diffusion(paths, Arc::new(Silent))
            .expect("engine")
    }

    #[tokio::test]
    async fn installed_models_register_list_and_uninstall_with_shared_files_kept() {
        let root = std::env::temp_dir().join(format!("sd-installed-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("root");
        let backend = backend(&root);
        let engine = backend.local_diffusion().expect("engine");
        let paths = engine.paths().clone();
        let build = paths.runtime_root("master-778-a", "a.zip");
        std::fs::create_dir_all(&build).expect("build");
        std::fs::write(build.join(server_executable_name()), b"server").expect("server");
        let catalog = lettuce_image_generation::diffusion_catalog();
        for variant_id in ["q3-k", "q4-k"] {
            let (profile, variant) = catalog
                .find_variant("z-image-turbo", variant_id)
                .expect("variant");
            for component in profile.components(variant) {
                let path = paths.component_path(component);
                std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
                std::fs::File::create(&path)
                    .and_then(|file| file.set_len(component.bytes))
                    .expect("sparse component");
            }
        }
        let model = crate::register_catalog_model(
            backend.database(),
            &paths,
            "z-image-turbo",
            "q4-k",
            "master-778-a",
            "a.zip",
            TimestampMillis::new(2),
        )
        .expect("register");
        let installed = backend.installed_local_image_models().expect("installed");
        assert_eq!(installed.len(), 2);
        let q4 = installed
            .iter()
            .find(|entry| entry.variant_id == "q4-k")
            .expect("q4");
        assert_eq!(q4.model_id, Some(model.id));
        assert_eq!(q4.runtime_release.as_deref(), Some("master-778-a"));
        assert_eq!(q4.components.len(), 3);
        let repaired = backend
            .repair_local_image_registration("z-image-turbo", "q4-k", TimestampMillis::new(3))
            .expect("repair");
        assert_eq!(repaired, model.id);

        backend
            .uninstall_local_image_model("z-image-turbo", "q4-k", true)
            .await
            .expect("uninstall");
        let (profile, q4) = catalog.find_variant("z-image-turbo", "q4-k").expect("q4");
        assert!(!paths.component_path(&q4.diffusion).exists());
        assert!(
            paths.component_path(&profile.shared_components[0]).exists(),
            "files another installed variant uses stay"
        );
        assert!(
            !build.exists(),
            "no remaining model names the build, as the unregistered survivor names none"
        );
        assert!(
            backend
                .installed_local_image_models()
                .expect("remaining")
                .is_empty(),
            "without an engine build nothing counts as installed, as in legacy"
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn the_lora_library_imports_lists_edits_and_deletes() {
        let root = std::env::temp_dir().join(format!("sd-lora-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("root");
        let backend = backend(&root);
        let header = serde_json::to_vec(&serde_json::json!({
            "__metadata__": {
                "ss_activation_tags": "ArsMovieStill",
                "ss_base_model_version": "z_image_turbo",
            }
        }))
        .expect("header");
        let padded = header.len().div_ceil(8) * 8;
        let mut file = (padded as u64).to_le_bytes().to_vec();
        file.extend_from_slice(&header);
        file.resize(8 + padded, b' ');
        file.extend_from_slice(&[0; 16]);
        let source = root.join("style.safetensors");
        std::fs::write(&source, &file).expect("lora");

        let library = backend.lora_library().expect("library");
        let imported = library
            .import(source.to_str().expect("path"))
            .await
            .expect("import");
        assert_eq!(imported.path, "style.safetensors");
        let listed = library
            .list(Some("z-image-turbo"), TimestampMillis::new(2))
            .expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].keywords, vec!["ArsMovieStill"]);
        assert_eq!(listed[0].keyword_source, LoraKeywordSource::Metadata);
        assert_eq!(listed[0].architecture.as_deref(), Some("z-image"));
        let edited = library
            .update_keywords(
                "style.safetensors",
                vec!["Manual".to_owned()],
                None,
                TimestampMillis::new(3),
            )
            .expect("edit");
        assert_eq!(edited.source, LoraKeywordSource::Manual);
        std::fs::write(&source, b"different").expect("different");
        assert!(
            library
                .import(source.to_str().expect("path"))
                .await
                .is_err_and(|error| error.starts_with("A different LoRA named style.safetensors"))
        );
        library.delete("style.safetensors").expect("delete");
        assert!(
            library
                .list(None, TimestampMillis::new(4))
                .expect("empty")
                .is_empty()
        );
        std::fs::remove_dir_all(root).ok();
    }
}
