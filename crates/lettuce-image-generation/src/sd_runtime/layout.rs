//! Where engine builds, downloads, components, LoRAs and upscalers live. The
//! layout is the one legacy installs use, so builds and models they already
//! downloaded keep working.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::policy::ComputePolicy;
use super::releases::{RuntimeAsset, RuntimePlatform, runtime_backend};
use crate::DiffusionComponent;

pub const LORA_EXTENSIONS: [&str; 3] = ["safetensors", "ckpt", "pt"];
pub const UPSCALER_EXTENSIONS: [&str; 4] = ["pth", "pt", "safetensors", "gguf"];
const MANIFEST_FILE: &str = ".lettuce-runtime-manifest.json";
const ACTIVE_RUNTIME_FILE: &str = ".active-runtime.json";
const COMPUTE_POLICY_FILE: &str = ".lettuce-compute-policy.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffusionPaths {
    /// `<app>/runtimes/stable-diffusion.cpp`
    pub runtimes: PathBuf,
    /// `<app>/downloads/sdcpp`
    pub downloads: PathBuf,
    /// The configurable image model folder; components live below it.
    pub image_root: PathBuf,
    /// `<app>/models/image` when the user moved the image folder elsewhere:
    /// components already downloaded there stay in use.
    pub default_image_root: Option<PathBuf>,
    /// `<app>/models/loras`
    pub loras: PathBuf,
    /// `<app>/models/upscalers`
    pub upscalers: PathBuf,
    /// `<app>/cache/upscale`
    pub upscale_scratch: PathBuf,
}

impl DiffusionPaths {
    /// The layout below the app data folder, with the image model folder the
    /// user configured.
    #[must_use]
    pub fn legacy_layout(app_dir: &Path, image_root: PathBuf) -> Self {
        let default_image_root = app_dir.join("models").join("image");
        Self {
            runtimes: app_dir.join("runtimes").join("stable-diffusion.cpp"),
            downloads: app_dir.join("downloads").join("sdcpp"),
            default_image_root: (default_image_root != image_root).then_some(default_image_root),
            image_root,
            loras: app_dir.join("models").join("loras"),
            upscalers: app_dir.join("models").join("upscalers"),
            upscale_scratch: app_dir.join("cache").join("upscale"),
        }
    }

    #[must_use]
    pub fn runtime_root(&self, release: &str, asset: &str) -> PathBuf {
        self.runtimes
            .join(safe_path_segment(release))
            .join(safe_path_segment(asset))
    }

    /// Where a release archive is downloaded, relative to `downloads`.
    #[must_use]
    pub fn archive_segments(release: &str, archive: &str) -> Vec<String> {
        vec![safe_path_segment(release), archive.to_owned()]
    }

    /// A catalog component's content-addressed location, relative to
    /// `image_root`.
    #[must_use]
    pub fn component_segments(component: &DiffusionComponent) -> Vec<String> {
        vec![
            "components".to_owned(),
            component.sha256.clone(),
            component.basename().to_owned(),
        ]
    }

    /// The component in the image folder, or in the default folder when only
    /// that one has it.
    #[must_use]
    pub fn component_path(&self, component: &DiffusionComponent) -> PathBuf {
        let relative = Self::component_segments(component)
            .iter()
            .fold(PathBuf::new(), |path, segment| path.join(segment));
        let configured = self.image_root.join(&relative);
        if let Some(default) = &self.default_image_root {
            let fallback = default.join(&relative);
            if !configured.exists() && fallback.exists() {
                return fallback;
            }
        }
        configured
    }
}

/// The folder-name rule: ASCII letters, digits, `-`, `_` and `.` kept,
/// anything else replaced by `_`.
#[must_use]
pub fn safe_path_segment(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

#[must_use]
pub const fn server_executable_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "sd-server.exe"
    } else {
        "sd-server"
    }
}

