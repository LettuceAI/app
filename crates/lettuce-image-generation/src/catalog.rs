use std::collections::{BTreeMap, HashSet};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

const CATALOG_JSON: &str = include_str!("../resources/stable-diffusion-cpp-catalog.json");
const CATALOG_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffusionComponentRole {
    DiffusionModel,
    TextEncoder,
    Vae,
    VisionEncoder,
}

/// One pinned Hugging Face file of a local image model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffusionComponent {
    pub role: DiffusionComponentRole,
    pub repo: String,
    pub revision: String,
    pub filename: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffusionVariant {
    pub id: String,
    pub label: String,
    pub description: String,
    pub recommended: bool,
    pub smaller: bool,
    pub diffusion: DiffusionComponent,
}

/// A one-click stable-diffusion.cpp model with its pinned files and the
/// markers that recognise user-picked files of the same model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffusionProfile {
    pub id: String,
    pub display_name: String,
    pub family: String,
    pub description: String,
    pub license: String,
    pub source_url: String,
    pub supports_text_to_image: bool,
    pub supports_image_edit: bool,
    pub max_reference_images: Option<u8>,
    pub requires_reference_image: bool,
    pub recommended_for_scenes: bool,
    pub default_width: u32,
    pub default_height: u32,
    pub default_steps: u16,
    pub default_cfg: f32,
    pub minimum_runtime_build: Option<u32>,
    pub variants: Vec<DiffusionVariant>,
    pub shared_components: Vec<DiffusionComponent>,
    pub diffusion_markers: Vec<String>,
    pub encoder_markers: Vec<String>,
    pub encoder_parameter_billions: f32,
}

/// The pinned recommended upscaler model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedUpscaler {
    pub filename: String,
    pub url: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffusionCatalog {
    pub format_version: u32,
    pub profiles: Vec<DiffusionProfile>,
    pub upscaler: PinnedUpscaler,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DiffusionCatalogError {
    #[error("Unknown local image architecture: {0}")]
    UnknownProfile(String),
    #[error("Unknown local image model: {0}")]
    UnknownModel(String),
    #[error("Unknown {display_name} variant: {id}")]
    UnknownVariant { display_name: String, id: String },
    #[error(
        "{display_name} requires stable-diffusion.cpp engine build {minimum} or newer. Install or switch to a compatible engine build first."
    )]
    RuntimeTooOld { display_name: String, minimum: u32 },
}

/// The bundled catalog. It is validated once; a malformed bundle is a build
/// defect caught by the tests.
pub fn diffusion_catalog() -> &'static DiffusionCatalog {
    static CATALOG: OnceLock<DiffusionCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let catalog: DiffusionCatalog =
            serde_json::from_str(CATALOG_JSON).expect("bundled diffusion catalog parses");
        assert!(
            catalog.is_valid(),
            "bundled diffusion catalog is inconsistent"
        );
        catalog
    })
}

fn is_hex(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_safe_relative_path(value: &str) -> bool {
    !value.is_empty()
        && value.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        })
}

impl DiffusionComponent {
    fn is_valid(&self) -> bool {
        is_hex(&self.revision, 40)
            && is_hex(&self.sha256, 64)
            && self.bytes > 0
            && is_safe_relative_path(&self.filename)
            && self.repo.split('/').count() == 2
            && is_safe_relative_path(&self.repo)
    }

    /// The file name alone, as components are stored under their hash.
    #[must_use]
    pub fn basename(&self) -> &str {
        self.filename.rsplit('/').next().unwrap_or(&self.filename)
    }
}

impl DiffusionCatalog {
    fn is_valid(&self) -> bool {
        let mut ids = HashSet::new();
        self.format_version == CATALOG_FORMAT_VERSION
            && is_hex(&self.upscaler.sha256, 64)
            && self.upscaler.bytes > 0
            && self.profiles.iter().all(|profile| {
                let mut variants = HashSet::new();
                ids.insert(profile.id.as_str())
                    && !profile.variants.is_empty()
                    && profile.variants.iter().all(|variant| {
                        variants.insert(variant.id.as_str())
                            && variant.diffusion.role == DiffusionComponentRole::DiffusionModel
                            && variant.diffusion.is_valid()
                    })
                    && profile.shared_components.iter().all(|component| {
                        component.role != DiffusionComponentRole::DiffusionModel
                            && component.is_valid()
                    })
            })
    }

