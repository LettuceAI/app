//! Assembling a local image model from Hugging Face files: which repositories
//! and files fit a catalog architecture's roles, and the bundle manifest that
//! tracks a download until the model is registered.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{DiffusionComponentRole, DiffusionProfile};

/// `owner/name` from a model id or a `https://huggingface.co/...` URL.
#[must_use]
pub fn parse_bundle_model_id(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_end_matches('/');
    let path = trimmed
        .strip_prefix("https://huggingface.co/")
        .unwrap_or(trimmed)
        .split(['?', '#'])
        .next()?;
    let parts = path
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    (parts.len() >= 2).then(|| format!("{}/{}", parts[0], parts[1]))
}

fn ancestry_values(card_data: &Value) -> Vec<String> {
    ["base_model", "base_models"]
        .into_iter()
        .filter_map(|key| card_data.get(key))
        .flat_map(|value| match value {
            Value::String(value) => vec![value.clone()],
            Value::Array(values) => values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            _ => Vec::new(),
        })
        .collect()
}

/// Whether a repository is this architecture's diffusion model, by its id,
/// tags and declared base models; the reason is shown either way.
#[must_use]
pub fn repo_compatibility(
    profile: &DiffusionProfile,
    model_id: &str,
    tags: &[String],
    card_data: &Value,
) -> (bool, &'static str) {
    let mut evidence = vec![model_id.to_ascii_lowercase()];
    evidence.extend(tags.iter().map(|value| value.to_ascii_lowercase()));
    evidence.extend(
        ancestry_values(card_data)
            .into_iter()
            .map(|value| value.to_ascii_lowercase()),
    );
    let joined = evidence.join(" ");
    let explicitly_wrong_variant = match profile.id.as_str() {
        "z-image" => joined.contains("z-image-turbo") || joined.contains("z_image_turbo"),
        "flux-2-klein-9b" => joined.contains("klein-base-9b"),
        "krea-2-turbo" => joined.contains("krea-2-raw") || joined.contains("krea-2-base"),
        "krea-2-raw" => joined.contains("krea-2-turbo"),
        _ => false,
    };
    if explicitly_wrong_variant {
        return (
            false,
            "The repository belongs to a different variant of this model family.",
        );
    }
    if profile
        .diffusion_markers
        .iter()
        .any(|marker| evidence.iter().any(|value| value.contains(marker.as_str())))
    {
        return (
            true,
            "Architecture or declared base-model ancestry matches this recipe.",
        );
    }
    (
        false,
        "This repository does not declare compatible architecture or base-model ancestry.",
    )
}

fn vae_family_term(profile: &DiffusionProfile) -> &'static str {
    if profile.id.starts_with("krea") {
        "wan"
    } else if profile.id == "qwen-image-edit-2511" {
        "qwen"
    } else {
        "flux"
    }
}

/// What a role search looks for when the query is empty.
#[must_use]
pub fn role_default_query(profile: &DiffusionProfile, role: DiffusionComponentRole) -> String {
    match role {
        DiffusionComponentRole::DiffusionModel => profile
            .diffusion_markers
            .first()
            .cloned()
            .unwrap_or_default(),
        DiffusionComponentRole::TextEncoder | DiffusionComponentRole::VisionEncoder => {
            profile.encoder_markers.first().cloned().unwrap_or_default()
        }
        DiffusionComponentRole::Vae => format!("{} vae", vae_family_term(profile)),
    }
}

/// Whether a search listing looks like a repository for `role`.
#[must_use]
pub fn role_search_matches(
    profile: &DiffusionProfile,
    role: DiffusionComponentRole,
    model_id: &str,
    tags: &[String],
) -> bool {
    let mut evidence = vec![model_id.to_ascii_lowercase()];
    evidence.extend(tags.iter().map(|tag| tag.to_ascii_lowercase()));
    let matches_any = |markers: &[&str]| {
        markers
            .iter()
            .any(|marker| evidence.iter().any(|value| value.contains(marker)))
    };
    let diffusion = profile
        .diffusion_markers
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let encoders = profile
        .encoder_markers
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    match role {
        DiffusionComponentRole::DiffusionModel => matches_any(&diffusion),
        DiffusionComponentRole::TextEncoder => matches_any(&encoders),
        DiffusionComponentRole::VisionEncoder => matches_any(&encoders) || matches_any(&["mmproj"]),
        DiffusionComponentRole::Vae => matches_any(&["vae", vae_family_term(profile), "ae."]),
    }
}

