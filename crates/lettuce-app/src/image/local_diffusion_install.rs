//! stable-diffusion.cpp engine builds, catalog models and the upscaler as
//! pinned installs, and the model registration when a catalog model finishes
//! downloading.

use std::path::PathBuf;

use lettuce_image_generation::sd_runtime::layout::{
    DiffusionPaths, extract_runtime_archive, runtime_is_installed, write_runtime_manifest,
};
use lettuce_image_generation::sd_runtime::releases::{
    GITHUB_REPOSITORY, GithubRelease, RuntimeAsset, RuntimeRelease, runtime_backend,
    runtime_releases,
};
use lettuce_image_generation::{
    DiffusionComponentRole, DiffusionProfile, DiffusionVariant, LOCAL_DIFFUSION_PROVIDER_KIND,
    diffusion_catalog,
};
use lettuce_model_hub::PinnedArtifact;
use lettuce_models::{
    CapabilityEvidence, CapabilityEvidenceSource, CapabilityStatus, ModalityCapabilities,
    ModelCapabilities, ModelKind, ModelLookup, ModelProfile, ModelProfileConfig,
    ModelProfileRepository, ModelRepositoryError, ProviderAccount, ProviderAccountRepository,
    ProviderConfig, ProviderProtocol, StableDiffusionCppBinding,
};
use lettuce_network::{JsonAuth, JsonClient, JsonQueryParameter, JsonStaticHeader, RequestPolicy};
use lettuce_settings::SecretOwnerId;
use lettuce_types::{ModelProfileId, ProviderAccountId, Revision, TimestampMillis};

use crate::{ArtifactInstallPlan, ArtifactSource, PlannedArtifact};

pub const LOCAL_DIFFUSION_PROVIDER_LABEL: &str = "stable-diffusion.cpp";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LocalDiffusionInstallError {
    #[error("{0}")]
    Message(String),
    #[error("the model catalog could not be updated")]
    Models(ModelRepositoryError),
}

impl From<String> for LocalDiffusionInstallError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl From<ModelRepositoryError> for LocalDiffusionInstallError {
    fn from(error: ModelRepositoryError) -> Self {
        Self::Models(error)
    }
}

/// The engine builds on GitHub that run on this platform.
pub async fn fetch_runtime_releases(
    client: &JsonClient,
) -> Result<Vec<RuntimeRelease>, LocalDiffusionInstallError> {
    let response = client
        .get_json_with_query(
            "https://api.github.com",
            &format!("/repos/{GITHUB_REPOSITORY}/releases"),
            &[JsonQueryParameter {
                name: "per_page",
                value: "20",
            }],
            &[
                JsonStaticHeader {
                    name: "user-agent",
                    value: "LettuceAI/1.0",
                },
                JsonStaticHeader {
                    name: "accept",
                    value: "application/vnd.github+json",
                },
                JsonStaticHeader {
                    name: "x-github-api-version",
                    value: "2022-11-28",
                },
            ],
            JsonAuth::None,
            Vec::new(),
            RequestPolicy::PROBE,
        )
        .await
        .map_err(|error| format!("Failed to fetch stable-diffusion.cpp releases: {error}"))?;
    if !(200..300).contains(&response.status) {
        return Err(format!(
            "GitHub release lookup failed with status {}",
            response.status
        )
        .into());
    }
    let releases = serde_json::from_slice::<Vec<GithubRelease>>(&response.body)
        .map_err(|error| format!("Failed to parse stable-diffusion.cpp releases: {error}"))?;
    Ok(runtime_releases(
        releases,
        lettuce_image_generation::sd_runtime::releases::RuntimePlatform::current(),
    ))
}

fn https_artifact(
    url: &str,
    local_segments: Vec<String>,
    bytes: u64,
    sha256: Option<&str>,
) -> PlannedArtifact {
    let source = ArtifactSource::Https {
        url: url.to_owned(),
    };
    PlannedArtifact {
        artifact: PinnedArtifact {
            source_identity: source.identity(),
            local_segments,
            byte_size: bytes,
            sha256: sha256.map(str::to_ascii_lowercase),
        },
        source,
    }
}