#[must_use]
pub const fn cli_executable_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "sd-cli.exe"
    } else {
        "sd-cli"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RuntimeManifest {
    archives: Vec<String>,
}

fn archive_marker(root: &Path, archive: &str) -> PathBuf {
    root.join(format!(
        ".lettuce-extracted-{}.complete",
        safe_path_segment(archive)
    ))
}

/// Records which archives make up an engine build before any is extracted.
pub fn write_runtime_manifest(root: &Path, asset: &RuntimeAsset) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|error| {
        format!("Failed to create stable-diffusion.cpp runtime directory: {error}")
    })?;
    let mut archives = vec![asset.name.clone()];
    archives.extend(
        asset
            .dependencies
            .iter()
            .map(|dependency| dependency.name.clone()),
    );
    let manifest = serde_json::to_vec_pretty(&RuntimeManifest { archives })
        .map_err(|error| format!("Failed to serialize runtime manifest: {error}"))?;
    std::fs::write(root.join(MANIFEST_FILE), manifest)
        .map_err(|error| format!("Failed to write runtime manifest: {error}"))
}

/// A build is complete when its server exists and, when a manifest lists
/// archives, every archive finished extracting.
#[must_use]
pub fn runtime_root_is_complete(root: &Path) -> bool {
    if !root.join(server_executable_name()).is_file() {
        return false;
    }
    let manifest_path = root.join(MANIFEST_FILE);
    if !manifest_path.is_file() {
        return true;
    }
    std::fs::read(manifest_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<RuntimeManifest>(&bytes).ok())
        .is_some_and(|manifest| {
            !manifest.archives.is_empty()
                && manifest
                    .archives
                    .iter()
                    .all(|archive| archive_marker(root, archive).is_file())
        })
}