/// The file format a bundle search filters by: safetensors when asked,
/// else GGUF.
#[must_use]
pub fn normalized_bundle_format(format: Option<&str>) -> &'static str {
    match format.map(str::trim) {
        Some("safetensors") => "safetensors",
        _ => "gguf",
    }
}

/// A relative path with only normal components.
#[must_use]
pub fn safe_relative_path(path: &str) -> bool {
    let path = Path::new(path);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

/// Adapters, training state and pickled checkpoints are never components.
#[must_use]
pub fn excluded_artifact(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    [
        "lora",
        "adapter",
        "optimizer",
        "training",
        "checkpoint",
        "global_step",
        "scheduler",
        "random_state",
        ".ckpt",
        ".pt",
        ".pth",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

#[must_use]
pub fn bundle_file_format(path: &str) -> Option<&'static str> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".gguf") {
        Some("gguf")
    } else if lower.ends_with(".safetensors") {
        Some("safetensors")
    } else if lower.ends_with(".sft") {
        Some("sft")
    } else {
        None
    }
}

#[must_use]
pub fn bundle_quantization(path: &str) -> Option<String> {
    let upper = path.to_ascii_uppercase();
    [
        "Q2_K", "Q3_K_S", "Q3_K_M", "Q3_K", "Q4_0", "Q4_K_M", "Q5_K_M", "Q6_K", "Q8_0", "BF16",
        "F16",
    ]
    .into_iter()
    .find(|value| upper.contains(value))
    .map(str::to_owned)
}

/// Whether a file of a repository can fill a non-diffusion `role`.
#[must_use]
pub fn role_compatible(
    profile: &DiffusionProfile,
    role: DiffusionComponentRole,
    model_id: &str,
    path: &str,
) -> bool {
    let evidence = format!("{model_id} {path}").to_ascii_lowercase();
    match role {
        DiffusionComponentRole::DiffusionModel => profile
            .diffusion_markers
            .iter()
            .any(|marker| evidence.contains(marker.as_str())),
        DiffusionComponentRole::TextEncoder => profile
            .encoder_markers
            .iter()
            .any(|marker| evidence.contains(marker.as_str())),
        DiffusionComponentRole::VisionEncoder => {
            profile.id == "qwen-image-edit-2511"
                && (evidence.contains("mmproj") || evidence.contains("vision"))
        }
        DiffusionComponentRole::Vae if profile.id.starts_with("krea") => {
            evidence.contains("wan") && evidence.contains("vae")
        }
        DiffusionComponentRole::Vae if profile.id == "qwen-image-edit-2511" => {
            evidence.contains("qwen") && evidence.contains("vae")
        }
        DiffusionComponentRole::Vae => {
            (evidence.contains("flux") || evidence.contains("ae."))
                && (evidence.contains("vae") || evidence.contains("ae."))
        }
    }
}

/// How many leading bytes of a GGUF encoder are read for its architecture.
pub const ENCODER_HINT_BYTES: u64 = 1_048_576;

/// The encoder family a GGUF header names, from its first bytes.
#[must_use]
pub fn gguf_architecture_hint(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    [
        "qwen2.5-vl-7b",
        "qwen2.5vl-7b",
        "qwen3-vl-4b",
        "qwen3vl-4b",
        "qwen3-8b",
        "qwen3-4b",
        "z-image",
        "flux",
        "wan",
    ]
    .into_iter()
    .find(|marker| text.contains(marker))
    .map(str::to_owned)
}

/// A repository file checked against a role, pinned to a revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleAsset {
    pub selection_id: String,
    pub profile_id: String,
    pub role: DiffusionComponentRole,
    pub model_id: String,
    pub revision: String,
    pub relative_path: String,
    pub format: String,
    pub quantization: Option<String>,
    pub size: u64,
    pub sha256: String,
    pub architecture: Option<String>,
    pub gated: bool,
}