/// The archives of one engine build. Its manifest is written first, so a
/// partly extracted build never counts as installed.
pub fn runtime_install_plan(
    paths: &DiffusionPaths,
    release: &RuntimeRelease,
    asset: &RuntimeAsset,
) -> Result<ArtifactInstallPlan, LocalDiffusionInstallError> {
    if runtime_is_installed(paths, &release.tag, &asset.name) {
        return Err(
            "This stable-diffusion.cpp engine build is already installed."
                .to_owned()
                .into(),
        );
    }
    write_runtime_manifest(&paths.runtime_root(&release.tag, &asset.name), asset)?;
    let mut artifacts = vec![https_artifact(
        &asset.download_url,
        DiffusionPaths::archive_segments(&release.tag, &asset.name),
        asset.bytes,
        asset.sha256.as_deref(),
    )];
    artifacts.extend(asset.dependencies.iter().map(|dependency| {
        https_artifact(
            &dependency.download_url,
            DiffusionPaths::archive_segments(&release.tag, &dependency.name),
            dependency.bytes,
            dependency.sha256.as_deref(),
        )
    }));
    Ok(ArtifactInstallPlan {
        install_id: format!("sdcpp-runtime:{}:{}", release.tag, asset.name),
        root: paths.downloads.clone(),
        artifacts,
    })
}

/// Extracts every downloaded archive of an engine build.
pub async fn finish_runtime_install(
    paths: &DiffusionPaths,
    release: &str,
    asset: &RuntimeAsset,
    archives: Vec<PathBuf>,
) -> Result<(), LocalDiffusionInstallError> {
    let destination = paths.runtime_root(release, &asset.name);
    let names = std::iter::once(asset.name.clone())
        .chain(
            asset
                .dependencies
                .iter()
                .map(|dependency| dependency.name.clone()),
        )
        .collect::<Vec<_>>();
    if archives.len() != names.len() {
        return Err("A stable-diffusion.cpp runtime archive is missing."
            .to_owned()
            .into());
    }
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        for (archive, name) in archives.iter().zip(names) {
            extract_runtime_archive(archive, &name, &destination)?;
        }
        Ok(())
    })
    .await
    .map_err(|error| format!("Runtime extraction task failed: {error}"))?
    .map_err(Into::into)
}

/// Every file of a catalog variant not yet on disk, content-addressed below
/// the image folder; components already downloaded into the default folder
/// are reused. An empty plan means only the registration is left.
pub fn variant_install_plan(
    paths: &DiffusionPaths,
    profile_id: &str,
    variant_id: &str,
    runtime_release: &str,
    runtime_asset: &str,
) -> Result<ArtifactInstallPlan, LocalDiffusionInstallError> {
    if !runtime_is_installed(paths, runtime_release, runtime_asset) {
        return Err(
            "Install a stable-diffusion.cpp engine build before downloading a model."
                .to_owned()
                .into(),
        );
    }
    let (profile, variant) = diffusion_catalog()
        .find_variant(profile_id, variant_id)
        .map_err(|error| error.to_string())?;
    profile
        .check_runtime(runtime_release)
        .map_err(|error| error.to_string())?;
    Ok(ArtifactInstallPlan {
        install_id: format!(
            "sdcpp:{}:{}:{runtime_release}:{runtime_asset}",
            profile.id, variant.id
        ),
        root: paths.image_root.clone(),
        artifacts: profile
            .components(variant)
            .into_iter()
            .filter(|component| !in_default_folder(paths, component))
            .map(|component| {
                let source = ArtifactSource::HuggingFace {
                    repository: component.repo.clone(),
                    revision: component.revision.clone(),
                    path: component.filename.clone(),
                };
                PlannedArtifact {
                    artifact: PinnedArtifact {
                        source_identity: source.identity(),
                        local_segments: DiffusionPaths::component_segments(component),
                        byte_size: component.bytes,
                        sha256: Some(component.sha256.clone()),
                    },
                    source,
                }
            })
            .collect(),
    })
}

fn in_default_folder(
    paths: &DiffusionPaths,
    component: &lettuce_image_generation::DiffusionComponent,
) -> bool {
    let path = paths.component_path(component);
    paths
        .default_image_root
        .as_deref()
        .is_some_and(|default| path.starts_with(default))
        && std::fs::metadata(&path).is_ok_and(|metadata| metadata.len() == component.bytes)
}

/// The pinned RealESRGAN upscaler.
#[must_use]
pub fn upscaler_install_plan(paths: &DiffusionPaths) -> ArtifactInstallPlan {
    let upscaler = &diffusion_catalog().upscaler;
    ArtifactInstallPlan {
        install_id: format!("sdcpp-upscaler:{}", upscaler.filename),
        root: paths.upscalers.clone(),
        artifacts: vec![https_artifact(
            &upscaler.url,
            vec![upscaler.filename.clone()],
            upscaler.bytes,
            Some(&upscaler.sha256),
        )],
    }
}

