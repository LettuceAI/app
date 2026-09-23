//! Local image models assembled from Hugging Face files: finding a
//! repository and file for each role of a catalog architecture, downloading
//! them as one pinned install, and registering the model once they are in.
//! Also the image model files on disk and the component library that
//! offers them for reuse.

use std::collections::{BTreeSet, HashMap};
use std::path::{Component, Path, PathBuf};

use futures_util::StreamExt;
use lettuce_image_generation::sd_runtime::layout::DiffusionPaths;
use lettuce_image_generation::sd_runtime::runnability::{
    RemoteBundleRunnabilityRequest, RunnabilityStatus,
};
use lettuce_image_generation::sd_runtime::server::LocalDiffusionEngine;
use lettuce_image_generation::{
    BundleAsset, BundleManifest, BundleRegistrationState, DiffusionComponentRole, DiffusionProfile,
    ManifestAsset, diffusion_catalog,
};
use lettuce_model_hub::{HfBrowseError, HfRepoDetail, HfSearchResult, HfTreeEntry, PinnedArtifact};
use lettuce_models::{ModelLookup, ModelProfileRepository, ProviderAccountRepository};
use lettuce_settings::SecretStore;
use lettuce_types::TimestampMillis;

use crate::{
    ArtifactInstallPlan, ArtifactSource, GgufHeaderSource, HfBundleRegistration,
    HuggingFaceBrowser, PlannedArtifact, register_hf_bundle_model,
};

const HINT_CONCURRENCY: usize = 6;

static MANIFEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn message(text: impl Into<String>) -> HfBrowseError {
    HfBrowseError::Message(text.into())
}

fn role_name(role: DiffusionComponentRole) -> &'static str {
    match role {
        DiffusionComponentRole::DiffusionModel => "diffusion_model",
        DiffusionComponentRole::TextEncoder => "text_encoder",
        DiffusionComponentRole::Vae => "vae",
        DiffusionComponentRole::VisionEncoder => "vision_encoder",
    }
}

fn bundle_profile(profile_id: &str) -> Result<&'static DiffusionProfile, HfBrowseError> {
    diffusion_catalog()
        .profile(profile_id)
        .map_err(|_| message(format!("Unknown local image architecture: {profile_id}")))
}

fn used_role(
    profile: &DiffusionProfile,
    role: DiffusionComponentRole,
) -> Result<(), HfBrowseError> {
    if profile.required_roles().contains(&role) {
        Ok(())
    } else {
        Err(message(format!(
            "{} is not used by the selected architecture.",
            role_name(role)
        )))
    }
}