impl BundleAsset {
    /// Where the asset lands below the bundle folder.
    pub fn local_segments(&self) -> Result<Vec<String>, String> {
        let mut parts = self.model_id.split('/');
        let author = parts
            .next()
            .ok_or_else(|| "Invalid Hugging Face author.".to_owned())?;
        let repository = parts
            .next()
            .ok_or_else(|| "Invalid Hugging Face repository.".to_owned())?;
        if parts.next().is_some()
            || !safe_relative_path(author)
            || !safe_relative_path(repository)
            || !safe_relative_path(&self.revision)
            || !safe_relative_path(&self.relative_path)
        {
            return Err("The selected repository contains an unsafe destination path.".to_owned());
        }
        Ok([author, repository, &self.revision]
            .into_iter()
            .map(str::to_owned)
            .chain(self.relative_path.split('/').map(str::to_owned))
            .collect())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestAsset {
    #[serde(flatten)]
    pub asset: BundleAsset,
    pub local_path: String,
    pub verified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BundleRegistrationState {
    Downloading,
    Registered,
    SetupFailed,
}

/// One bundle download, kept next to its files (the old app's manifest
/// format).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleManifest {
    pub bundle_id: String,
    pub profile_id: String,
    pub display_name: String,
    pub runtime_release: String,
    pub runtime_asset: String,
    pub runnability: Option<Value>,
    pub assets: Vec<ManifestAsset>,
    pub registration_state: BundleRegistrationState,
    pub model_id: Option<String>,
    pub setup_error: Option<String>,
}

impl BundleManifest {
    /// Every asset verified and on disk.
    #[must_use]
    pub fn complete(&self) -> bool {
        self.assets
            .iter()
            .all(|asset| asset.verified && Path::new(&asset.local_path).is_file())
    }
}

/// `<image root>/huggingface`, where bundle files and manifests live.
#[must_use]
pub fn bundle_root(image_root: &Path) -> PathBuf {
    image_root.join("huggingface")
}

fn manifest_path(image_root: &Path, bundle_id: &str) -> Result<PathBuf, String> {
    if !safe_relative_path(bundle_id) || bundle_id.contains(['/', '\\']) {
        return Err("Invalid bundle id.".to_owned());
    }
    Ok(bundle_root(image_root)
        .join("bundles")
        .join(format!("{bundle_id}.json")))
}

pub fn write_bundle_manifest(image_root: &Path, manifest: &BundleManifest) -> Result<(), String> {
    let path = manifest_path(image_root, &manifest.bundle_id)?;
    let parent = path
        .parent()
        .ok_or_else(|| "Invalid bundle manifest path.".to_owned())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("Failed to create the bundle manifest directory: {error}"))?;
    let temp = path.with_extension("json.tmp");
    std::fs::write(
        &temp,
        serde_json::to_vec_pretty(manifest)
            .map_err(|error| format!("Failed to serialize the bundle manifest: {error}"))?,
    )
    .map_err(|error| format!("Failed to write the bundle manifest: {error}"))?;
    std::fs::rename(temp, path)
        .map_err(|error| format!("Failed to finalize the bundle manifest: {error}"))
}

pub fn read_bundle_manifest(image_root: &Path, bundle_id: &str) -> Result<BundleManifest, String> {
    serde_json::from_slice(
        &std::fs::read(manifest_path(image_root, bundle_id)?)
            .map_err(|error| format!("Failed to read the bundle manifest: {error}"))?,
    )
    .map_err(|error| format!("Failed to parse the bundle manifest: {error}"))
}

/// Every readable manifest below `image_root`.
#[must_use]
pub fn bundle_manifests(image_root: &Path) -> Vec<BundleManifest> {
    let Ok(entries) = std::fs::read_dir(bundle_root(image_root).join("bundles")) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| serde_json::from_slice(&std::fs::read(entry.path()).ok()?).ok())
        .collect()
}