/// The message for a failed upscaler install.
#[must_use]
pub fn upscaler_install_error_text(error: &crate::ArtifactInstallError) -> String {
    match error {
        crate::ArtifactInstallError::Store(lettuce_model_hub::PinnedArtifactError::Mismatch) => {
            "Upscaler model download failed its integrity check.".to_owned()
        }
        crate::ArtifactInstallError::Source(error) => {
            format!("Failed to download the upscaler model: {error}")
        }
        error => format!("Failed to store the upscaler model: {error}"),
    }
}

/// Whether every file of a variant is on disk at its expected size, with
/// the engine build installed.
#[must_use]
pub fn is_variant_installed(
    paths: &DiffusionPaths,
    profile: &DiffusionProfile,
    variant: &DiffusionVariant,
    runtime: Option<(&str, &str)>,
    any_runtime_installed: bool,
) -> bool {
    let runtime_installed = match runtime {
        Some((release, asset)) => runtime_is_installed(paths, release, asset),
        None => any_runtime_installed,
    };
    runtime_installed
        && profile.components(variant).iter().all(|component| {
            std::fs::metadata(paths.component_path(component))
                .is_ok_and(|metadata| metadata.len() == component.bytes)
        })
}

fn capabilities(profile: &DiffusionProfile, now: TimestampMillis) -> ModelCapabilities {
    ModelCapabilities {
        evidence: CapabilityEvidence {
            source: CapabilityEvidenceSource::Catalog,
            source_version: 1,
            observed_at: now,
        },
        input_modalities: ModalityCapabilities {
            text: CapabilityStatus::Supported,
            image: if profile.supports_image_edit {
                CapabilityStatus::Supported
            } else {
                CapabilityStatus::Unsupported
            },
            audio: CapabilityStatus::Unsupported,
        },
        output_modalities: ModalityCapabilities {
            text: CapabilityStatus::Unsupported,
            image: CapabilityStatus::Supported,
            audio: CapabilityStatus::Unsupported,
        },
        ..ModelCapabilities::unknown(CapabilityEvidence::default())
    }
}

/// The managed stable-diffusion.cpp account, created or re-enabled.
fn local_diffusion_account<R>(
    repository: &R,
    now: TimestampMillis,
) -> Result<ProviderAccount, LocalDiffusionInstallError>
where
    R: ModelLookup + ProviderAccountRepository + ?Sized,
{
    Ok(
        match repository.account_by_kind_and_label(
            LOCAL_DIFFUSION_PROVIDER_KIND,
            LOCAL_DIFFUSION_PROVIDER_LABEL,
        )? {
            Some(account) if account.enabled => account,
            Some(account) => {
                let revision = account.revision;
                ProviderAccountRepository::upsert(
                    repository,
                    ProviderAccount {
                        enabled: true,
                        updated_at: now,
                        ..account
                    },
                    Some(revision),
                )?
            }
            None => ProviderAccountRepository::upsert(
                repository,
                ProviderAccount {
                    id: ProviderAccountId::new(),
                    secret_owner_id: SecretOwnerId::new(),
                    provider_kind: LOCAL_DIFFUSION_PROVIDER_KIND.to_owned(),
                    protocol: ProviderProtocol::StableDiffusion,
                    label: LOCAL_DIFFUSION_PROVIDER_LABEL.to_owned(),
                    endpoint: None,
                    enabled: true,
                    streaming_enabled: true,
                    allow_invalid_tls: false,
                    api_key_ref: None,
                    secret_headers: Vec::new(),
                    config: ProviderConfig::Standard,
                    revision: Revision::INITIAL,
                    created_at: now,
                    updated_at: now,
                },
                None,
            )?,
        },
    )
}