    pub fn profile(&self, id: &str) -> Result<&DiffusionProfile, DiffusionCatalogError> {
        self.profiles
            .iter()
            .find(|profile| profile.id == id)
            .ok_or_else(|| DiffusionCatalogError::UnknownProfile(id.to_owned()))
    }

    /// A profile and variant for an install; an unknown profile is reported
    /// as a model rather than an architecture.
    pub fn find_variant(
        &self,
        profile_id: &str,
        variant_id: &str,
    ) -> Result<(&DiffusionProfile, &DiffusionVariant), DiffusionCatalogError> {
        let profile = self
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .ok_or_else(|| DiffusionCatalogError::UnknownModel(profile_id.to_owned()))?;
        Ok((profile, profile.variant(variant_id)?))
    }

    /// The role of a catalog file by its SHA-256, first match in catalog order.
    #[must_use]
    pub fn component_role(&self, sha256: &str) -> Option<DiffusionComponentRole> {
        self.profiles.iter().find_map(|profile| {
            profile
                .variants
                .iter()
                .map(|variant| &variant.diffusion)
                .chain(&profile.shared_components)
                .find(|component| component.sha256 == sha256)
                .map(|component| component.role)
        })
    }

    /// The profile whose diffusion marker best matches the file's name and
    /// parent directory (longest marker wins, first profile on ties).
    #[must_use]
    pub fn detect_profile(&self, path: &str) -> Option<&DiffusionProfile> {
        let evidence = std::path::Path::new(path.trim())
            .iter()
            .rev()
            .take(2)
            .map(|component| component.to_string_lossy().to_ascii_lowercase())
            .collect::<Vec<_>>()
            .join(" ");
        let mut best: Option<(usize, &DiffusionProfile)> = None;
        for profile in &self.profiles {
            let longest = profile
                .diffusion_markers
                .iter()
                .filter(|marker| evidence.contains(marker.as_str()))
                .map(String::len)
                .max();
            if let Some(length) = longest
                && best.is_none_or(|(current, _)| length > current)
            {
                best = Some((length, profile));
            }
        }
        best.map(|(_, profile)| profile)
    }
}

impl DiffusionProfile {
    pub fn variant(&self, id: &str) -> Result<&DiffusionVariant, DiffusionCatalogError> {
        self.variants
            .iter()
            .find(|variant| variant.id == id)
            .ok_or_else(|| DiffusionCatalogError::UnknownVariant {
                display_name: self.display_name.clone(),
                id: id.to_owned(),
            })
    }

    /// The variant's diffusion file followed by the profile's shared files.
    #[must_use]
    pub fn components<'a>(&'a self, variant: &'a DiffusionVariant) -> Vec<&'a DiffusionComponent> {
        std::iter::once(&variant.diffusion)
            .chain(&self.shared_components)
            .collect()
    }

    #[must_use]
    pub fn download_bytes(&self, variant: &DiffusionVariant) -> u64 {
        self.components(variant)
            .iter()
            .map(|component| component.bytes)
            .sum()
    }

    /// The installed model name: the profile, then the variant label without
    /// its recommendation suffix.
    #[must_use]
    pub fn installed_display_name(&self, variant: &DiffusionVariant) -> String {
        format!(
            "{} ({})",
            self.display_name,
            variant
                .label
                .replace(" (recommended)", "")
                .replace(" (smaller)", "")
        )
    }

    /// The roles a user-assembled bundle must fill, diffusion model first.
    #[must_use]
    pub fn required_roles(&self) -> Vec<DiffusionComponentRole> {
        let mut roles = vec![DiffusionComponentRole::DiffusionModel];
        for component in &self.shared_components {
            if !roles.contains(&component.role) {
                roles.push(component.role);
            }
        }
        roles
    }

    /// Each role's suggested repository: the first variant's, then the first
    /// shared file of each role.
    #[must_use]
    pub fn recommended_repositories(&self) -> BTreeMap<DiffusionComponentRole, &str> {
        let mut repositories = BTreeMap::new();
        if let Some(variant) = self.variants.first() {
            repositories.insert(
                DiffusionComponentRole::DiffusionModel,
                variant.diffusion.repo.as_str(),
            );
        }
        for component in &self.shared_components {
            repositories
                .entry(component.role)
                .or_insert(component.repo.as_str());
        }
        repositories
    }

