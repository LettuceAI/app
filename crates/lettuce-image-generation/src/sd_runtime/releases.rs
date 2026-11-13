//! stable-diffusion.cpp engine builds from the upstream GitHub releases,
//! resolved at runtime (no pinned engine version).

use serde::{Deserialize, Serialize};

pub const GITHUB_REPOSITORY: &str = "leejet/stable-diffusion.cpp";

#[derive(Debug, Clone, Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub published_at: Option<String>,
    pub draft: bool,
    pub prerelease: bool,
    pub assets: Vec<GithubAsset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(rename = "size")]
    pub bytes: u64,
    pub digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDependency {
    pub name: String,
    pub bytes: u64,
    pub sha256: Option<String>,
    pub download_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAsset {
    pub name: String,
    pub backend: String,
    pub bytes: u64,
    pub sha256: Option<String>,
    pub download_url: String,
    pub dependencies: Vec<RuntimeDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRelease {
    pub tag: String,
    pub name: String,
    pub published_at: Option<String>,
    pub prerelease: bool,
    /// A short compatibility note shown after the version (e.g. "glibc 2.39"
    /// for the pinned older build). `None` for the GitHub-fetched releases.
    pub note: Option<String>,
    pub assets: Vec<RuntimeAsset>,
}

/// Engine builds we always offer, even once they age out of the GitHub release
/// page we fetch. Kept last in the list so the newest builds stay on top. Linux
/// x86_64 only: the pinned archive is an Ubuntu 24.04 Vulkan build (glibc 2.39)
/// for hosts whose system libraries are older than the current builds need.
#[must_use]
pub fn pinned_runtime_releases(platform: RuntimePlatform) -> Vec<RuntimeRelease> {
    if platform != RuntimePlatform::LinuxX64 {
        return Vec::new();
    }
    let tag = "master-1043-3be2378";
    let asset = "sd-master-3be2378-bin-Linux-Ubuntu-24.04-x86_64-vulkan.zip";
    vec![RuntimeRelease {
        tag: tag.to_owned(),
        name: tag.to_owned(),
        published_at: Some("2026-11-02T19:26:20Z".to_owned()),
        prerelease: false,
        note: Some("glibc 2.39".to_owned()),
        assets: vec![RuntimeAsset {
            name: asset.to_owned(),
            backend: "vulkan".to_owned(),
            bytes: 37_675_781,
            sha256: Some(
                "7608901016fb4a348bc5b75c46fc971085dadadf44e6f30daa97301844b1ce2e".to_owned(),
            ),
            download_url: format!(
                "https://github.com/{GITHUB_REPOSITORY}/releases/download/{tag}/{asset}"
            ),
            dependencies: Vec::new(),
        }],
    }]
}

/// The platforms engine builds are offered for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePlatform {
    LinuxX64,
    WindowsX64,
    MacosArm64,
    Unsupported,
}

impl RuntimePlatform {
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            Self::LinuxX64
        } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            Self::WindowsX64
        } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Self::MacosArm64
        } else {
            Self::Unsupported
        }
    }
}

/// The backend of an engine archive for a platform, or `None` when the
/// archive does not run there.
#[must_use]
pub fn runtime_backend(name: &str, platform: RuntimePlatform) -> Option<&'static str> {
    let lower = name.to_ascii_lowercase();
    if !lower.ends_with(".zip") || lower.starts_with("cudart-") {
        return None;
    }
    match platform {
        RuntimePlatform::LinuxX64 if lower.contains("linux") && lower.contains("x86_64") => {
            Some(if lower.contains("vulkan") {
                "vulkan"
            } else if lower.contains("rocm") {
                "rocm"
            } else if lower.contains("cuda") {
                "cuda"
            } else {
                "cpu"
            })
        }
        RuntimePlatform::WindowsX64 if lower.contains("bin-win") && lower.contains("x64") => {
            if lower.contains("vulkan") {
                Some("vulkan")
            } else if lower.contains("rocm") {
                Some("rocm")
            } else if lower.contains("cuda") {
                Some("cuda")
            } else if lower.contains("cpu") {
                Some("cpu")
            } else {
                None
            }
        }
        RuntimePlatform::MacosArm64 if lower.contains("darwin") && lower.contains("arm64") => {
            Some("metal")
        }
        _ => None,
    }
}

/// The message shown when an engine build cannot load because the system's C
/// library is older than the build was compiled against.
pub const RUNTIME_LIBRARY_INCOMPATIBLE_MESSAGE: &str = "This image engine build needs newer system libraries (glibc) than this computer provides. Update the host or container base image to a newer OS, or install an engine build that matches this system.";