/// Registers an installed catalog variant as a model on the managed
/// stable-diffusion.cpp account, updating the model an earlier install (or
/// an old `sdcpp:<profile>:<variant>` name) created. The binding keys are
/// rewritten and every other model setting is kept.
pub fn register_catalog_model<R>(
    repository: &R,
    paths: &DiffusionPaths,
    profile_id: &str,
    variant_id: &str,
    runtime_release: &str,
    runtime_asset: &str,
    now: TimestampMillis,
) -> Result<ModelProfile, LocalDiffusionInstallError>
where
    R: ModelLookup + ModelProfileRepository + ProviderAccountRepository + ?Sized,
{
    let (profile, variant) = diffusion_catalog()
        .find_variant(profile_id, variant_id)
        .map_err(|error| error.to_string())?;
    let account = local_diffusion_account(repository, now)?;
    let path_of = |role| {
        profile
            .components(variant)
            .into_iter()
            .find(|component| component.role == role)
            .map(|component| paths.component_path(component).display().to_string())
    };
    let diffusion = path_of(DiffusionComponentRole::DiffusionModel).ok_or_else(|| {
        format!(
            "{} does not define a diffusion_model component",
            profile.display_name
        )
    })?;
    let existing = match repository.profile_by_external_id(account.id, &diffusion)? {
        Some(existing) => Some(existing),
        None => repository
            .profile_by_external_id(account.id, &format!("sdcpp:{}:{}", profile.id, variant.id))?,
    };
    let binding = StableDiffusionCppBinding {
        profile_id: Some(profile.id.clone()),
        variant_id: Some(variant.id.clone()),
        text_encoder_path: path_of(DiffusionComponentRole::TextEncoder),
        vae_path: path_of(DiffusionComponentRole::Vae),
        vision_encoder_path: path_of(DiffusionComponentRole::VisionEncoder),
        runtime_release: Some(runtime_release.to_owned()),
        runtime_asset: Some(runtime_asset.to_owned()),
        runtime_backend: runtime_backend(
            runtime_asset,
            lettuce_image_generation::sd_runtime::releases::RuntimePlatform::current(),
        )
        .map(str::to_owned),
        max_reference_images: profile.max_reference_images.map(u32::from),
        supports_lora: Some(true),
        supports_text_to_image: Some(profile.supports_text_to_image),
        supports_image_edit: Some(profile.supports_image_edit),
        recommended_for_scenes: Some(profile.recommended_for_scenes),
        requires_reference_image: Some(profile.requires_reference_image),
    };
    let (id, revision, created_at, mut config) = match existing {
        Some(existing) => (
            existing.id,
            Some(existing.revision),
            existing.created_at,
            existing.config,
        ),
        None => (
            ModelProfileId::new(),
            None,
            now,
            ModelProfileConfig {
                chat_parameters: Default::default(),
                feature_parameters: Default::default(),
                capabilities: ModelCapabilities::default(),
                llama_cpp: Default::default(),
                stable_diffusion: Default::default(),
            },
        ),
    };
    config.stable_diffusion.cpp = binding;
    let kept = config.capabilities.clone();
    config.capabilities = ModelCapabilities {
        streaming: kept.streaming,
        tools: kept.tools,
        structured_output: kept.structured_output,
        reasoning: kept.reasoning,
        prompt_cache: kept.prompt_cache,
        context_length: kept.context_length,
        max_visible_output_tokens: kept.max_visible_output_tokens,
        max_total_completion_tokens: kept.max_total_completion_tokens,
        parameter_support: kept.parameter_support,
        ..capabilities(profile, now)
    };
    Ok(ModelProfileRepository::upsert(
        repository,
        ModelProfile {
            id,
            provider_account_id: account.id,
            external_model_id: diffusion,
            display_name: profile.installed_display_name(variant),
            kind: ModelKind::Image,
            config,
            revision: revision.unwrap_or(Revision::INITIAL),
            created_at,
            updated_at: now,
        },
        revision,
    )?)
}

/// The files of a bundle assembled from Hugging Face, as downloaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HfBundleRegistration<'a> {
    pub profile_id: &'a str,
    pub display_name: &'a str,
    pub diffusion_path: &'a str,
    pub text_encoder_path: &'a str,
    pub vae_path: &'a str,
    pub vision_encoder_path: Option<&'a str>,
    pub runtime_release: &'a str,
    pub runtime_asset: &'a str,
}