    /// Accepts a runtime only when its `master-<build>` tag is at least the
    /// profile's minimum build.
    pub fn check_runtime(&self, runtime_release: &str) -> Result<(), DiffusionCatalogError> {
        let Some(minimum) = self.minimum_runtime_build else {
            return Ok(());
        };
        if runtime_build_number(runtime_release).is_some_and(|build| build >= minimum) {
            return Ok(());
        }
        Err(DiffusionCatalogError::RuntimeTooOld {
            display_name: self.display_name.clone(),
            minimum,
        })
    }
}

#[must_use]
pub fn runtime_build_number(release: &str) -> Option<u32> {
    release
        .strip_prefix("master-")?
        .split('-')
        .next()?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_catalog_keeps_the_legacy_profiles_and_pins() {
        let catalog = diffusion_catalog();
        assert_eq!(
            catalog
                .profiles
                .iter()
                .map(|profile| profile.id.as_str())
                .collect::<Vec<_>>(),
            [
                "z-image-turbo",
                "z-image",
                "flux-2-klein-4b",
                "flux-2-klein-9b",
                "flux-2-klein-base-9b",
                "krea-2-turbo",
                "krea-2-raw",
                "qwen-image-edit-2511",
            ]
        );
        let turbo = catalog.profile("z-image-turbo").expect("turbo");
        let q4 = turbo.variant("q4-k").expect("q4");
        assert_eq!(
            turbo.download_bytes(q4),
            3_864_250_304 + 2_497_281_120 + 335_304_388
        );
        assert_eq!(turbo.installed_display_name(q4), "Z-Image Turbo (Q4 K)");
        assert_eq!(turbo.default_cfg, 0.0);
        assert_eq!(turbo.max_reference_images, Some(0));
        let qwen = catalog.profile("qwen-image-edit-2511").expect("qwen");
        assert_eq!(
            qwen.required_roles(),
            [
                DiffusionComponentRole::DiffusionModel,
                DiffusionComponentRole::TextEncoder,
                DiffusionComponentRole::VisionEncoder,
                DiffusionComponentRole::Vae,
            ]
        );
        assert_eq!(qwen.encoder_parameter_billions, 7.0);
        assert_eq!(
            catalog.upscaler.sha256,
            "f872d837d3c90ed2e05227bed711af5671a6fd1c9f7d7e91c911a61f155e99da"
        );
        assert_eq!(catalog.upscaler.bytes, 17_938_799);
        assert_eq!(
            catalog
                .component_role("afc8e28272cd15db3919bacdb6918ce9c1ed22e96cb12c4d5ed0fba823529e38"),
            Some(DiffusionComponentRole::Vae)
        );
        assert_eq!(
            catalog
                .profile("missing")
                .map_err(|error| error.to_string()),
            Err("Unknown local image architecture: missing".to_owned())
        );
    }

    #[test]
    fn krea_needs_build_721_and_files_are_detected_by_the_longest_marker() {
        let catalog = diffusion_catalog();
        let krea = catalog.profile("krea-2-turbo").expect("krea");
        assert_eq!(krea.check_runtime("master-721-abc1234"), Ok(()));
        assert_eq!(
            krea.check_runtime("master-720-abc1234")
                .map_err(|error| error.to_string()),
            Err("Krea 2 Turbo requires stable-diffusion.cpp engine build 721 or newer. Install or switch to a compatible engine build first.".to_owned())
        );
        assert!(krea.check_runtime("v1.0").is_err());
        assert_eq!(
            catalog
                .profile("z-image")
                .expect("z-image")
                .check_runtime("v1.0"),
            Ok(())
        );
        let detected = |path: &str| {
            catalog
                .detect_profile(path)
                .map(|profile| profile.id.as_str())
        };
        assert_eq!(
            detected("/models/Z-Image-Turbo-GGUF/z_image_turbo-Q4_K.gguf"),
            Some("z-image-turbo")
        );
        assert_eq!(
            detected("/models/flux/./FLUX.2-klein-base-9B-Q4_0.gguf"),
            Some("flux-2-klein-base-9b")
        );
        assert_eq!(
            catalog
                .find_variant("z-image-turbo", "q9")
                .map(|_| ())
                .map_err(|error| error.to_string()),
            Err("Unknown Z-Image Turbo variant: q9".to_owned())
        );
        assert_eq!(
            catalog
                .find_variant("missing", "q4-k")
                .map(|_| ())
                .map_err(|error| error.to_string()),
            Err("Unknown local image model: missing".to_owned())
        );
        assert_eq!(detected("/models/other/model.gguf"), None);
    }
}