/// Links a verified copy of the same file from another bundle into place,
/// so it is not downloaded twice.
pub fn reuse_verified_asset(image_root: &Path, asset: &ManifestAsset) {
    let destination = Path::new(&asset.local_path);
    if destination.exists() {
        return;
    }
    let Some(source) = bundle_manifests(image_root)
        .into_iter()
        .flat_map(|manifest| manifest.assets)
        .find(|candidate| {
            candidate.verified
                && candidate.asset.sha256 == asset.asset.sha256
                && Path::new(&candidate.local_path).is_file()
        })
        .map(|candidate| candidate.local_path)
    else {
        return;
    };
    if let Some(parent) = destination.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::hard_link(source, destination);
}

/// The role the image model list shows for a file (`llm`, `llmVision`,
/// `vae`, `diffusionModel`).
#[must_use]
pub const fn listed_image_role(role: DiffusionComponentRole) -> &'static str {
    match role {
        DiffusionComponentRole::TextEncoder => "llm",
        DiffusionComponentRole::VisionEncoder => "llmVision",
        DiffusionComponentRole::Vae => "vae",
        DiffusionComponentRole::DiffusionModel => "diffusionModel",
    }
}

/// A file's role guessed from its lowercase name.
#[must_use]
pub fn infer_image_role(lower_filename: &str) -> DiffusionComponentRole {
    if lower_filename.contains("vae") || lower_filename.starts_with("ae.") {
        DiffusionComponentRole::Vae
    } else if lower_filename.contains("mmproj") || lower_filename.contains("vision") {
        DiffusionComponentRole::VisionEncoder
    } else if ["qwen3", "qwen2.5", "qwen2_5", "t5", "umt5", "clip"]
        .iter()
        .any(|marker| lower_filename.contains(marker))
    {
        DiffusionComponentRole::TextEncoder
    } else {
        DiffusionComponentRole::DiffusionModel
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::diffusion_catalog;

    fn profile(id: &str) -> &'static DiffusionProfile {
        diffusion_catalog().profile(id).expect("profile")
    }

    #[test]
    fn rejects_traversal_and_training_artifacts() {
        assert!(!safe_relative_path("../model.gguf"));
        assert!(!safe_relative_path("/tmp/model.gguf"));
        assert!(excluded_artifact("optimizer/model.safetensors"));
        assert!(excluded_artifact("flux-lora.safetensors"));
    }

    #[test]
    fn rejects_cross_family_components() {
        let z_image = profile("z-image-turbo");
        assert!(!role_compatible(
            z_image,
            DiffusionComponentRole::TextEncoder,
            "unsloth/Qwen3-8B-GGUF",
            "Qwen3-8B-Q4_K_M.gguf"
        ));
        assert!(role_compatible(
            z_image,
            DiffusionComponentRole::TextEncoder,
            "unsloth/Qwen3-4B-GGUF",
            "Qwen3-4B-Q4_K_M.gguf"
        ));
    }

    #[test]
    fn classifies_every_supported_diffusion_architecture() {
        for (profile_id, model_id) in [
            ("z-image-turbo", "vendor/Z-Image-Turbo-GGUF"),
            ("z-image", "vendor/Z-Image-GGUF"),
            ("flux-2-klein-4b", "vendor/FLUX.2-klein-4B-GGUF"),
            ("flux-2-klein-9b", "vendor/FLUX.2-klein-9B-GGUF"),
            ("flux-2-klein-base-9b", "vendor/FLUX.2-klein-base-9B-GGUF"),
            ("krea-2-turbo", "vendor/Krea-2-Turbo-GGUF"),
            ("krea-2-raw", "vendor/Krea-2-Raw-GGUF"),
            ("qwen-image-edit-2511", "vendor/Qwen-Image-Edit-2511-GGUF"),
        ] {
            assert!(
                repo_compatibility(profile(profile_id), model_id, &[], &json!({})).0,
                "{profile_id} did not classify {model_id}"
            );
        }
    }

    #[test]
    fn accepts_declared_fine_tune_ancestry_but_rejects_cross_family_ancestry() {
        let flux = profile("flux-2-klein-4b");
        assert!(
            repo_compatibility(
                flux,
                "artist/custom-style",
                &[],
                &json!({"base_model": "black-forest-labs/FLUX.2-klein-4B"})
            )
            .0
        );
        assert!(
            !repo_compatibility(
                flux,
                "artist/custom-style",
                &[],
                &json!({"base_model": "Tongyi-MAI/Z-Image"})
            )
            .0
        );
    }

    #[test]
    fn role_search_filters_listings_by_marker() {
        let z_image = profile("z-image-turbo");
        use DiffusionComponentRole::{DiffusionModel, TextEncoder, Vae, VisionEncoder};
        assert!(role_search_matches(
            z_image,
            DiffusionModel,
            "vendor/Z-Image-Turbo-GGUF",
            &[]
        ));
        assert!(!role_search_matches(
            z_image,
            DiffusionModel,
            "vendor/FLUX.2-klein-4B-GGUF",
            &[]
        ));
        assert!(role_search_matches(
            z_image,
            TextEncoder,
            "unsloth/Qwen3-4B-GGUF",
            &[]
        ));
        assert!(!role_search_matches(
            z_image,
            TextEncoder,
            "unsloth/Qwen3-8B-GGUF",
            &[]
        ));
        assert!(role_search_matches(
            z_image,
            Vae,
            "vendor/flux-vae-collection",
            &[]
        ));
        assert!(role_search_matches(
            profile("qwen-image-edit-2511"),
            VisionEncoder,
            "vendor/some-projector",
            &["mmproj".to_owned()]
        ));
    }

    #[test]
    fn qwen_edit_recipe_requires_a_vision_encoder() {
        assert!(
            profile("qwen-image-edit-2511")
                .required_roles()
                .contains(&DiffusionComponentRole::VisionEncoder)
        );
        assert!(
            !profile("z-image")
                .required_roles()
                .contains(&DiffusionComponentRole::VisionEncoder)
        );
    }

    #[test]
    fn manifests_round_trip_in_the_old_format_and_ids_are_parsed_from_urls() {
        let root = std::env::temp_dir().join(format!(
            "bundle-manifest-{}",
            lettuce_types::OperationId::new()
        ));
        let manifest = BundleManifest {
            bundle_id: "b1".to_owned(),
            profile_id: "z-image-turbo".to_owned(),
            display_name: "Mine".to_owned(),
            runtime_release: "master-1".to_owned(),
            runtime_asset: "asset".to_owned(),
            runnability: None,
            assets: vec![ManifestAsset {
                asset: BundleAsset {
                    selection_id: "s".to_owned(),
                    profile_id: "z-image-turbo".to_owned(),
                    role: DiffusionComponentRole::Vae,
                    model_id: "org/repo".to_owned(),
                    revision: "abc".to_owned(),
                    relative_path: "vae/ae.safetensors".to_owned(),
                    format: "safetensors".to_owned(),
                    quantization: None,
                    size: 3,
                    sha256: "ff".to_owned(),
                    architecture: None,
                    gated: false,
                },
                local_path: "/x".to_owned(),
                verified: false,
            }],
            registration_state: BundleRegistrationState::Downloading,
            model_id: None,
            setup_error: None,
        };
        write_bundle_manifest(&root, &manifest).expect("write");
        let stored =
            std::fs::read_to_string(root.join("huggingface/bundles/b1.json")).expect("stored");
        assert!(stored.contains("\"registrationState\": \"downloading\""));
        assert!(stored.contains("\"role\": \"vae\""));
        assert!(stored.contains("\"localPath\": \"/x\""));
        assert_eq!(read_bundle_manifest(&root, "b1").expect("read"), manifest);
        assert_eq!(
            manifest.assets[0].asset.local_segments().expect("segments"),
            ["org", "repo", "abc", "vae", "ae.safetensors"]
        );
        assert!(read_bundle_manifest(&root, "../b1").is_err());
        let _ = std::fs::remove_dir_all(root);
        assert_eq!(
            parse_bundle_model_id("https://huggingface.co/org/repo/tree/main?x=1").as_deref(),
            Some("org/repo")
        );
        assert_eq!(parse_bundle_model_id("single"), None);
        assert_eq!(
            infer_image_role("ae.safetensors"),
            DiffusionComponentRole::Vae
        );
        assert_eq!(
            infer_image_role("qwen3-4b-q4.gguf"),
            DiffusionComponentRole::TextEncoder
        );
    }
}