/// Registers a downloaded bundle as a model on the managed
/// stable-diffusion.cpp account. A model already registered for the same
/// diffusion file keeps its name, settings and every binding value it
/// already has; only missing ones are filled in.
pub fn register_hf_bundle_model<R>(
    repository: &R,
    paths: &DiffusionPaths,
    registration: HfBundleRegistration<'_>,
    now: TimestampMillis,
) -> Result<ModelProfile, LocalDiffusionInstallError>
where
    R: ModelLookup + ModelProfileRepository + ProviderAccountRepository + ?Sized,
{
    let profile = diffusion_catalog()
        .profile(registration.profile_id)
        .map_err(|_| {
            format!(
                "Unknown local image architecture: {}",
                registration.profile_id
            )
        })?;
    profile
        .check_runtime(registration.runtime_release)
        .map_err(|error| error.to_string())?;
    if !lettuce_image_generation::sd_runtime::layout::runtime_is_installed(
        paths,
        registration.runtime_release,
        registration.runtime_asset,
    ) {
        return Err(
            "The selected stable-diffusion.cpp engine is no longer installed."
                .to_owned()
                .into(),
        );
    }
    let display_name = registration.display_name.trim();
    let account = local_diffusion_account(repository, now)?;
    let existing = repository.profile_by_external_id(account.id, registration.diffusion_path)?;
    let (id, revision, created_at, name, mut config) = match existing {
        Some(existing) => (
            existing.id,
            Some(existing.revision),
            existing.created_at,
            existing.display_name,
            existing.config,
        ),
        None if display_name.is_empty() => {
            return Err("The model display name cannot be empty.".to_owned().into());
        }
        None => (
            ModelProfileId::new(),
            None,
            now,
            display_name.to_owned(),
            ModelProfileConfig {
                chat_parameters: Default::default(),
                feature_parameters: Default::default(),
                capabilities: capabilities(profile, now),
                llama_cpp: Default::default(),
                stable_diffusion: Default::default(),
            },
        ),
    };
    let binding = &mut config.stable_diffusion.cpp;
    fn fill<T>(slot: &mut Option<T>, value: T) {
        if slot.is_none() {
            *slot = Some(value);
        }
    }
    fill(&mut binding.profile_id, profile.id.clone());
    fill(
        &mut binding.text_encoder_path,
        registration.text_encoder_path.to_owned(),
    );
    fill(&mut binding.vae_path, registration.vae_path.to_owned());
    if let Some(vision) = registration.vision_encoder_path {
        fill(&mut binding.vision_encoder_path, vision.to_owned());
    }
    fill(
        &mut binding.runtime_release,
        registration.runtime_release.to_owned(),
    );
    fill(
        &mut binding.runtime_asset,
        registration.runtime_asset.to_owned(),
    );
    if let Some(backend) = runtime_backend(
        registration.runtime_asset,
        lettuce_image_generation::sd_runtime::releases::RuntimePlatform::current(),
    ) {
        fill(&mut binding.runtime_backend, backend.to_owned());
    }
    if let Some(maximum) = profile.max_reference_images {
        fill(&mut binding.max_reference_images, u32::from(maximum));
    }
    fill(&mut binding.supports_lora, true);
    fill(
        &mut binding.supports_text_to_image,
        profile.supports_text_to_image,
    );
    fill(
        &mut binding.supports_image_edit,
        profile.supports_image_edit,
    );
    fill(
        &mut binding.recommended_for_scenes,
        profile.recommended_for_scenes,
    );
    fill(
        &mut binding.requires_reference_image,
        profile.requires_reference_image,
    );
    binding.variant_id = None;
    let settings = &mut config.stable_diffusion;
    fill(
        &mut settings.size,
        format!("{}x{}", profile.default_width, profile.default_height),
    );
    fill(&mut settings.steps, u32::from(profile.default_steps));
    fill(&mut settings.cfg_scale, f64::from(profile.default_cfg));
    Ok(ModelProfileRepository::upsert(
        repository,
        ModelProfile {
            id,
            provider_account_id: account.id,
            external_model_id: registration.diffusion_path.to_owned(),
            display_name: name,
            kind: ModelKind::Image,
            config,
            revision: revision.unwrap_or(Revision::INITIAL),
            created_at,
            updated_at: now,
        },
        revision,
    )?)
}

#[cfg(test)]
mod tests {
    use lettuce_database::Database;
    use lettuce_image_generation::sd_runtime::layout::server_executable_name;
    use lettuce_image_generation::sd_runtime::policy::HardwareGpu;
    use lettuce_image_generation::sd_runtime::server::{EngineHost, LocalDiffusionEngine};
    use lettuce_types::OperationId;

    use super::*;