/// Recognises the dynamic-loader failures that mean an engine build targets a
/// newer or different system library than the host provides — e.g. a build
/// compiled for a newer Ubuntu/glibc than the machine (or container) runs. The
/// linker prints these in English before `main`, so matching the stable markers
/// lets us explain the mismatch instead of surfacing the raw loader error.
#[must_use]
pub fn is_runtime_library_incompatibility(detail: &str) -> bool {
    let lower = detail.to_ascii_lowercase();
    lower.contains("glibc_")
        || lower.contains("error while loading shared libraries")
        || lower.contains("cannot open shared object file")
        || (lower.contains("version `") && lower.contains("not found"))
}

fn digest_sha256(digest: Option<&str>) -> Option<String> {
    digest
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .map(str::to_owned)
}

/// Non-draft releases with at least one archive for the platform. CUDA
/// archives depend on the release's `cudart-*` archives.
#[must_use]
pub fn runtime_releases(
    releases: Vec<GithubRelease>,
    platform: RuntimePlatform,
) -> Vec<RuntimeRelease> {
    releases
        .into_iter()
        .filter(|release| !release.draft)
        .filter_map(|release| {
            let assets = release
                .assets
                .iter()
                .filter_map(|asset| {
                    let backend = runtime_backend(&asset.name, platform)?;
                    let dependencies = if backend == "cuda" {
                        release
                            .assets
                            .iter()
                            .filter(|candidate| {
                                candidate.name.to_ascii_lowercase().starts_with("cudart-")
                            })
                            .map(|candidate| RuntimeDependency {
                                name: candidate.name.clone(),
                                bytes: candidate.bytes,
                                sha256: digest_sha256(candidate.digest.as_deref()),
                                download_url: candidate.browser_download_url.clone(),
                            })
                            .collect()
                    } else {
                        Vec::new()
                    };
                    Some(RuntimeAsset {
                        name: asset.name.clone(),
                        backend: backend.to_owned(),
                        bytes: asset.bytes,
                        sha256: digest_sha256(asset.digest.as_deref()),
                        download_url: asset.browser_download_url.clone(),
                        dependencies,
                    })
                })
                .collect::<Vec<_>>();
            (!assets.is_empty()).then(|| RuntimeRelease {
                name: release.name.unwrap_or_else(|| release.tag_name.clone()),
                tag: release.tag_name,
                published_at: release.published_at,
                prerelease: release.prerelease,
                note: None,
                assets,
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeSelectionError {
    #[error("stable-diffusion.cpp release is no longer available: {0}")]
    ReleaseGone(String),
    #[error("Runtime asset {asset} is not available for this platform in {release}")]
    AssetUnavailable { asset: String, release: String },
}

/// The archive a user picked, re-resolved against the current release list.
pub fn select_runtime<'a>(
    releases: &'a [RuntimeRelease],
    release_tag: &str,
    asset_name: &str,
) -> Result<(&'a RuntimeRelease, &'a RuntimeAsset), RuntimeSelectionError> {
    let release = releases
        .iter()
        .find(|release| release.tag == release_tag)
        .ok_or_else(|| RuntimeSelectionError::ReleaseGone(release_tag.to_owned()))?;
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == asset_name)
        .ok_or_else(|| RuntimeSelectionError::AssetUnavailable {
            asset: asset_name.to_owned(),
            release: release_tag.to_owned(),
        })?;
    Ok((release, asset))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str, digest: Option<&str>) -> GithubAsset {
        GithubAsset {
            name: name.to_owned(),
            browser_download_url: format!("https://github.com/download/{name}"),
            bytes: 100,
            digest: digest.map(str::to_owned),
        }
    }

    #[test]
    fn platform_archives_pick_their_backend_like_legacy() {
        let linux = RuntimePlatform::LinuxX64;
        assert_eq!(
            runtime_backend(
                "sd-master-721-bin-Linux-Ubuntu-24.04-x86_64-vulkan.zip",
                linux
            ),
            Some("vulkan")
        );
        assert_eq!(
            runtime_backend("sd-master-bin-Linux-x86_64-rocm.zip", linux),
            Some("rocm")
        );
        assert_eq!(runtime_backend("sd-linux-x86_64.zip", linux), Some("cpu"));
        assert_eq!(runtime_backend("sd-linux-x86_64.tar.gz", linux), None);
        assert_eq!(runtime_backend("cudart-linux-x86_64.zip", linux), None);
        let windows = RuntimePlatform::WindowsX64;
        assert_eq!(
            runtime_backend("sd-master-bin-win-cuda12-x64.zip", windows),
            Some("cuda")
        );
        assert_eq!(
            runtime_backend("sd-master-bin-win-avx2-x64.zip", windows),
            None
        );
        assert_eq!(
            runtime_backend(
                "sd-master-bin-Darwin-macOS-arm64.zip",
                RuntimePlatform::MacosArm64
            ),
            Some("metal")
        );
        assert_eq!(
            runtime_backend(
                "sd-master-bin-win-cpu-x64.zip",
                RuntimePlatform::Unsupported
            ),
            None
        );
    }

    #[test]
    fn releases_keep_platform_archives_and_cuda_runtime_dependencies() {
        let releases = runtime_releases(
            vec![
                GithubRelease {
                    tag_name: "master-721-8caa3f9".to_owned(),
                    name: None,
                    published_at: Some("2026-01-01T00:00:00Z".to_owned()),
                    draft: false,
                    prerelease: true,
                    assets: vec![
                        asset("sd-master-bin-win-cuda12-x64.zip", Some("sha256:abc")),
                        asset("cudart-sd-bin-win-cu12-x64.zip", Some("sha256:def")),
                        asset("sd-master-bin-win-vulkan-x64.zip", None),
                    ],
                },
                GithubRelease {
                    tag_name: "draft".to_owned(),
                    name: None,
                    published_at: None,
                    draft: true,
                    prerelease: false,
                    assets: vec![asset("sd-master-bin-win-cpu-x64.zip", None)],
                },
                GithubRelease {
                    tag_name: "linux-only".to_owned(),
                    name: None,
                    published_at: None,
                    draft: false,
                    prerelease: false,
                    assets: vec![asset("sd-linux-x86_64.zip", None)],
                },
            ],
            RuntimePlatform::WindowsX64,
        );
        assert_eq!(releases.len(), 1);
        let release = &releases[0];
        assert_eq!(release.name, "master-721-8caa3f9");
        assert!(release.prerelease);
        assert_eq!(release.assets.len(), 2);
        assert_eq!(release.assets[0].backend, "cuda");
        assert_eq!(release.assets[0].sha256.as_deref(), Some("abc"));
        assert_eq!(release.assets[0].dependencies.len(), 1);
        assert_eq!(
            release.assets[0].dependencies[0].sha256.as_deref(),
            Some("def")
        );
        assert!(release.assets[1].dependencies.is_empty());
        assert!(
            select_runtime(
                &releases,
                "master-721-8caa3f9",
                "sd-master-bin-win-vulkan-x64.zip"
            )
            .is_ok()
        );
        assert_eq!(
            select_runtime(&releases, "gone", "x").map_err(|error| error.to_string()),
            Err("stable-diffusion.cpp release is no longer available: gone".to_owned())
        );
    }

    #[test]
    fn pinned_release_offers_the_ubuntu_2404_vulkan_build_on_linux_only() {
        let linux = pinned_runtime_releases(RuntimePlatform::LinuxX64);
        assert_eq!(linux.len(), 1);
        let release = &linux[0];
        assert_eq!(release.tag, "master-1043-3be2378");
        assert_eq!(release.note.as_deref(), Some("glibc 2.39"));
        assert!(!release.prerelease);
        assert_eq!(release.assets.len(), 1);
        let asset = &release.assets[0];
        assert_eq!(asset.backend, "vulkan");
        assert_eq!(
            asset.name,
            "sd-master-3be2378-bin-Linux-Ubuntu-24.04-x86_64-vulkan.zip"
        );
        assert_eq!(
            asset.download_url,
            "https://github.com/leejet/stable-diffusion.cpp/releases/download/master-1043-3be2378/sd-master-3be2378-bin-Linux-Ubuntu-24.04-x86_64-vulkan.zip"
        );
        // The pinned archive stays installable: it resolves and its backend is
        // recognised for the platform it is offered on.
        assert_eq!(runtime_backend(&asset.name, RuntimePlatform::LinuxX64), Some("vulkan"));
        assert!(select_runtime(&linux, &release.tag, &asset.name).is_ok());

        assert!(pinned_runtime_releases(RuntimePlatform::WindowsX64).is_empty());
        assert!(pinned_runtime_releases(RuntimePlatform::MacosArm64).is_empty());
    }
}