fn valid_revision(sha: Option<&str>) -> Option<&str> {
    sha.filter(|sha| sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

/// A search for one role of an architecture's bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HfBundleRoleSearch {
    pub profile_id: String,
    pub role: DiffusionComponentRole,
    pub query: String,
    pub sort: String,
    pub author: Option<String>,
    /// `safetensors`, else GGUF.
    pub format: Option<String>,
}

/// A bundle to download: one checked asset per role of the architecture.
#[derive(Debug, Clone, PartialEq)]
pub struct HfBundleInstallRequest {
    pub profile_id: String,
    pub display_name: String,
    pub runtime_release: String,
    pub runtime_asset: String,
    pub assets: Vec<BundleAsset>,
}

impl HuggingFaceBrowser {
    async fn bundle_detail<S: SecretStore + ?Sized>(
        &self,
        secrets: &S,
        model_id: &str,
    ) -> Result<HfRepoDetail, HfBrowseError> {
        let token = Self::saved_token(secrets).await?;
        let response = self
            .get(
                &lettuce_model_hub::model_detail_request(model_id),
                token.as_ref(),
            )
            .await
            .map_err(|error| {
                message(format!(
                    "Failed to fetch Hugging Face model metadata: {error}"
                ))
            })?;
        match response.status {
            401 => Err(message(
                "Hugging Face authentication is missing or invalid.",
            )),
            403 => Err(message(
                "This gated model requires accepting its license on Hugging Face before retrying.",
            )),
            status if !(200..300).contains(&status) => Err(message(format!(
                "Hugging Face model lookup failed with status {}.",
                lettuce_network::status_text(status)
            ))),
            _ => lettuce_model_hub::parse_repo_detail(&response.body).map_err(|error| {
                message(format!(
                    "Failed to parse Hugging Face model metadata: {error}"
                ))
            }),
        }
    }

    async fn bundle_tree<S: SecretStore + ?Sized>(
        &self,
        secrets: &S,
        model_id: &str,
        revision: &str,
    ) -> Result<Vec<HfTreeEntry>, HfBrowseError> {
        let token = Self::saved_token(secrets).await?;
        let response = self
            .get(
                &lettuce_model_hub::repo_tree_request(model_id, revision),
                token.as_ref(),
            )
            .await
            .map_err(|error| {
                message(format!(
                    "Failed to fetch the recursive repository tree: {error}"
                ))
            })?;
        if response.status == 403 {
            return Err(message(
                "Accept this model's license on Hugging Face, then retry.",
            ));
        }
        if !(200..300).contains(&response.status) {
            return Err(message(format!(
                "Repository tree lookup failed with status {}.",
                lettuce_network::status_text(response.status)
            )));
        }
        lettuce_model_hub::parse_repo_tree(&response.body)
            .map_err(|error| message(format!("Failed to parse the repository tree: {error}")))
    }

    async fn bundle_list<S: SecretStore + ?Sized>(
        &self,
        secrets: &S,
        request: &lettuce_model_hub::HfRequest,
    ) -> Result<Vec<HfSearchResult>, HfBrowseError> {
        let token = Self::saved_token(secrets).await?;
        let response = self
            .get(request, token.as_ref())
            .await
            .map_err(|error| message(format!("HuggingFace API request failed: {error}")))?;
        if let Some(error) = lettuce_model_hub::access_error(
            response.status,
            lettuce_model_hub::HfResource::List,
            "",
            token.is_some(),
        ) {
            return Err(error);
        }
        if !(200..300).contains(&response.status) {
            return Err(message(format!(
                "HuggingFace API error {}: {}",
                lettuce_network::status_text(response.status),
                String::from_utf8_lossy(&response.body)
            )));
        }
        lettuce_model_hub::parse_model_list(&response.body)
    }

    /// Repositories for one role of an architecture: a pasted repository
    /// directly, else the top matches for the query (or the role's default
    /// query) in the chosen file format.
    pub async fn bundle_role_search<S: SecretStore + ?Sized>(
        &self,
        secrets: &S,
        search: &HfBundleRoleSearch,
    ) -> Result<Vec<HfSearchResult>, HfBrowseError> {
        let HfBundleRoleSearch {
            profile_id,
            role,
            query,
            sort,
            author,
            format,
        } = search;
        let (role, author, format) = (*role, author.as_deref(), format.as_deref());
        let profile = bundle_profile(profile_id)?;
        used_role(profile, role)?;
        let query = query.as_str();
        if query.contains('/')
            && !query.contains(char::is_whitespace)
            && let Some(model_id) = lettuce_image_generation::parse_bundle_model_id(query)
        {
            let detail = self.bundle_detail(secrets, &model_id).await?;
            if role == DiffusionComponentRole::DiffusionModel {
                let (compatible, reason) = lettuce_image_generation::repo_compatibility(
                    profile,
                    &detail.model_id,
                    &detail.tags,
                    &detail.card_data,
                );
                if !compatible {
                    return Err(message(reason));
                }
            }
            return Ok(vec![detail.search_result()]);
        }
        let query = match query.trim() {
            "" => lettuce_image_generation::role_default_query(profile, role),
            query => query.to_owned(),
        };
        let format_filter = if role == DiffusionComponentRole::Vae {
            "safetensors"
        } else {
            lettuce_image_generation::normalized_bundle_format(format)
        };
        let mut results = self
            .bundle_list(
                secrets,
                &lettuce_model_hub::bundle_search_request(
                    sort,
                    author,
                    &query,
                    Some(format_filter),
                ),
            )
            .await?;
        if results.is_empty() && format_filter == "safetensors" {
            results = self
                .bundle_list(
                    secrets,
                    &lettuce_model_hub::bundle_search_request(sort, author, &query, None),
                )
                .await?;
        }
        let (matched, unmatched): (Vec<_>, Vec<_>) = results.into_iter().partition(|result| {
            lettuce_image_generation::role_search_matches(
                profile,
                role,
                &result.model_id,
                &result.tags,
            )
        });
        if role == DiffusionComponentRole::DiffusionModel || !matched.is_empty() {
            Ok(matched)
        } else {
            Ok(unmatched)
        }
    }

    /// The files of a repository that can fill `role`, pinned to its current
    /// revision, smallest first. GGUF text encoders are checked against the
    /// architecture's encoder by their header.
    pub async fn bundle_files<S, H>(
        &self,
        secrets: &S,
        headers: &H,
        profile_id: &str,
        model_id: &str,
        role: DiffusionComponentRole,
    ) -> Result<Vec<BundleAsset>, HfBrowseError>
    where
        S: SecretStore + ?Sized,
        H: GgufHeaderSource + ?Sized,
    {
        let profile = bundle_profile(profile_id)?;
        used_role(profile, role)?;
        let model_id = lettuce_image_generation::parse_bundle_model_id(model_id)
            .ok_or_else(|| message("Enter a valid Hugging Face model ID or URL."))?;
        let detail = self.bundle_detail(secrets, &model_id).await?;
        let revision = valid_revision(detail.sha.as_deref())
            .ok_or_else(|| message("Hugging Face did not return an immutable commit revision."))?
            .to_owned();
        if role == DiffusionComponentRole::DiffusionModel
            && !lettuce_image_generation::repo_compatibility(
                profile,
                &detail.model_id,
                &detail.tags,
                &detail.card_data,
            )
            .0
        {
            return Err(message(
                "The selected diffusion repository is not compatible with this architecture recipe.",
            ));
        }
        let tree = self.bundle_tree(secrets, &model_id, &revision).await?;
        let prevalidated = tree
            .into_iter()
            .filter(|entry| {
                entry.is_file
                    && lettuce_image_generation::safe_relative_path(&entry.path)
                    && !lettuce_image_generation::excluded_artifact(&entry.path)
            })
            .filter_map(|entry| {
                let format = lettuce_image_generation::bundle_file_format(&entry.path)?;
                if role != DiffusionComponentRole::DiffusionModel
                    && !lettuce_image_generation::role_compatible(
                        profile,
                        role,
                        &model_id,
                        &entry.path,
                    )
                {
                    return None;
                }
                let sha256 = entry.sha256.clone().filter(|oid| {
                    oid.len() == 64 && oid.bytes().all(|byte| byte.is_ascii_hexdigit())
                })?;
                (entry.size != 0).then_some((entry.path, format, entry.size, sha256))
            })
            .collect::<Vec<_>>();
        let token = Self::saved_token(secrets).await.ok().flatten();
        let architectures = if role == DiffusionComponentRole::TextEncoder {
            futures_util::stream::iter(prevalidated.iter().map(|(path, format, _, _)| {
                let token = token.as_ref();
                let (model_id, revision) = (model_id.as_str(), revision.as_str());
                async move {
                    if *format != "gguf" {
                        return None;
                    }
                    let bytes = headers
                        .read_prefix(
                            model_id,
                            revision,
                            path,
                            lettuce_image_generation::ENCODER_HINT_BYTES,
                            token,
                        )
                        .await?;
                    lettuce_image_generation::gguf_architecture_hint(&bytes)
                }
            }))
            .buffered(HINT_CONCURRENCY)
            .collect::<Vec<_>>()
            .await
        } else {
            vec![None; prevalidated.len()]
        };
        let mut candidates = prevalidated
            .into_iter()
            .zip(architectures)
            .filter(|(_, architecture)| {
                role != DiffusionComponentRole::TextEncoder
                    || architecture.as_ref().is_none_or(|architecture| {
                        profile
                            .encoder_markers
                            .iter()
                            .any(|marker| architecture.contains(marker.as_str()))
                    })
            })
            .map(|((path, format, size, sha256), architecture)| BundleAsset {
                selection_id: uuid::Uuid::new_v4().to_string(),
                profile_id: profile_id.to_owned(),
                role,
                model_id: model_id.clone(),
                revision: revision.clone(),
                quantization: lettuce_image_generation::bundle_quantization(&path),
                relative_path: path,
                format: format.to_owned(),
                size,
                sha256,
                architecture,
                gated: detail.gated,
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|asset| asset.size);
        Ok(candidates)
    }

    async fn refetch_bundle_asset<S: SecretStore + ?Sized>(
        &self,
        secrets: &S,
        asset: &BundleAsset,
    ) -> Result<(), HfBrowseError> {
        let detail = self.bundle_detail(secrets, &asset.model_id).await?;
        if detail.sha.as_deref() != Some(asset.revision.as_str()) {
            return Err(message(
                "The repository revision changed after selection. Select the component again.",
            ));
        }
        let tree = self
            .bundle_tree(secrets, &asset.model_id, &asset.revision)
            .await?;
        let entry = tree
            .into_iter()
            .find(|entry| entry.is_file && entry.path == asset.relative_path)
            .ok_or_else(|| {
                message("A selected component no longer exists at the pinned revision.")
            })?;
        if entry.size != asset.size || entry.sha256.as_deref().unwrap_or_default() != asset.sha256 {
            return Err(message(
                "A selected component no longer matches its validated size and hash.",
            ));
        }
        Ok(())
    }

    /// Checks a selected bundle against the engine and Hugging Face again,
    /// writes its manifest and returns the install that downloads it.
    pub async fn bundle_install<S: SecretStore + ?Sized>(
        &self,
        secrets: &S,
        engine: &LocalDiffusionEngine,
        request: &HfBundleInstallRequest,
    ) -> Result<(BundleManifest, ArtifactInstallPlan), HfBrowseError> {
        let profile = bundle_profile(&request.profile_id)?;
        let display_name = request.display_name.trim();
        if display_name.is_empty() {
            return Err(message("Enter a display name for the model."));
        }
        if request.assets.is_empty() {
            return Err(message(
                "Select every required bundle component before downloading.",
            ));
        }
        if request
            .assets
            .iter()
            .any(|asset| asset.profile_id != request.profile_id)
        {
            return Err(message(
                "A component selection expired. Select it again before downloading.",
            ));
        }
        let roles = request
            .assets
            .iter()
            .map(|asset| asset.role)
            .collect::<BTreeSet<_>>();
        for role in profile.required_roles() {
            if !roles.contains(&role) {
                return Err(message(format!(
                    "The required {} component is missing.",
                    role_name(role)
                )));
            }
        }
        if roles.len() != request.assets.len() {
            return Err(message(
                "Select exactly one component for each bundle role.",
            ));
        }
        let size_for = |role| {
            request
                .assets
                .iter()
                .find(|asset| asset.role == role)
                .map_or(0, |asset| asset.size)
        };
        let estimate = engine
            .remote_bundle_runnability(&RemoteBundleRunnabilityRequest {
                profile_id: request.profile_id.clone(),
                runtime_release: request.runtime_release.clone(),
                runtime_asset: request.runtime_asset.clone(),
                diffusion_bytes: size_for(DiffusionComponentRole::DiffusionModel),
                text_encoder_bytes: size_for(DiffusionComponentRole::TextEncoder),
                vae_bytes: size_for(DiffusionComponentRole::Vae),
                vision_encoder_bytes: size_for(DiffusionComponentRole::VisionEncoder),
            })
            .await
            .map_err(message)?;
        if matches!(
            estimate.status,
            RunnabilityStatus::NotInstalled | RunnabilityStatus::IncompatibleRuntime
        ) {
            return Err(message(
                "Install or select a compatible stable-diffusion.cpp engine before downloading this bundle.",
            ));
        }
        for asset in &request.assets {
            if asset.role != DiffusionComponentRole::DiffusionModel
                && !lettuce_image_generation::role_compatible(
                    profile,
                    asset.role,
                    &asset.model_id,
                    &asset.relative_path,
                )
            {
                return Err(message(format!(
                    "The selected {} is not compatible with {}.",
                    role_name(asset.role),
                    profile.display_name
                )));
            }
            self.refetch_bundle_asset(secrets, asset).await?;
        }
        let image_root = &engine.paths().image_root;
        let root = lettuce_image_generation::bundle_root(image_root);
        let bundle_id = uuid::Uuid::new_v4().to_string();
        let mut assets = Vec::with_capacity(request.assets.len());
        let mut artifacts = Vec::with_capacity(request.assets.len());
        for asset in &request.assets {
            let segments = asset.local_segments().map_err(message)?;
            let local_path = segments
                .iter()
                .fold(root.clone(), |path, segment| path.join(segment));
            let source = ArtifactSource::HuggingFace {
                repository: asset.model_id.clone(),
                revision: asset.revision.clone(),
                path: asset.relative_path.clone(),
            };
            artifacts.push(PlannedArtifact {
                artifact: PinnedArtifact {
                    source_identity: source.identity(),
                    local_segments: segments,
                    byte_size: asset.size,
                    sha256: Some(asset.sha256.to_ascii_lowercase()),
                },
                source,
            });
            assets.push(ManifestAsset {
                asset: asset.clone(),
                local_path: local_path.to_string_lossy().into_owned(),
                verified: false,
            });
        }
        let manifest = BundleManifest {
            bundle_id: bundle_id.clone(),
            profile_id: request.profile_id.clone(),
            display_name: display_name.to_owned(),
            runtime_release: request.runtime_release.clone(),
            runtime_asset: request.runtime_asset.clone(),
            runnability: Some(serde_json::to_value(&estimate).map_err(|error| {
                message(format!("Failed to save the runnability estimate: {error}"))
            })?),
            assets,
            registration_state: BundleRegistrationState::Downloading,
            model_id: None,
            setup_error: None,
        };
        for asset in &manifest.assets {
            lettuce_image_generation::reuse_verified_asset(image_root, asset);
        }
        {
            let _guard = MANIFEST_LOCK.lock().await;
            lettuce_image_generation::write_bundle_manifest(image_root, &manifest)
                .map_err(message)?;
        }
        Ok((
            manifest,
            ArtifactInstallPlan {
                install_id: format!("hf-bundle:{bundle_id}"),
                root,
                artifacts,
            },
        ))
    }

    /// The install for a bundle's missing or unverified files, checked
    /// against Hugging Face again; `None` when every file is already in.
    pub async fn bundle_retry_downloads<S: SecretStore + ?Sized>(
        &self,
        secrets: &S,
        image_root: &Path,
        bundle_id: &str,
    ) -> Result<Option<ArtifactInstallPlan>, HfBrowseError> {
        let _guard = MANIFEST_LOCK.lock().await;
        let mut manifest = lettuce_image_generation::read_bundle_manifest(image_root, bundle_id)
            .map_err(message)?;
        let root = lettuce_image_generation::bundle_root(image_root);
        let mut artifacts = Vec::new();
        for asset in &manifest.assets {
            if asset.verified && Path::new(&asset.local_path).is_file() {
                continue;
            }
            self.refetch_bundle_asset(secrets, &asset.asset).await?;
            let source = ArtifactSource::HuggingFace {
                repository: asset.asset.model_id.clone(),
                revision: asset.asset.revision.clone(),
                path: asset.asset.relative_path.clone(),
            };
            artifacts.push(PlannedArtifact {
                artifact: PinnedArtifact {
                    source_identity: source.identity(),
                    local_segments: asset.asset.local_segments().map_err(message)?,
                    byte_size: asset.asset.size,
                    sha256: Some(asset.asset.sha256.to_ascii_lowercase()),
                },
                source,
            });
        }
        if artifacts.is_empty() {
            return Ok(None);
        }
        manifest.registration_state = BundleRegistrationState::Downloading;
        manifest.setup_error = None;
        lettuce_image_generation::write_bundle_manifest(image_root, &manifest).map_err(message)?;
        Ok(Some(ArtifactInstallPlan {
            install_id: format!("hf-bundle:{bundle_id}"),
            root,
            artifacts,
        }))
    }
}

fn register_manifest<R>(
    repository: &R,
    paths: &DiffusionPaths,
    manifest: &BundleManifest,
    now: TimestampMillis,
) -> Result<String, String>
where
    R: ModelLookup + ModelProfileRepository + ProviderAccountRepository + ?Sized,
{
    let by_role = manifest
        .assets
        .iter()
        .map(|asset| (asset.asset.role, asset.local_path.as_str()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let role = |role| by_role.get(&role).copied();
    register_hf_bundle_model(
        repository,
        paths,
        HfBundleRegistration {
            profile_id: &manifest.profile_id,
            display_name: &manifest.display_name,
            diffusion_path: role(DiffusionComponentRole::DiffusionModel)
                .ok_or_else(|| "The diffusion model is missing.".to_owned())?,
            text_encoder_path: role(DiffusionComponentRole::TextEncoder)
                .ok_or_else(|| "The text encoder is missing.".to_owned())?,
            vae_path: role(DiffusionComponentRole::Vae)
                .ok_or_else(|| "The VAE is missing.".to_owned())?,
            vision_encoder_path: role(DiffusionComponentRole::VisionEncoder),
            runtime_release: &manifest.runtime_release,
            runtime_asset: &manifest.runtime_asset,
        },
        now,
    )
    .map(|profile| profile.id.to_string())
    .map_err(|error| error.to_string())
}

fn register_into(
    manifest: &mut BundleManifest,
    registered: Result<String, String>,
) -> Result<String, String> {
    match registered {
        Ok(model_id) => {
            manifest.registration_state = BundleRegistrationState::Registered;
            manifest.model_id = Some(model_id.clone());
            manifest.setup_error = None;
            Ok(model_id)
        }
        Err(error) => {
            manifest.registration_state = BundleRegistrationState::SetupFailed;
            manifest.setup_error = Some(error.clone());
            Err(error)
        }
    }
}

/// Marks the bundle's downloaded files verified (the install checked their
/// After `completed` installed and verified its files: marks the bundle's
/// files among them verified and registers the model once every file is in.
pub async fn finish_hf_bundle<R>(
    repository: &R,
    paths: &DiffusionPaths,
    bundle_id: &str,
    completed: &ArtifactInstallPlan,
    now: TimestampMillis,
) -> Result<BundleManifest, String>
where
    R: ModelLookup + ModelProfileRepository + ProviderAccountRepository + ?Sized,
{
    let _guard = MANIFEST_LOCK.lock().await;
    let mut manifest =
        lettuce_image_generation::read_bundle_manifest(&paths.image_root, bundle_id)?;
    let installed = completed
        .artifacts
        .iter()
        .map(|planned| {
            planned
                .artifact
                .local_segments
                .iter()
                .fold(completed.root.clone(), |path, segment| path.join(segment))
                .to_string_lossy()
                .into_owned()
        })
        .collect::<BTreeSet<_>>();
    for asset in &mut manifest.assets {
        if installed.contains(&asset.local_path) {
            asset.verified = true;
        }
    }
    if manifest.complete() {
        let registered = register_manifest(repository, paths, &manifest, now);
        let _ = register_into(&mut manifest, registered);
    }
    lettuce_image_generation::write_bundle_manifest(&paths.image_root, &manifest)?;
    Ok(manifest)
}

/// Registers a fully downloaded bundle's model again after a failure.
pub async fn retry_hf_bundle_registration<R>(
    repository: &R,
    paths: &DiffusionPaths,
    bundle_id: &str,
    now: TimestampMillis,
) -> Result<String, String>
where
    R: ModelLookup + ModelProfileRepository + ProviderAccountRepository + ?Sized,
{
    let _guard = MANIFEST_LOCK.lock().await;
    let mut manifest =
        lettuce_image_generation::read_bundle_manifest(&paths.image_root, bundle_id)?;
    if !manifest.complete() {
        return Err(
            "Every bundle component must be verified before model creation can be retried."
                .to_owned(),
        );
    }
    let registered = register_manifest(repository, paths, &manifest, now);
    let result = register_into(&mut manifest, registered);
    lettuce_image_generation::write_bundle_manifest(&paths.image_root, &manifest)?;
    result
}

/// An image model file on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedImageModel {
    pub model_id: String,
    pub filename: String,
    pub path: String,
    pub size: u64,
    pub quantization: String,
    pub is_mmproj: bool,
    pub architecture: Option<String>,
    pub role: DiffusionComponentRole,
}

fn image_roots(paths: &DiffusionPaths) -> Vec<PathBuf> {
    std::iter::once(paths.image_root.clone())
        .chain(paths.default_image_root.clone())
        .collect()
}

/// Every GGUF and safetensors file below the image model folders, with the
/// role its bundle manifest, the catalog or its name gives it.
#[must_use]
pub fn downloaded_image_models(paths: &DiffusionPaths) -> Vec<DownloadedImageModel> {
    let roots = image_roots(paths);
    let role_by_path = roots
        .iter()
        .flat_map(|root| lettuce_image_generation::bundle_manifests(root))
        .flat_map(|manifest| manifest.assets)
        .map(|asset| (asset.local_path, asset.asset.role))
        .collect::<HashMap<_, _>>();
    let mut results = Vec::new();
    for root in roots {
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let filename = entry.file_name().to_string_lossy().into_owned();
                let lower = filename.to_lowercase();
                if !(lower.ends_with(".gguf")
                    || lower.ends_with(".safetensors")
                    || lower.ends_with(".sft"))
                {
                    continue;
                }
                let segments = path
                    .strip_prefix(&root)
                    .unwrap_or(&path)
                    .components()
                    .filter_map(|component| match component {
                        Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let model_id = if segments.first().map(String::as_str) == Some("huggingface") {
                    if segments.get(1).map(String::as_str) == Some("manual") {
                        segments.get(2).map_or_else(
                            || "huggingface".to_owned(),
                            |part| part.replace("--", "/"),
                        )
                    } else if segments.len() >= 3 {
                        format!("{}/{}", segments[1], segments[2])
                    } else {
                        "huggingface".to_owned()
                    }
                } else {
                    segments.first().cloned().unwrap_or_default()
                };
                let path_text = path.to_string_lossy().into_owned();
                let role = role_by_path
                    .get(&path_text)
                    .copied()
                    .or_else(|| {
                        (segments.first().map(String::as_str) == Some("components"))
                            .then(|| segments.get(1))
                            .flatten()
                            .and_then(|sha| diffusion_catalog().component_role(sha))
                    })
                    .unwrap_or_else(|| lettuce_image_generation::infer_image_role(&lower));
                let architecture = lower
                    .ends_with(".gguf")
                    .then(|| crate::model_runnability::local_gguf_meta(&path))
                    .flatten()
                    .and_then(|meta| meta.architecture);
                results.push(DownloadedImageModel {
                    model_id,
                    size: entry.metadata().map_or(0, |metadata| metadata.len()),
                    quantization: lettuce_model_hub::extract_quantization(&path_text),
                    is_mmproj: lower.contains("mmproj"),
                    architecture,
                    role,
                    filename,
                    path: path_text,
                });
            }
        }
    }
    results.sort_by(|left, right| {
        left.model_id
            .cmp(&right.model_id)
            .then(left.filename.cmp(&right.filename))
    });
    results
}

/// Where a component library file comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentSource {
    ImageComponents,
    LlmLibrary,
    ImageDownloads,
}

/// A file a bundle role can reuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentLibraryEntry {
    pub path: String,
    pub filename: String,
    pub bytes: u64,
    pub role: Option<DiffusionComponentRole>,
    pub source: ComponentSource,
}

/// Encoders and VAEs already on disk: catalog components, the GGUF library
/// (projectors as vision encoders, MTP drafts left out) and other image
/// downloads, by file name.
pub fn component_library(
    paths: &DiffusionPaths,
    llm_root: &Path,
) -> Result<Vec<ComponentLibraryEntry>, String> {
    if cfg!(any(target_os = "android", target_os = "ios")) {
        return Err("Local stable-diffusion.cpp image generation is desktop-only.".to_owned());
    }
    let mut entries = Vec::new();
    if let Ok(directories) = std::fs::read_dir(paths.image_root.join("components")) {
        for directory in directories.flatten() {
            let directory_path = directory.path();
            if !directory_path.is_dir() {
                continue;
            }
            let sha = directory.file_name().to_string_lossy().into_owned();
            let role = diffusion_catalog().component_role(&sha);
            if role == Some(DiffusionComponentRole::DiffusionModel) {
                continue;
            }
            let Ok(files) = std::fs::read_dir(&directory_path) else {
                continue;
            };
            for file in files.flatten() {
                let path = file.path();
                if !path.is_file() {
                    continue;
                }
                let Some(filename) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                if filename.starts_with('.') || filename.ends_with(".tmp") {
                    continue;
                }
                entries.push(ComponentLibraryEntry {
                    filename: filename.to_owned(),
                    path: path.to_string_lossy().into_owned(),
                    bytes: file.metadata().map_or(0, |metadata| metadata.len()),
                    role,
                    source: ComponentSource::ImageComponents,
                });
            }
        }
    }
    if let Ok(models) = crate::downloaded_ggufs(llm_root) {
        entries.extend(
            models
                .into_iter()
                .filter(|model| !model.is_mtp)
                .map(|model| ComponentLibraryEntry {
                    role: Some(if model.is_mmproj {
                        DiffusionComponentRole::VisionEncoder
                    } else {
                        DiffusionComponentRole::TextEncoder
                    }),
                    path: model.path,
                    filename: model.filename,
                    bytes: model.size,
                    source: ComponentSource::LlmLibrary,
                }),
        );
    }
    let known = entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<BTreeSet<_>>();
    entries.extend(
        downloaded_image_models(paths)
            .into_iter()
            .filter(|file| {
                file.role != DiffusionComponentRole::DiffusionModel && !known.contains(&file.path)
            })
            .map(|file| ComponentLibraryEntry {
                path: file.path,
                filename: file.filename,
                bytes: file.size,
                role: Some(file.role),
                source: ComponentSource::ImageDownloads,
            }),
    );
    entries.sort_by_key(|entry| entry.filename.to_lowercase());
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use lettuce_image_generation::sd_runtime::layout::server_executable_name;
    use lettuce_models::ModelProfileRepository;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    const RELEASE: &str = "master-778-b";
    const ASSET: &str = "sd-master-bin-Linux-Ubuntu-24.04-x86_64.zip";

    fn scratch() -> (PathBuf, DiffusionPaths) {
        let root =
            std::env::temp_dir().join(format!("hf-bundles-{}", lettuce_types::OperationId::new()));
        let paths = DiffusionPaths::legacy_layout(&root, root.join("models").join("image"));
        let build = paths.runtime_root(RELEASE, ASSET);
        std::fs::create_dir_all(&build).expect("build");
        std::fs::write(build.join(server_executable_name()), b"server").expect("server");
        (root, paths)
    }

    fn asset(role: DiffusionComponentRole, path: &str, size: u64) -> BundleAsset {
        BundleAsset {
            selection_id: path.to_owned(),
            profile_id: "z-image-turbo".to_owned(),
            role,
            model_id: "org/repo".to_owned(),
            revision: "d24c4cf2a0cd98a42f23467e27e3d76ee9438b8e".to_owned(),
            relative_path: path.to_owned(),
            format: "gguf".to_owned(),
            quantization: None,
            size,
            sha256: "a".repeat(64),
            architecture: None,
            gated: false,
        }
    }

    fn manifest(paths: &DiffusionPaths) -> BundleManifest {
        let root = lettuce_image_generation::bundle_root(&paths.image_root);
        let assets = [
            asset(
                DiffusionComponentRole::DiffusionModel,
                "z-image-turbo-Q4_K.gguf",
                4,
            ),
            asset(
                DiffusionComponentRole::TextEncoder,
                "Qwen3-4B-Q4_K_M.gguf",
                3,
            ),
            asset(DiffusionComponentRole::Vae, "ae.safetensors", 2),
        ]
        .into_iter()
        .map(|asset| ManifestAsset {
            local_path: asset
                .local_segments()
                .expect("segments")
                .iter()
                .fold(root.clone(), |path, segment| path.join(segment))
                .to_string_lossy()
                .into_owned(),
            asset,
            verified: false,
        })
        .collect();
        BundleManifest {
            bundle_id: "bundle-1".to_owned(),
            profile_id: "z-image-turbo".to_owned(),
            display_name: " Custom Z ".to_owned(),
            runtime_release: RELEASE.to_owned(),
            runtime_asset: ASSET.to_owned(),
            runnability: None,
            assets,
            registration_state: BundleRegistrationState::Downloading,
            model_id: None,
            setup_error: None,
        }
    }

    fn completed(paths: &DiffusionPaths, assets: &[ManifestAsset]) -> ArtifactInstallPlan {
        ArtifactInstallPlan {
            install_id: "hf-bundle:test".to_owned(),
            root: lettuce_image_generation::bundle_root(&paths.image_root),
            artifacts: assets
                .iter()
                .map(|asset| {
                    let source = ArtifactSource::HuggingFace {
                        repository: asset.asset.model_id.clone(),
                        revision: asset.asset.revision.clone(),
                        path: asset.asset.relative_path.clone(),
                    };
                    PlannedArtifact {
                        artifact: PinnedArtifact {
                            source_identity: source.identity(),
                            local_segments: asset.asset.local_segments().expect("segments"),
                            byte_size: asset.asset.size,
                            sha256: Some(asset.asset.sha256.clone()),
                        },
                        source,
                    }
                })
                .collect(),
        }
    }

    #[tokio::test]
    async fn a_bundle_registers_its_model_once_every_file_is_in() {
        let (root, paths) = scratch();
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let database = backend.database();
        let manifest = manifest(&paths);
        lettuce_image_generation::write_bundle_manifest(&paths.image_root, &manifest)
            .expect("write");
        for asset in &manifest.assets[..2] {
            let path = Path::new(&asset.local_path);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
            std::fs::write(
                path,
                vec![0_u8; usize::try_from(asset.asset.size).expect("size")],
            )
            .expect("file");
        }
        let partial = finish_hf_bundle(
            database,
            &paths,
            "bundle-1",
            &completed(&paths, &manifest.assets[..2]),
            TimestampMillis::new(2),
        )
        .await
        .expect("partial");
        assert_eq!(
            partial.registration_state,
            BundleRegistrationState::Downloading
        );
        assert_eq!(
            retry_hf_bundle_registration(database, &paths, "bundle-1", TimestampMillis::new(2))
                .await,
            Err(
                "Every bundle component must be verified before model creation can be retried."
                    .to_owned()
            )
        );
        let vae = &manifest.assets[2];
        std::fs::write(&vae.local_path, [0_u8; 2]).expect("vae");
        let done = finish_hf_bundle(
            database,
            &paths,
            "bundle-1",
            &completed(&paths, &manifest.assets[2..]),
            TimestampMillis::new(3),
        )
        .await
        .expect("done");
        assert_eq!(done.registration_state, BundleRegistrationState::Registered);
        let model_id = done.model_id.expect("model").parse().expect("id");
        let mut model = ModelProfileRepository::get(database, model_id)
            .expect("get")
            .expect("model");
        assert_eq!(model.display_name, "Custom Z");
        assert_eq!(model.external_model_id, manifest.assets[0].local_path);
        let binding = &model.config.stable_diffusion.cpp;
        assert_eq!(binding.vae_path.as_deref(), Some(vae.local_path.as_str()));
        assert_eq!(binding.profile_id.as_deref(), Some("z-image-turbo"));
        assert!(model.config.stable_diffusion.size.is_some());
        model.display_name = "Renamed".to_owned();
        model.config.stable_diffusion.steps = Some(3);
        model.config.stable_diffusion.cpp.vae_path = Some("/kept/vae".to_owned());
        let revision = model.revision;
        ModelProfileRepository::upsert(database, model, Some(revision)).expect("edit");
        let again =
            retry_hf_bundle_registration(database, &paths, "bundle-1", TimestampMillis::new(4))
                .await
                .expect("again");
        let kept = ModelProfileRepository::get(database, again.parse().expect("id"))
            .expect("get")
            .expect("model");
        assert_eq!(kept.display_name, "Renamed");
        assert_eq!(kept.config.stable_diffusion.steps, Some(3));
        assert_eq!(
            kept.config.stable_diffusion.cpp.vae_path.as_deref(),
            Some("/kept/vae")
        );
        let listed = downloaded_image_models(&paths);
        assert_eq!(listed.len(), 3);
        assert!(listed.iter().all(|file| file.model_id == "org/repo"));
        let library = component_library(&paths, &root.join("gguf")).expect("library");
        assert_eq!(
            library
                .iter()
                .map(|entry| (entry.filename.as_str(), entry.role))
                .collect::<Vec<_>>(),
            [
                ("ae.safetensors", Some(DiffusionComponentRole::Vae)),
                (
                    "Qwen3-4B-Q4_K_M.gguf",
                    Some(DiffusionComponentRole::TextEncoder)
                ),
            ]
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_bundle_model_needs_a_name_and_an_installed_engine() {
        let (root, paths) = scratch();
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let registration = HfBundleRegistration {
            profile_id: "z-image-turbo",
            display_name: " ",
            diffusion_path: "/d.gguf",
            text_encoder_path: "/t.gguf",
            vae_path: "/v.safetensors",
            vision_encoder_path: None,
            runtime_release: RELEASE,
            runtime_asset: ASSET,
        };
        let error = |registration| {
            register_hf_bundle_model(
                backend.database(),
                &paths,
                registration,
                TimestampMillis::new(2),
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
        };
        assert_eq!(
            error(registration),
            Err("The model display name cannot be empty.".to_owned())
        );
        assert_eq!(
            error(HfBundleRegistration {
                display_name: "Z",
                runtime_asset: "other.zip",
                ..registration
            }),
            Err("The selected stable-diffusion.cpp engine is no longer installed.".to_owned())
        );
        assert_eq!(
            error(HfBundleRegistration {
                profile_id: "nope",
                ..registration
            }),
            Err("Unknown local image architecture: nope".to_owned())
        );
        let _ = std::fs::remove_dir_all(root);
    }

    struct NoHeaders;

    #[async_trait::async_trait]
    impl GgufHeaderSource for NoHeaders {
        async fn read_prefix(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: u64,
            _: Option<&lettuce_settings::SecretValue>,
        ) -> Option<Vec<u8>> {
            Some(b"GGUF qwen3-8b".to_vec())
        }
    }

    #[tokio::test]
    async fn a_repository_offers_only_pinned_files_that_fit_the_role() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("address");
        let sha = "d24c4cf2a0cd98a42f23467e27e3d76ee9438b8e";
        let oid = "b".repeat(64);
        let responses = vec![
            format!(
                r#"{{"modelId": "unsloth/Qwen3-4B-GGUF", "sha": "{sha}", "gated": "auto", "tags": []}}"#
            ),
            format!(
                r#"[{{"type": "file", "path": "Qwen3-4B-Q8_0.gguf", "size": 1, "lfs": {{"oid": "{oid}", "size": 9}}}},
                    {{"type": "file", "path": "Qwen3-4B-Q4_K_M.gguf", "size": 1, "lfs": {{"oid": "{oid}", "size": 5}}}},
                    {{"type": "file", "path": "README.md", "size": 3}},
                    {{"type": "file", "path": "lora/Qwen3-4B-lora.gguf", "size": 1, "lfs": {{"oid": "{oid}", "size": 2}}}},
                    {{"type": "file", "path": "Qwen3-4B-nohash.gguf", "size": 4}}]"#
            ),
        ];
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for body in responses {
                let (mut stream, _) = listener.accept().await.expect("accept");
                let mut buffer = [0_u8; 4096];
                let read = stream.read(&mut buffer).await.expect("read");
                requests.push(String::from_utf8_lossy(&buffer[..read]).into_owned());
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.expect("write");
            }
            requests
        });
        let browser = HuggingFaceBrowser::with_endpoint(
            lettuce_network::JsonClient::new().expect("client"),
            format!("http://{address}"),
        );
        assert_eq!(
            browser
                .bundle_files(
                    &lettuce_settings::InMemorySecretStore::default(),
                    &NoHeaders,
                    "z-image-turbo",
                    "unsloth/Qwen3-4B-GGUF",
                    DiffusionComponentRole::VisionEncoder,
                )
                .await,
            Err(message(
                "vision_encoder is not used by the selected architecture."
            ))
        );
        let files = browser
            .bundle_files(
                &lettuce_settings::InMemorySecretStore::default(),
                &NoHeaders,
                "z-image-turbo",
                "https://huggingface.co/unsloth/Qwen3-4B-GGUF",
                DiffusionComponentRole::TextEncoder,
            )
            .await
            .expect("files");
        let requests = server.await.expect("requests");
        assert!(requests[1].starts_with(&format!(
            "GET /api/models/unsloth/Qwen3-4B-GGUF/tree/{sha}?recursive=true "
        )));
        assert!(
            files.is_empty(),
            "an 8B encoder header is refused: {files:?}"
        );
    }
}