    #[test]
    fn catalog_models_register_once_and_keep_user_settings() {
        let root = std::env::temp_dir().join(format!("sd-register-{}", OperationId::new()));
        let paths = DiffusionPaths::legacy_layout(&root, root.join("models").join("image"));
        let database = Database::open_in_memory().expect("database");
        let first = register_catalog_model(
            &database,
            &paths,
            "qwen-image-edit-2511",
            "q3-k-m",
            "master-778-c00a9e9",
            "sd-master-c00a9e9-bin-Linux-Ubuntu-24.04-x86_64-vulkan.zip",
            TimestampMillis::new(10),
        )
        .expect("register");
        assert_eq!(first.display_name, "Qwen Image Edit 2511 (Q3 K M)");
        assert_eq!(first.kind, ModelKind::Image);
        assert!(first.external_model_id.ends_with(".gguf"));
        let binding = &first.config.stable_diffusion.cpp;
        assert!(binding.vision_encoder_path.is_some());
        assert_eq!(binding.requires_reference_image, Some(true));
        assert_eq!(binding.max_reference_images, None);
        assert_eq!(
            first.config.capabilities.input_modalities.image,
            CapabilityStatus::Supported
        );
        assert_eq!(
            first.config.capabilities.output_modalities.image,
            CapabilityStatus::Supported
        );

        let mut edited = first.clone();
        edited.config.stable_diffusion.steps = Some(30);
        let edited = ModelProfileRepository::upsert(&database, edited, Some(first.revision))
            .expect("user edit");
        let again = register_catalog_model(
            &database,
            &paths,
            "qwen-image-edit-2511",
            "q3-k-m",
            "master-779-aaaaaaa",
            "sd-master-aaaaaaa-bin-Linux-Ubuntu-24.04-x86_64-vulkan.zip",
            TimestampMillis::new(20),
        )
        .expect("register again");
        assert_eq!(again.id, first.id);
        assert_eq!(again.revision, edited.revision.next().expect("revision"));
        assert_eq!(again.config.stable_diffusion.steps, Some(30));
        assert_eq!(
            again.config.stable_diffusion.cpp.runtime_release.as_deref(),
            Some("master-779-aaaaaaa")
        );
        assert_eq!(
            again.provider_account_id, first.provider_account_id,
            "the managed account is reused"
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn variant_installs_need_a_compatible_engine_and_use_legacy_paths() {
        let root = std::env::temp_dir().join(format!("sd-plan-{}", OperationId::new()));
        let paths = DiffusionPaths::legacy_layout(&root, root.join("image"));
        assert_eq!(
            variant_install_plan(&paths, "z-image-turbo", "q4-k", "master-1-a", "a.zip")
                .map(|_| ())
                .map_err(|error| error.to_string()),
            Err(
                "Install a stable-diffusion.cpp engine build before downloading a model."
                    .to_owned()
            )
        );
        let build = paths.runtime_root("master-700-a", "a.zip");
        std::fs::create_dir_all(&build).expect("build");
        std::fs::write(build.join(server_executable_name()), b"server").expect("server");
        assert_eq!(
            variant_install_plan(&paths, "krea-2-turbo", "q4-k-m", "master-700-a", "a.zip")
                .map(|_| ())
                .map_err(|error| error.to_string()),
            Err("Krea 2 Turbo requires stable-diffusion.cpp engine build 721 or newer. Install or switch to a compatible engine build first.".to_owned())
        );
        let plan = variant_install_plan(&paths, "z-image-turbo", "q4-k", "master-700-a", "a.zip")
            .expect("plan");
        assert_eq!(
            plan.install_id,
            "sdcpp:z-image-turbo:q4-k:master-700-a:a.zip"
        );
        assert_eq!(plan.artifacts.len(), 3);
        assert_eq!(
            plan.artifacts[2].artifact.local_segments[2],
            "ae.safetensors"
        );
        assert_eq!(
            plan.artifacts[2].source,
            ArtifactSource::HuggingFace {
                repository: "Comfy-Org/z_image_turbo".to_owned(),
                revision: "d24c4cf2a0cd98a42f23467e27e3d76ee9438b8e".to_owned(),
                path: "split_files/vae/ae.safetensors".to_owned(),
            }
        );
        let upscaler = upscaler_install_plan(&paths);
        assert_eq!(upscaler.artifacts[0].artifact.byte_size, 17_938_799);

        let moved = DiffusionPaths::legacy_layout(&root, root.join("moved-image"));
        let (profile, variant) = diffusion_catalog()
            .find_variant("z-image-turbo", "q4-k")
            .expect("variant");
        std::fs::create_dir_all(moved.runtime_root("master-700-a", "a.zip")).expect("build");
        std::fs::write(
            moved
                .runtime_root("master-700-a", "a.zip")
                .join(server_executable_name()),
            b"server",
        )
        .expect("server");
        for component in profile.components(variant).into_iter().take(2) {
            let legacy_path = root
                .join("models/image")
                .join(DiffusionPaths::component_segments(component).join("/"));
            std::fs::create_dir_all(legacy_path.parent().expect("parent")).expect("folder");
            std::fs::File::create(&legacy_path)
                .and_then(|file| file.set_len(component.bytes))
                .expect("sparse legacy component");
        }
        let plan = variant_install_plan(&moved, "z-image-turbo", "q4-k", "master-700-a", "a.zip")
            .expect("plan");
        assert_eq!(
            plan.artifacts.len(),
            1,
            "components in the default folder are reused"
        );
        assert_eq!(
            plan.artifacts[0].artifact.local_segments[2],
            "ae.safetensors"
        );
        assert!(
            moved
                .component_path(&variant.diffusion)
                .starts_with(root.join("models/image"))
        );
        std::fs::remove_dir_all(root).ok();
    }

    struct NoProgress;

    struct EngineDevices(Vec<HardwareGpu>);

    #[async_trait::async_trait]
    impl EngineHost for EngineDevices {
        async fn gpu_devices(&self) -> Result<Vec<HardwareGpu>, String> {
            Ok(self.0.clone())
        }

        fn available_memory_bytes(&self) -> Option<u64> {
            None
        }

        async fn unload_local_llm(&self) -> Result<(), String> {
            Ok(())
        }
    }

    impl lettuce_image_generation::sd_runtime::output::GenerationProgressSink for NoProgress {
        fn progress(&self, _: lettuce_image_generation::sd_runtime::output::GenerationProgress) {}
    }

    #[tokio::test]
    #[ignore = "needs a legacy app folder with an sd.cpp engine archive and FLUX.2 Klein 4B in LETTUCE_SD_APP_DIR"]
    async fn a_registered_catalog_model_generates_through_the_image_job() {
        use std::sync::Arc;
        use std::time::Duration;

        use lettuce_image_generation::{
            ImageAttribution, ImageGenerationRequest, ImageGenerationSource, ImageGenerationState,
            ImageOutputPolicy,
        };
        use lettuce_jobs::{CancellationReason, ResourceAvailability, WorkerId};
        use lettuce_media::LocalMediaBlobStore;
        use lettuce_platform::{DirectorySnapshot, FilesystemAuthority, ManagedRoot};
        use lettuce_types::RequestId;

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
            .expect("release")
            .flatten()
            .map(|entry| entry.path())
            .find(|path| path.extension().is_some_and(|extension| extension == "zip"))
            .expect("archive");
        let release = archive_dir
            .file_name()
            .and_then(|name| name.to_str())
            .expect("release")
            .to_owned();
        let asset_name = archive
            .file_name()
            .and_then(|name| name.to_str())
            .expect("asset")
            .to_owned();
        let root = std::env::temp_dir().join(format!("sd-job-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("root");
        let paths = DiffusionPaths::legacy_layout(&root, app_dir.join("models/image"));
        let asset = RuntimeAsset {
            name: asset_name.clone(),
            backend: "vulkan".to_owned(),
            bytes: 1,
            sha256: None,
            download_url: String::new(),
            dependencies: Vec::new(),
        };
        write_runtime_manifest(&paths.runtime_root(&release, &asset_name), &asset)
            .expect("manifest");
        finish_runtime_install(&paths, &release, &asset, vec![archive])
            .await
            .expect("extract");
        let database_path = root.join("app.sqlite3");
        let backend =
            crate::AppBackend::open(&database_path, TimestampMillis::new(1)).expect("backend");
        let probe = LocalDiffusionEngine::new(
            paths.clone(),
            lettuce_network::BulkHttpClient::new().expect("client"),
            Arc::new(EngineDevices(Vec::new())),
            Arc::new(NoProgress),
        );
        let hardware = probe
            .runtime_devices(&release, &asset_name)
            .await
            .expect("engine devices")
            .into_iter()
            .enumerate()
            .map(|(index, device)| HardwareGpu {
                index,
                name: device.name,
                description: device.description,
                memory_total: 8 << 30,
                memory_free: 6 << 30,
            })
            .collect();
        let engine = Arc::new(LocalDiffusionEngine::new(
            paths.clone(),
            lettuce_network::BulkHttpClient::new().expect("client"),
            Arc::new(EngineDevices(hardware)),
            Arc::new(NoProgress),
        ));
        let providers = crate::AppImageProviders {
            local: Some(Arc::clone(&engine)),
            remote: None,
        };
        let model = register_catalog_model(
            backend.database(),
            &paths,
            "flux-2-klein-4b",
            "q4-0",
            &release,
            &asset_name,
            TimestampMillis::new(2),
        )
        .expect("register");
        let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
            .expect("authority");
        let media = LocalMediaBlobStore::new(
            authority.managed_files(),
            authority
                .read_capability(ManagedRoot::MediaBlobs)
                .expect("read"),
            authority
                .write_capability(ManagedRoot::MediaBlobs)
                .expect("write"),
            lettuce_database::Database::open(&database_path).expect("blobs"),
            lettuce_database::Database::open(&database_path).expect("assets"),
        );
        let coordinator = backend.image_generations();
        let admitted = coordinator
            .admit(
                ImageGenerationRequest {
                    id: RequestId::new(),
                    model_profile_id: model.id,
                    prompt: "a small green frog on a lily pad".to_owned(),
                    settings: lettuce_models::StableDiffusionSettings {
                        seed: Some(3),
                        ..Default::default()
                    },
                    input_images: Vec::new(),
                    mask_image: None,
                    loras: Vec::new(),
                    size: Some("512x512".to_owned()),
                    quality: None,
                    style: None,
                    count: 1,
                    source: ImageGenerationSource::Playground,
                    attribution: ImageAttribution::default(),
                    output_policy: ImageOutputPolicy::Retained,
                    created_at: TimestampMillis::new(3),
                },
                backend.database(),
            )
            .expect("admit");
        let work = coordinator
            .claim(
                admitted.job.id,
                WorkerId::new(),
                TimestampMillis::new(4),
                Duration::from_secs(600),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("work");
        let result = coordinator
            .run(
                work,
                backend.database(),
                &media,
                &providers,
                CancellationReason::User,
                TimestampMillis::new(5),
            )
            .await
            .expect("run");
        let crate::ImageGenerationRunResult::Succeeded { record, .. } = result else {
            panic!("expected success: {result:?}");
        };
        let ImageGenerationState::Succeeded { result } = record.state else {
            panic!("expected images");
        };
        assert_eq!(result.images.len(), 1);
        assert_eq!(result.images[0].mime_type, "image/png");
        assert_eq!(
            (result.images[0].width, result.images[0].height),
            (Some(512), Some(512))
        );
        engine.shutdown().await;
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    #[ignore = "downloads the pinned upscaler and needs an sd.cpp engine archive in LETTUCE_SD_APP_DIR"]
    async fn the_pinned_upscaler_installs_and_upscales_with_the_real_engine() {
        use std::sync::Arc;
        use std::time::Duration;

        use lettuce_jobs::{CancellationReason, ResourceAvailability, WorkerId};

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
            .expect("release")
            .flatten()
            .map(|entry| entry.path())
            .find(|path| path.extension().is_some_and(|extension| extension == "zip"))
            .expect("archive");
        let release = archive_dir
            .file_name()
            .and_then(|name| name.to_str())
            .expect("release")
            .to_owned();
        let asset_name = archive
            .file_name()
            .and_then(|name| name.to_str())
            .expect("asset")
            .to_owned();
        let root = std::env::temp_dir().join(format!("sd-upscale-{}", OperationId::new()));
        let paths = DiffusionPaths::legacy_layout(&root, root.join("image"));
        let asset = RuntimeAsset {
            name: asset_name.clone(),
            backend: "vulkan".to_owned(),
            bytes: 1,
            sha256: None,
            download_url: String::new(),
            dependencies: Vec::new(),
        };
        write_runtime_manifest(&paths.runtime_root(&release, &asset_name), &asset)
            .expect("manifest");
        finish_runtime_install(&paths, &release, &asset, vec![archive])
            .await
            .expect("extract");

        let database = Database::open_in_memory().expect("database");
        let installer = crate::ArtifactInstallCoordinator::new(&database);
        let plan = upscaler_install_plan(&paths);
        let admitted = installer.admit(&plan).expect("admit");
        let work = installer
            .claim(
                plan,
                admitted.job.id,
                WorkerId::new(),
                TimestampMillis::new(1),
                Duration::from_secs(600),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("work");
        let result = installer
            .run(
                work,
                &lettuce_network::ArtifactDownloadClient::new().expect("client"),
                CancellationReason::User,
                TimestampMillis::new(2),
            )
            .await
            .expect("install");
        assert!(
            matches!(result, crate::ArtifactInstallRunResult::Succeeded { .. }),
            "{result:?}"
        );

        let engine = LocalDiffusionEngine::new(
            paths.clone(),
            lettuce_network::BulkHttpClient::new().expect("client"),
            Arc::new(EngineDevices(Vec::new())),
            Arc::new(NoProgress),
        );
        let inventory = engine.upscaler_inventory();
        assert!(inventory.recommended_installed);
        assert_eq!(
            inventory.hires_upscaler_names,
            ["RealESRGAN_x4plus_anime_6B"]
        );
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbImage::from_pixel(8, 8, image::Rgb([200, 120, 40]))
            .write_to(&mut png, image::ImageFormat::Png)
            .expect("png");
        let png = png.into_inner();
        let upscaled = engine.upscale(&png).await.expect("upscale");
        assert_eq!(&upscaled[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(
            u32::from_be_bytes(upscaled[16..20].try_into().expect("width")),
            32
        );
        std::fs::remove_dir_all(root).ok();
    }
}