/// Unpacks one archive into an engine build folder. Entries that would leave
/// the folder are skipped; the server and CLI become executable on Unix.
pub fn extract_runtime_archive(
    archive: &Path,
    archive_name: &str,
    destination: &Path,
) -> Result<(), String> {
    std::fs::create_dir_all(destination).map_err(|error| {
        format!("Failed to create stable-diffusion.cpp runtime directory: {error}")
    })?;
    let file = std::fs::File::open(archive)
        .map_err(|error| format!("Failed to open stable-diffusion.cpp runtime archive: {error}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| format!("Failed to read stable-diffusion.cpp runtime archive: {error}"))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Failed to read runtime archive entry: {error}"))?;
        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        let output = destination.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&output)
                .map_err(|error| format!("Failed to create runtime directory: {error}"))?;
            continue;
        }
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create runtime directory: {error}"))?;
        }
        let mut output_file = std::fs::File::create(&output)
            .map_err(|error| format!("Failed to extract runtime file: {error}"))?;
        std::io::copy(&mut entry, &mut output_file)
            .map_err(|error| format!("Failed to extract runtime file: {error}"))?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for executable in ["sd-server", "sd-cli"] {
            let path = destination.join(executable);
            if path.exists() {
                let mut permissions = std::fs::metadata(&path)
                    .map_err(|error| format!("Failed to inspect runtime executable: {error}"))?
                    .permissions();
                permissions.set_mode(0o755);
                std::fs::set_permissions(path, permissions)
                    .map_err(|error| format!("Failed to mark runtime executable: {error}"))?;
            }
        }
    }
    std::fs::write(archive_marker(destination, archive_name), b"ok")
        .map_err(|error| format!("Failed to finalize runtime extraction: {error}"))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveRuntime {
    pub release: String,
    pub asset: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledRuntime {
    pub release: String,
    pub asset: String,
    pub backend: String,
    pub size_bytes: u64,
    pub active: bool,
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

/// Complete engine builds, newest release first, then backend and archive.
#[must_use]
pub fn installed_runtimes(
    paths: &DiffusionPaths,
    platform: RuntimePlatform,
) -> Vec<InstalledRuntime> {
    let mut installed = Vec::new();
    let Ok(releases) = std::fs::read_dir(&paths.runtimes) else {
        return installed;
    };
    for release_entry in releases.flatten() {
        if !release_entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let release = release_entry.file_name().to_string_lossy().to_string();
        let Ok(assets) = std::fs::read_dir(release_entry.path()) else {
            continue;
        };
        for asset_entry in assets.flatten() {
            let asset_path = asset_entry.path();
            if !runtime_root_is_complete(&asset_path) {
                continue;
            }
            let asset = asset_entry.file_name().to_string_lossy().to_string();
            installed.push(InstalledRuntime {
                release: release.clone(),
                backend: runtime_backend(&asset, platform)
                    .unwrap_or("unknown")
                    .to_owned(),
                size_bytes: directory_size(&asset_path),
                asset,
                active: false,
            });
        }
    }
    installed.sort_by(|left, right| {
        right
            .release
            .cmp(&left.release)
            .then_with(|| left.backend.cmp(&right.backend))
            .then_with(|| left.asset.cmp(&right.asset))
    });
    installed
}

#[must_use]
pub fn runtime_is_installed(paths: &DiffusionPaths, release: &str, asset: &str) -> bool {
    runtime_root_is_complete(&paths.runtime_root(release, asset))
}

/// The saved engine selection when that build is still complete.
#[must_use]
pub fn saved_active_runtime(paths: &DiffusionPaths) -> Option<ActiveRuntime> {
    let bytes = std::fs::read(paths.runtimes.join(ACTIVE_RUNTIME_FILE)).ok()?;
    let selected = serde_json::from_slice::<ActiveRuntime>(&bytes).ok()?;
    runtime_is_installed(paths, &selected.release, &selected.asset).then_some(selected)
}

/// The saved selection, else the first installed build.
#[must_use]
pub fn effective_active_runtime(
    paths: &DiffusionPaths,
    installed: &[InstalledRuntime],
) -> Option<ActiveRuntime> {
    saved_active_runtime(paths).or_else(|| {
        installed.first().map(|runtime| ActiveRuntime {
            release: runtime.release.clone(),
            asset: runtime.asset.clone(),
        })
    })
}

pub fn save_active_runtime(
    paths: &DiffusionPaths,
    selected: &ActiveRuntime,
) -> std::io::Result<()> {
    std::fs::create_dir_all(&paths.runtimes)?;
    let bytes = serde_json::to_vec_pretty(selected).map_err(std::io::Error::other)?;
    std::fs::write(paths.runtimes.join(ACTIVE_RUNTIME_FILE), bytes)
}

pub fn clear_active_runtime(paths: &DiffusionPaths) -> std::io::Result<()> {
    match std::fs::remove_file(paths.runtimes.join(ACTIVE_RUNTIME_FILE)) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// The build's stored compute policy; missing or unreadable files mean the
/// default automatic policy.
#[must_use]
pub fn load_compute_policy(paths: &DiffusionPaths, release: &str, asset: &str) -> ComputePolicy {
    std::fs::read(paths.runtime_root(release, asset).join(COMPUTE_POLICY_FILE))
        .ok()
        .and_then(|bytes| ComputePolicy::from_stored_json(&bytes).ok())
        .unwrap_or_default()
}

pub fn save_compute_policy(
    paths: &DiffusionPaths,
    release: &str,
    asset: &str,
    policy: &ComputePolicy,
) -> std::io::Result<()> {
    let root = paths.runtime_root(release, asset);
    std::fs::create_dir_all(&root)?;
    let bytes = serde_json::to_vec_pretty(policy).map_err(std::io::Error::other)?;
    let path = root.join(COMPUTE_POLICY_FILE);
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(&temporary, &path)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use lettuce_types::OperationId;

    use super::*;

    fn paths() -> (PathBuf, DiffusionPaths) {
        let root = std::env::temp_dir().join(format!("sd-layout-{}", OperationId::new()));
        let paths = DiffusionPaths::legacy_layout(&root, root.join("models").join("image"));
        (root, paths)
    }

    fn zip_with(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .expect("entry");
            writer.write_all(bytes).expect("bytes");
        }
        writer.finish().expect("zip").into_inner()
    }

    #[test]
    fn builds_become_complete_once_every_listed_archive_is_extracted() {
        let (root, paths) = paths();
        let asset = RuntimeAsset {
            name: "sd-master-bin-Linux-x86_64-cuda.zip".to_owned(),
            backend: "cuda".to_owned(),
            bytes: 1,
            sha256: None,
            download_url: "https://example.invalid/a.zip".to_owned(),
            dependencies: vec![super::super::releases::RuntimeDependency {
                name: "cudart-linux.zip".to_owned(),
                bytes: 1,
                sha256: None,
                download_url: "https://example.invalid/b.zip".to_owned(),
            }],
        };
        let build = paths.runtime_root("master-721/8caa3f9", &asset.name);
        assert!(build.ends_with(
            "stable-diffusion.cpp/master-721_8caa3f9/sd-master-bin-Linux-x86_64-cuda.zip"
        ));
        write_runtime_manifest(&build, &asset).expect("manifest");
        let archive = root.join("a.zip");
        std::fs::write(
            &archive,
            zip_with(&[
                (server_executable_name(), b"server"),
                ("lib/libsd.so", b"lib"),
                ("../escape", b"no"),
            ]),
        )
        .expect("archive");
        extract_runtime_archive(&archive, &asset.name, &build).expect("extract");
        assert!(
            !root
                .join("runtimes")
                .join("stable-diffusion.cpp")
                .join("master-721_8caa3f9")
                .join("escape")
                .exists()
        );
        assert!(build.join("lib/libsd.so").is_file());
        assert!(!runtime_root_is_complete(&build));
        let dependency = root.join("b.zip");
        std::fs::write(&dependency, zip_with(&[("cudart.so", b"rt")])).expect("dependency");
        extract_runtime_archive(&dependency, "cudart-linux.zip", &build).expect("extract");
        assert!(runtime_root_is_complete(&build));
        let installed = installed_runtimes(&paths, RuntimePlatform::LinuxX64);
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].backend, "cuda");
        assert_eq!(installed[0].release, "master-721_8caa3f9");
        assert_eq!(
            effective_active_runtime(&paths, &installed),
            Some(ActiveRuntime {
                release: "master-721_8caa3f9".to_owned(),
                asset: asset.name.clone(),
            })
        );
        assert_eq!(saved_active_runtime(&paths), None);
        save_active_runtime(
            &paths,
            &ActiveRuntime {
                release: "master-721_8caa3f9".to_owned(),
                asset: asset.name.clone(),
            },
        )
        .expect("save");
        assert!(saved_active_runtime(&paths).is_some());
        clear_active_runtime(&paths).expect("clear");
        clear_active_runtime(&paths).expect("clear twice");
        assert_eq!(
            load_compute_policy(&paths, "master-721/8caa3f9", &asset.name),
            ComputePolicy::default()
        );
        let policy = ComputePolicy {
            single_gpu_device_id: Some(1),
            ..ComputePolicy::default()
        };
        save_compute_policy(&paths, "master-721/8caa3f9", &asset.name, &policy).expect("policy");
        assert_eq!(
            load_compute_policy(&paths, "master-721/8caa3f9", &asset.name),
            policy
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn components_are_content_addressed_below_the_image_folder() {
        let (_, paths) = paths();
        let catalog = crate::diffusion_catalog();
        let vae = &catalog
            .profile("z-image")
            .expect("z-image")
            .shared_components[1];
        assert_eq!(
            DiffusionPaths::component_segments(vae),
            [
                "components",
                "afc8e28272cd15db3919bacdb6918ce9c1ed22e96cb12c4d5ed0fba823529e38",
                "ae.safetensors"
            ]
        );
        assert!(paths.component_path(vae).starts_with(&paths.image_root));
        assert_eq!(safe_path_segment("a b/c:d"), "a_b_c_d");
    }
}
