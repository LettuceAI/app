use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::process::Child;
use tokio::sync::Mutex;

use crate::hf_browser::QueueDownloadMetadata;

const PROVIDER_ID: &str = "sdcpp";
const PROVIDER_LABEL: &str = "Local Image Generation";
const GITHUB_REPOSITORY: &str = "leejet/stable-diffusion.cpp";

struct ManagedServer {
    key: String,
    base_url: String,
    child: Child,
}

lazy_static::lazy_static! {
    static ref MANAGED_SERVER: Mutex<Option<ManagedServer>> = Mutex::new(None);
}

#[derive(Clone, Copy)]
struct ComponentSpec {
    role: &'static str,
    repo: &'static str,
    revision: &'static str,
    filename: &'static str,
    bytes: u64,
    sha256: &'static str,
}

#[derive(Clone, Copy)]
struct VariantSpec {
    id: &'static str,
    label: &'static str,
    description: &'static str,
    diffusion: ComponentSpec,
}

#[derive(Clone, Copy)]
struct ProfileSpec {
    id: &'static str,
    display_name: &'static str,
    description: &'static str,
    license: &'static str,
    source_url: &'static str,
    supports_text_to_image: bool,
    supports_image_edit: bool,
    max_reference_images: Option<u8>,
    requires_reference_image: bool,
    recommended_for_scenes: bool,
    default_width: u32,
    default_height: u32,
    default_steps: u16,
    default_cfg: f32,
    variants: &'static [VariantSpec],
    shared_components: &'static [ComponentSpec],
}

const Z_ENCODER: ComponentSpec = ComponentSpec {
    role: "text_encoder",
    repo: "unsloth/Qwen3-4B-Instruct-2507-GGUF",
    revision: "a06e946bb6b655725eafa393f4a9745d460374c9",
    filename: "Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
    bytes: 2_497_281_120,
    sha256: "3605803b982cb64aead44f6c1b2ae36e3acdb41d8e46c8a94c6533bc4c67e597",
};
const Z_VAE: ComponentSpec = ComponentSpec {
    role: "vae",
    repo: "black-forest-labs/FLUX.1-schnell",
    revision: "741f7c3ce8b383c54771c7003378a50191e9efe9",
    filename: "ae.safetensors",
    bytes: 335_304_388,
    sha256: "afc8e28272cd15db3919bacdb6918ce9c1ed22e96cb12c4d5ed0fba823529e38",
};
const Z_SHARED: &[ComponentSpec] = &[Z_ENCODER, Z_VAE];

const Z_TURBO_VARIANTS: &[VariantSpec] = &[
    VariantSpec {
        id: "q3-k",
        label: "Q3 K (smaller)",
        description: "Lower memory use with a modest quality tradeoff.",
        diffusion: ComponentSpec {
            role: "diffusion_model",
            repo: "leejet/Z-Image-Turbo-GGUF",
            revision: "c61c0e422dc8b541b7548cf33a4ef8302b0f8085",
            filename: "z_image_turbo-Q3_K.gguf",
            bytes: 3_143_559_104,
            sha256: "4b44bdaa7814f20d7cf144e3939bd93aa32f50660204dd0c2aea5c5376232980",
        },
    },
    VariantSpec {
        id: "q4-k",
        label: "Q4 K (recommended)",
        description: "Best default balance of image quality, speed, and memory.",
        diffusion: ComponentSpec {
            role: "diffusion_model",
            repo: "leejet/Z-Image-Turbo-GGUF",
            revision: "c61c0e422dc8b541b7548cf33a4ef8302b0f8085",
            filename: "z_image_turbo-Q4_K.gguf",
            bytes: 3_864_250_304,
            sha256: "14b375ab4f226bc5378f68f37e899ef3c2242b8541e61e2bc1aff40976086fbd",
        },
    },
];

const Z_BASE_VARIANTS: &[VariantSpec] = &[
    VariantSpec {
        id: "q3-k-m",
        label: "Q3 K M (smaller)",
        description: "Reduced memory use for systems that cannot fit Q4 K M.",
        diffusion: ComponentSpec {
            role: "diffusion_model",
            repo: "unsloth/Z-Image-GGUF",
            revision: "c9913e69743c5d9dfa7fdac58a0cc5709a17aa08",
            filename: "z-image-Q3_K_M.gguf",
            bytes: 4_559_946_816,
            sha256: "e0382d4b1affe9e552392aa9c53a20c2d661b4ddd7f8c56f1f34626c2538368c",
        },
    },
    VariantSpec {
        id: "q4-0",
        label: "Q4 0",
        description: "A compact Q4 option with broad backend compatibility.",
        diffusion: ComponentSpec {
            role: "diffusion_model",
            repo: "unsloth/Z-Image-GGUF",
            revision: "c9913e69743c5d9dfa7fdac58a0cc5709a17aa08",
            filename: "z-image-Q4_0.gguf",
            bytes: 4_585_244_736,
            sha256: "4c5cfc02e6007ae1f0b0d690f68bda452f51f7b8ab6d9f500f8c5b829fea4377",
        },
    },
    VariantSpec {
        id: "q4-k-m",
        label: "Q4 K M (recommended)",
        description: "Higher quality for the full, non-distilled Z-Image model.",
        diffusion: ComponentSpec {
            role: "diffusion_model",
            repo: "unsloth/Z-Image-GGUF",
            revision: "c9913e69743c5d9dfa7fdac58a0cc5709a17aa08",
            filename: "z-image-Q4_K_M.gguf",
            bytes: 5_066_995_776,
            sha256: "a62b929f76553b21f68894e9ed34d24b7fb67fb59b5689fa06981865986cce40",
        },
    },
];

const FLUX_SHARED: &[ComponentSpec] = &[
    ComponentSpec {
        role: "text_encoder",
        repo: "unsloth/Qwen3-4B-GGUF",
        revision: "22c9fc8a8c7700b76a1789366280a6a5a1ad1120",
        filename: "Qwen3-4B-Q4_K_M.gguf",
        bytes: 2_497_281_312,
        sha256: "f6f851777709861056efcdad3af01da38b31223a3ba26e61a4f8bf3a2195813a",
    },
    ComponentSpec {
        role: "vae",
        repo: "Comfy-Org/flux2-dev",
        revision: "03d6521e6f6a47396b3f951cbea50f7e6c2f482e",
        filename: "split_files/vae/flux2-vae.safetensors",
        bytes: 336_213_556,
        sha256: "d64f3a68e1cc4f9f4e29b6e0da38a0204fe9a49f2d4053f0ec1fa1ca02f9c4b5",
    },
];
const FLUX_VARIANTS: &[VariantSpec] = &[VariantSpec {
    id: "q4-0",
    label: "Q4 0 (recommended)",
    description: "Fast four-step generation and multi-reference scene composition.",
    diffusion: ComponentSpec {
        role: "diffusion_model",
        repo: "leejet/FLUX.2-klein-4B-GGUF",
        revision: "3b1f5a9dc3abb32238b053aeb3d823c30afdacbd",
        filename: "flux-2-klein-4b-Q4_0.gguf",
        bytes: 2_460_378_560,
        sha256: "d1023499ef3f2f82ff7c50e6778495195c1b6cc34835741778868428111f9ff4",
    },
}];

const QWEN_SHARED: &[ComponentSpec] = &[
    ComponentSpec {
        role: "text_encoder",
        repo: "unsloth/Qwen2.5-VL-7B-Instruct-GGUF",
        revision: "68bb8bc4b7df5289c143aaec0ab477a7d4051aab",
        filename: "Qwen2.5-VL-7B-Instruct-Q4_K_M.gguf",
        bytes: 4_683_072_384,
        sha256: "d16776dcd9a28d42758c2958ed3a752aabf20a305252cd64ff2be72b4a78c503",
    },
    ComponentSpec {
        role: "vision_encoder",
        repo: "unsloth/Qwen2.5-VL-7B-Instruct-GGUF",
        revision: "68bb8bc4b7df5289c143aaec0ab477a7d4051aab",
        filename: "mmproj-BF16.gguf",
        bytes: 1_354_163_040,
        sha256: "f0edf43c09b69d6e5dd24262f33b356a1e9dd978e7c3299b3e69141fcbb87553",
    },
    ComponentSpec {
        role: "vae",
        repo: "QuantStack/Qwen-Image-Edit-GGUF",
        revision: "acab6f9f09973bc8a128a1e04e809acb65784e1c",
        filename: "VAE/Qwen_Image-VAE.safetensors",
        bytes: 253_806_246,
        sha256: "a70580f0213e67967ee9c95f05bb400e8fb08307e017a924bf3441223e023d1f",
    },
];
const QWEN_VARIANTS: &[VariantSpec] = &[
    VariantSpec {
        id: "q2-k",
        label: "Q2 K (smaller)",
        description: "The practical option for lower-memory systems.",
        diffusion: ComponentSpec {
            role: "diffusion_model",
            repo: "unsloth/Qwen-Image-Edit-2511-GGUF",
            revision: "0d33d9692b4b26212297240d87b0d4719aa4fd06",
            filename: "qwen-image-edit-2511-Q2_K.gguf",
            bytes: 7_468_022_368,
            sha256: "a3d09042b64657970654941aa08d895de29b4d98edf3632a89e70d4d6e23c47c",
        },
    },
    VariantSpec {
        id: "q3-k-m",
        label: "Q3 K M (recommended)",
        description: "Better edit fidelity when system memory allows it.",
        diffusion: ComponentSpec {
            role: "diffusion_model",
            repo: "unsloth/Qwen-Image-Edit-2511-GGUF",
            revision: "0d33d9692b4b26212297240d87b0d4719aa4fd06",
            filename: "qwen-image-edit-2511-Q3_K_M.gguf",
            bytes: 9_920_805_472,
            sha256: "5631fd3a407880e1fb541dc47696628633c898565136c128d5a2741d4b84e9e9",
        },
    },
];

const PROFILES: &[ProfileSpec] = &[
    ProfileSpec {
        id: "z-image-turbo",
        display_name: "Z-Image Turbo",
        description: "Fast, high-quality text-to-image generation in about eight steps.",
        license: "Apache-2.0",
        source_url: "https://huggingface.co/Tongyi-MAI/Z-Image-Turbo",
        supports_text_to_image: true,
        supports_image_edit: false,
        max_reference_images: Some(0),
        requires_reference_image: false,
        recommended_for_scenes: false,
        default_width: 1024,
        default_height: 1024,
        default_steps: 8,
        default_cfg: 0.0,
        variants: Z_TURBO_VARIANTS,
        shared_components: Z_SHARED,
    },
    ProfileSpec {
        id: "z-image",
        display_name: "Z-Image",
        description: "The full non-distilled model: slower, with stronger prompt following and negative prompts.",
        license: "Apache-2.0",
        source_url: "https://huggingface.co/Tongyi-MAI/Z-Image",
        supports_text_to_image: true,
        supports_image_edit: false,
        max_reference_images: Some(0),
        requires_reference_image: false,
        recommended_for_scenes: false,
        default_width: 1024,
        default_height: 1024,
        default_steps: 40,
        default_cfg: 4.0,
        variants: Z_BASE_VARIANTS,
        shared_components: Z_SHARED,
    },
    ProfileSpec {
        id: "flux-2-klein-4b",
        display_name: "FLUX.2 Klein 4B",
        description: "Fast generation and editing with multi-reference scene composition.",
        license: "Apache-2.0",
        source_url: "https://huggingface.co/black-forest-labs/FLUX.2-klein-4B",
        supports_text_to_image: true,
        supports_image_edit: true,
        max_reference_images: None,
        requires_reference_image: false,
        recommended_for_scenes: true,
        default_width: 1024,
        default_height: 1024,
        default_steps: 4,
        default_cfg: 1.0,
        variants: FLUX_VARIANTS,
        shared_components: FLUX_SHARED,
    },
    ProfileSpec {
        id: "qwen-image-edit-2511",
        display_name: "Qwen Image Edit 2511",
        description: "Identity-preserving image editing and scene composition with multiple references.",
        license: "Apache-2.0",
        source_url: "https://huggingface.co/Qwen/Qwen-Image-Edit-2511",
        supports_text_to_image: false,
        supports_image_edit: true,
        max_reference_images: None,
        requires_reference_image: true,
        recommended_for_scenes: true,
        default_width: 1024,
        default_height: 1024,
        default_steps: 40,
        default_cfg: 4.0,
        variants: QWEN_VARIANTS,
        shared_components: QWEN_SHARED,
    },
];

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    name: Option<String>,
    published_at: Option<String>,
    draft: bool,
    prerelease: bool,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    #[serde(rename = "size")]
    bytes: u64,
    digest: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRelease {
    tag: String,
    name: String,
    published_at: Option<String>,
    prerelease: bool,
    assets: Vec<RuntimeAsset>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAsset {
    name: String,
    backend: String,
    bytes: u64,
    sha256: Option<String>,
    dependencies: Vec<RuntimeDependency>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDependency {
    name: String,
    bytes: u64,
    sha256: Option<String>,
}

#[derive(Clone)]
struct SelectedRuntime {
    release: String,
    asset_name: String,
    bytes: u64,
    sha256: Option<String>,
    download_url: String,
    dependencies: Vec<SelectedRuntimeDependency>,
}

#[derive(Clone)]
struct SelectedRuntimeDependency {
    name: String,
    bytes: u64,
    sha256: Option<String>,
    download_url: String,
}

#[derive(Serialize, Deserialize)]
struct RuntimeManifest {
    archives: Vec<String>,
}

async fn fetch_runtime_releases() -> Result<Vec<(GithubRelease, Vec<RuntimeAsset>)>, String> {
    let url = format!(
        "https://api.github.com/repos/{}/releases?per_page=20",
        GITHUB_REPOSITORY
    );
    let client = reqwest::Client::builder()
        .user_agent("LettuceAI/1.0")
        .build()
        .map_err(|e| format!("Failed to create GitHub release client: {}", e))?;
    let response = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch stable-diffusion.cpp releases: {}", e))?;
    if !response.status().is_success() {
        return Err(format!(
            "GitHub release lookup failed with status {}",
            response.status()
        ));
    }
    let releases = response
        .json::<Vec<GithubRelease>>()
        .await
        .map_err(|e| format!("Failed to parse stable-diffusion.cpp releases: {}", e))?;
    Ok(releases
        .into_iter()
        .filter(|release| !release.draft)
        .filter_map(|release| {
            let assets = release
                .assets
                .iter()
                .filter_map(|asset| {
                    runtime_backend_for_current_platform(&asset.name).map(|backend| {
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
                                    sha256: candidate
                                        .digest
                                        .as_deref()
                                        .and_then(|digest| digest.strip_prefix("sha256:"))
                                        .map(str::to_string),
                                })
                                .collect()
                        } else {
                            Vec::new()
                        };
                        RuntimeAsset {
                            name: asset.name.clone(),
                            backend,
                            bytes: asset.bytes,
                            sha256: asset
                                .digest
                                .as_deref()
                                .and_then(|digest| digest.strip_prefix("sha256:"))
                                .map(str::to_string),
                            dependencies,
                        }
                    })
                })
                .collect::<Vec<_>>();
            (!assets.is_empty()).then_some((release, assets))
        })
        .collect())
}

async fn resolve_runtime_selection(
    release_tag: &str,
    asset_name: &str,
) -> Result<SelectedRuntime, String> {
    let releases = fetch_runtime_releases().await?;
    let (release, assets) = releases
        .into_iter()
        .find(|(release, _)| release.tag_name == release_tag)
        .ok_or_else(|| {
            format!(
                "stable-diffusion.cpp release is no longer available: {}",
                release_tag
            )
        })?;
    let listed_asset = assets
        .into_iter()
        .find(|asset| asset.name == asset_name)
        .ok_or_else(|| {
            format!(
                "Runtime asset {} is not available for this platform in {}",
                asset_name, release_tag
            )
        })?;
    let source_asset = release
        .assets
        .iter()
        .find(|asset| asset.name == asset_name)
        .ok_or_else(|| "Selected runtime asset disappeared from the GitHub response".to_string())?;
    let dependencies = listed_asset
        .dependencies
        .iter()
        .map(|dependency| {
            let source = release
                .assets
                .iter()
                .find(|asset| asset.name == dependency.name)
                .ok_or_else(|| {
                    format!(
                        "Runtime dependency disappeared from the GitHub response: {}",
                        dependency.name
                    )
                })?;
            Ok(SelectedRuntimeDependency {
                name: dependency.name.clone(),
                bytes: dependency.bytes,
                sha256: dependency.sha256.clone(),
                download_url: source.browser_download_url.clone(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(SelectedRuntime {
        release: release.tag_name,
        asset_name: listed_asset.name,
        bytes: listed_asset.bytes,
        sha256: listed_asset.sha256,
        download_url: source_asset.browser_download_url.clone(),
        dependencies,
    })
}

fn runtime_backend_for_current_platform(name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    if !lower.ends_with(".zip") || lower.starts_with("cudart-") {
        return None;
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    if lower.contains("linux") && lower.contains("x86_64") {
        return Some(
            if lower.contains("vulkan") {
                "vulkan"
            } else if lower.contains("rocm") {
                "rocm"
            } else if lower.contains("cuda") {
                "cuda"
            } else {
                "cpu"
            }
            .to_string(),
        );
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    if lower.contains("bin-win") && lower.contains("x64") {
        return Some(
            if lower.contains("vulkan") {
                "vulkan"
            } else if lower.contains("rocm") {
                "rocm"
            } else if lower.contains("cuda") {
                "cuda"
            } else if lower.contains("cpu") {
                "cpu"
            } else {
                return None;
            }
            .to_string(),
        );
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    if lower.contains("darwin") && lower.contains("arm64") {
        return Some("metal".to_string());
    }
    None
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    runtime_supported: bool,
    unsupported_reason: Option<String>,
    runtime_releases: Vec<RuntimeRelease>,
    profiles: Vec<CatalogProfile>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogProfile {
    id: &'static str,
    display_name: &'static str,
    description: &'static str,
    license: &'static str,
    source_url: &'static str,
    supports_text_to_image: bool,
    supports_image_edit: bool,
    supports_lora: bool,
    max_reference_images: Option<u8>,
    requires_reference_image: bool,
    recommended_for_scenes: bool,
    default_width: u32,
    default_height: u32,
    default_steps: u16,
    default_cfg: f32,
    variants: Vec<CatalogVariant>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogVariant {
    id: &'static str,
    label: &'static str,
    description: &'static str,
    download_bytes: u64,
    installed: bool,
}

#[tauri::command]
pub async fn sdcpp_catalog(app: AppHandle) -> Result<Catalog, String> {
    if cfg!(mobile) {
        return Err("Local stable-diffusion.cpp image generation is desktop-only.".to_string());
    }
    let fetched_releases = fetch_runtime_releases().await?;
    let supported = !fetched_releases.is_empty();
    let runtime_releases = fetched_releases
        .into_iter()
        .map(|(release, assets)| RuntimeRelease {
            name: release.name.unwrap_or_else(|| release.tag_name.clone()),
            tag: release.tag_name,
            published_at: release.published_at,
            prerelease: release.prerelease,
            assets,
        })
        .collect::<Vec<_>>();
    let mut profiles = Vec::new();
    for profile in PROFILES {
        let variants = profile
            .variants
            .iter()
            .map(|variant| CatalogVariant {
                id: variant.id,
                label: variant.label,
                description: variant.description,
                download_bytes: all_components(profile, variant)
                    .iter()
                    .map(|component| component.bytes)
                    .sum::<u64>(),
                installed: is_variant_installed(&app, profile, variant, None, None),
            })
            .collect();
        profiles.push(CatalogProfile {
            id: profile.id,
            display_name: profile.display_name,
            description: profile.description,
            license: profile.license,
            source_url: profile.source_url,
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
            variants,
        });
    }
    Ok(Catalog {
        runtime_supported: supported,
        unsupported_reason: (!supported).then(|| {
            "No stable-diffusion.cpp release assets match this operating system and architecture."
                .to_string()
        }),
        runtime_releases,
        profiles,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRequest {
    profile_id: String,
    variant_id: String,
    runtime_release: String,
    runtime_asset: String,
}

#[tauri::command]
pub async fn sdcpp_install(app: AppHandle, request: InstallRequest) -> Result<Vec<String>, String> {
    if cfg!(mobile) {
        return Err("Local stable-diffusion.cpp image generation is desktop-only.".to_string());
    }
    let runtime =
        resolve_runtime_selection(&request.runtime_release, &request.runtime_asset).await?;
    let (profile, variant) = find_profile_variant(&request.profile_id, &request.variant_id)?;
    let install_id = format!(
        "sdcpp:{}:{}:{}:{}",
        profile.id, variant.id, runtime.release, runtime.asset_name
    );
    let image_root = image_root(&app)?;
    let mut queue_ids = Vec::new();
    write_runtime_manifest(&app, &runtime)?;

    let runtime_archive = runtime_archive_path(&app, &runtime)?;
    queue_ids.push(
        crate::hf_browser::hf_queue_download(
            app.clone(),
            GITHUB_REPOSITORY.to_string(),
            runtime.asset_name.clone(),
            Some(QueueDownloadMetadata {
                install_id: Some(install_id.clone()),
                display_name: Some(profile.display_name.to_string()),
                download_role: Some("runtime".to_string()),
                queue_kind: Some("sdcpp".to_string()),
                asset_root: Some(image_root.to_string_lossy().to_string()),
                install_kind: Some(profile.id.to_string()),
                variant: Some(variant.id.to_string()),
                download_url: Some(runtime.download_url.clone()),
                destination_path: Some(runtime_archive.to_string_lossy().to_string()),
                expected_size: Some(runtime.bytes),
                sha256: runtime.sha256.clone(),
                runtime_release: Some(runtime.release.clone()),
                runtime_asset: Some(runtime.asset_name.clone()),
                ..Default::default()
            }),
        )
        .await?,
    );

    for dependency in &runtime.dependencies {
        let destination = runtime_dependency_archive_path(&app, &runtime, dependency)?;
        queue_ids.push(
            crate::hf_browser::hf_queue_download(
                app.clone(),
                GITHUB_REPOSITORY.to_string(),
                dependency.name.clone(),
                Some(QueueDownloadMetadata {
                    install_id: Some(install_id.clone()),
                    display_name: Some(profile.display_name.to_string()),
                    download_role: Some("runtime_dependency".to_string()),
                    queue_kind: Some("sdcpp".to_string()),
                    asset_root: Some(image_root.to_string_lossy().to_string()),
                    install_kind: Some(profile.id.to_string()),
                    variant: Some(variant.id.to_string()),
                    download_url: Some(dependency.download_url.clone()),
                    destination_path: Some(destination.to_string_lossy().to_string()),
                    expected_size: Some(dependency.bytes),
                    sha256: dependency.sha256.clone(),
                    runtime_release: Some(runtime.release.clone()),
                    runtime_asset: Some(runtime.asset_name.clone()),
                    ..Default::default()
                }),
            )
            .await?,
        );
    }

    for component in all_components(profile, variant) {
        let destination = component_path(&app, component)?;
        let url = format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            component.repo, component.revision, component.filename
        );
        queue_ids.push(
            crate::hf_browser::hf_queue_download(
                app.clone(),
                component.repo.to_string(),
                component.filename.to_string(),
                Some(QueueDownloadMetadata {
                    install_id: Some(install_id.clone()),
                    display_name: Some(profile.display_name.to_string()),
                    download_role: Some(component.role.to_string()),
                    queue_kind: Some("sdcpp".to_string()),
                    asset_root: Some(image_root.to_string_lossy().to_string()),
                    install_kind: Some(profile.id.to_string()),
                    variant: Some(variant.id.to_string()),
                    download_url: Some(url),
                    destination_path: Some(destination.to_string_lossy().to_string()),
                    expected_size: Some(component.bytes),
                    sha256: Some(component.sha256.to_string()),
                    runtime_release: Some(runtime.release.clone()),
                    runtime_asset: Some(runtime.asset_name.clone()),
                    ..Default::default()
                }),
            )
            .await?,
        );
    }
    Ok(queue_ids)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunnabilityRequest {
    profile_id: String,
    variant_id: String,
    runtime_release: String,
    runtime_asset: String,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    reference_image_count: Option<u8>,
    #[serde(default)]
    loras: Vec<super::types::ImageLora>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Runnability {
    status: String,
    method: &'static str,
    exact: bool,
    elapsed_ms: Option<u64>,
    reason: String,
}

#[tauri::command]
pub async fn sdcpp_runnability(
    app: AppHandle,
    request: RunnabilityRequest,
) -> Result<Runnability, String> {
    if cfg!(mobile) {
        return Err("Local stable-diffusion.cpp image generation is desktop-only.".to_string());
    }
    let (profile, variant) = find_profile_variant(&request.profile_id, &request.variant_id)?;
    let refs = request.reference_image_count.unwrap_or(0);
    if let Some(maximum) = profile.max_reference_images {
        if refs > maximum {
            return Err(format!(
                "{} accepts at most {} reference images.",
                profile.display_name, maximum
            ));
        }
    }
    if profile.requires_reference_image && refs == 0 {
        return Err(format!(
            "{} requires at least one reference image.",
            profile.display_name
        ));
    }
    let (width, height) = (
        request.width.unwrap_or(profile.default_width),
        request.height.unwrap_or(profile.default_height),
    );
    let installed = is_variant_installed(
        &app,
        profile,
        variant,
        Some(&request.runtime_release),
        Some(&request.runtime_asset),
    );
    if !installed {
        return Ok(Runnability {
            status: "notInstalled".to_string(),
            method: "stableDiffusionCppRuntimeProbe",
            exact: false,
            elapsed_ms: None,
            reason: "Install the selected model and runtime before running the exact fit test. No file-size or VRAM formula was used."
                .to_string(),
        });
    }

    let config = InstalledModelConfig {
        sdcpp_profile_id: profile.id.to_string(),
        sdcpp_variant_id: variant.id.to_string(),
        sdcpp_runtime_release: request.runtime_release,
        sdcpp_runtime_asset: request.runtime_asset,
    };
    let started = Instant::now();
    let base_url = match ensure_server(&app, &config, profile, variant).await {
        Ok(base_url) => base_url,
        Err(error) => {
            return Ok(Runnability {
                status: "failed".to_string(),
                method: "stableDiffusionCppRuntimeProbe",
                exact: true,
                elapsed_ms: Some(started.elapsed().as_millis() as u64),
                reason: error,
            });
        }
    };
    let reference = blank_reference_data_url(width, height)?;
    let loras = normalize_loras(&app, &request.loras)?;
    let payload = serde_json::json!({
        "prompt": "runnability probe",
        "negative_prompt": "",
        "width": width,
        "height": height,
        "seed": 1,
        "batch_count": 1,
        "auto_resize_ref_image": true,
        "ref_images": vec![reference; refs as usize],
        "sample_params": {
            "scheduler": "discrete",
            "sample_method": "euler",
            "sample_steps": 1,
            "guidance": {
                "txt_cfg": profile.default_cfg,
                "img_cfg": profile.default_cfg,
                "distilled_guidance": 0.0
            }
        },
        "lora": loras,
        "vae_tiling_params": { "enabled": true },
        "output_format": "png",
        "output_compression": 1
    });
    let result = run_probe_job(&base_url, payload).await;
    Ok(match result {
        Ok(()) => Runnability {
            status: "passed".to_string(),
            method: "stableDiffusionCppRuntimeProbe",
            exact: true,
            elapsed_ms: Some(started.elapsed().as_millis() as u64),
            reason: "stable-diffusion.cpp loaded the selected components and completed a one-step graph execution at the requested resolution with the requested reference count and LoRAs."
                .to_string(),
        },
        Err(error) => Runnability {
            status: "failed".to_string(),
            method: "stableDiffusionCppRuntimeProbe",
            exact: true,
            elapsed_ms: Some(started.elapsed().as_millis() as u64),
            reason: error,
        },
    })
}

fn blank_reference_data_url(width: u32, height: u32) -> Result<String, String> {
    let image = image::DynamicImage::new_rgb8(width, height);
    let mut bytes = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to create probe reference image: {}", e))?;
    Ok(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(bytes.into_inner())
    ))
}

async fn run_probe_job(base_url: &str, payload: Value) -> Result<(), String> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/sdcpp/v1/img_gen", base_url))
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Failed to submit stable-diffusion.cpp fit test: {}", e))?;
    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await.unwrap_or_default();
        return Err(format!("Fit test was rejected ({}): {}", status, detail));
    }
    let accepted = response
        .json::<Value>()
        .await
        .map_err(|e| format!("Failed to parse fit-test response: {}", e))?;
    let poll_path = accepted
        .get("poll_url")
        .and_then(Value::as_str)
        .ok_or_else(|| "Fit-test response did not include a poll URL".to_string())?;
    let poll_url = if poll_path.starts_with("http://") || poll_path.starts_with("https://") {
        poll_path.to_string()
    } else {
        format!("{}{}", base_url, poll_path)
    };
    for _ in 0..1_200 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let response = client
            .get(&poll_url)
            .send()
            .await
            .map_err(|e| format!("Failed to poll stable-diffusion.cpp fit test: {}", e))?;
        let job = response
            .json::<Value>()
            .await
            .map_err(|e| format!("Failed to parse stable-diffusion.cpp fit test: {}", e))?;
        match job.get("status").and_then(Value::as_str) {
            Some("queued") | Some("generating") | Some("running") => continue,
            Some("completed") => return Ok(()),
            Some("failed") | Some("cancelled") => {
                return Err(job
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("stable-diffusion.cpp fit test failed")
                    .to_string());
            }
            status => return Err(format!("Unknown fit-test job status: {:?}", status)),
        }
    }
    Err("stable-diffusion.cpp fit test timed out after ten minutes".to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledModelConfig {
    sdcpp_profile_id: String,
    sdcpp_variant_id: String,
    sdcpp_runtime_release: String,
    sdcpp_runtime_asset: String,
}

fn installed_model_config(
    app: &AppHandle,
    model_name: &str,
) -> Result<InstalledModelConfig, String> {
    use rusqlite::OptionalExtension;

    let conn = crate::storage_manager::db::open_db(app)?;
    let advanced = conn
        .query_row(
            "SELECT advanced_model_settings FROM models WHERE provider_id = ?1 AND name = ?2 LIMIT 1",
            rusqlite::params![PROVIDER_ID, model_name],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .flatten()
        .ok_or_else(|| format!("Local image model is not installed: {}", model_name))?;
    serde_json::from_str(&advanced)
        .map_err(|e| format!("Local image model has invalid runtime settings: {}", e))
}

fn selected_component_path(
    app: &AppHandle,
    profile: &ProfileSpec,
    variant: &VariantSpec,
    role: &str,
) -> Result<PathBuf, String> {
    all_components(profile, variant)
        .into_iter()
        .find(|component| component.role == role)
        .ok_or_else(|| {
            format!(
                "{} does not define a {} component",
                profile.display_name, role
            )
        })
        .and_then(|component| component_path(app, component))
}

async fn find_available_port() -> Result<u16, String> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|e| format!("Failed to reserve a local image server port: {}", e))?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|e| format!("Failed to read the local image server port: {}", e))
}

async fn ensure_server(
    app: &AppHandle,
    config: &InstalledModelConfig,
    profile: &ProfileSpec,
    variant: &VariantSpec,
) -> Result<String, String> {
    let key = format!(
        "{}:{}:{}:{}",
        profile.id, variant.id, config.sdcpp_runtime_release, config.sdcpp_runtime_asset
    );
    let mut managed = MANAGED_SERVER.lock().await;
    if let Some(server) = managed.as_mut() {
        if server.key == key && server.child.try_wait().ok().flatten().is_none() {
            return Ok(server.base_url.clone());
        }
        let _ = server.child.kill().await;
        let _ = server.child.wait().await;
        *managed = None;
    }

    crate::llama_cpp::llamacpp_unload(app.clone()).await?;

    let executable = runtime_executable(
        app,
        &config.sdcpp_runtime_release,
        &config.sdcpp_runtime_asset,
    )?;
    if !runtime_is_installed(
        app,
        &config.sdcpp_runtime_release,
        &config.sdcpp_runtime_asset,
    ) {
        return Err(format!(
            "The selected stable-diffusion.cpp runtime is not fully installed: {}",
            executable.display()
        ));
    }
    let diffusion = selected_component_path(app, profile, variant, "diffusion_model")?;
    let text_encoder = selected_component_path(app, profile, variant, "text_encoder")?;
    let vae = selected_component_path(app, profile, variant, "vae")?;
    let vision_encoder = all_components(profile, variant)
        .into_iter()
        .find(|component| component.role == "vision_encoder")
        .map(|component| component_path(app, component))
        .transpose()?;
    for path in [&diffusion, &text_encoder, &vae] {
        if !path.is_file() {
            return Err(format!(
                "Local image component is missing: {}",
                path.display()
            ));
        }
    }

    let port = find_available_port().await?;
    let runtime_dir = runtime_root(
        app,
        &config.sdcpp_runtime_release,
        &config.sdcpp_runtime_asset,
    )?;
    let lora_dir = lora_root(app)?;
    std::fs::create_dir_all(&lora_dir)
        .map_err(|e| format!("Failed to create the local LoRA library: {}", e))?;
    let mut command = tokio::process::Command::new(&executable);
    command
        .current_dir(&runtime_dir)
        .arg("--diffusion-model")
        .arg(&diffusion)
        .arg("--llm")
        .arg(&text_encoder)
        .arg("--vae")
        .arg(&vae)
        .arg("--lora-model-dir")
        .arg(&lora_dir)
        .arg("--listen-ip")
        .arg("127.0.0.1")
        .arg("--listen-port")
        .arg(port.to_string())
        .arg("--auto-fit")
        .arg("--diffusion-fa")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    if let Some(vision_encoder) = vision_encoder {
        command.arg("--llm_vision").arg(vision_encoder);
    }
    #[cfg(target_os = "linux")]
    {
        let existing = std::env::var_os("LD_LIBRARY_PATH").unwrap_or_default();
        let mut paths = vec![runtime_dir.clone()];
        paths.extend(std::env::split_paths(&existing));
        let joined = std::env::join_paths(paths)
            .map_err(|e| format!("Failed to configure stable-diffusion.cpp libraries: {}", e))?;
        command.env("LD_LIBRARY_PATH", joined);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Failed to start stable-diffusion.cpp: {}", e))?;
    let base_url = format!("http://127.0.0.1:{}", port);
    let client = reqwest::Client::new();
    let capabilities_url = format!("{}/sdcpp/v1/capabilities", base_url);
    let mut ready = false;
    for _ in 0..300 {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| format!("Failed to inspect stable-diffusion.cpp: {}", e))?
        {
            return Err(format!(
                "stable-diffusion.cpp exited while loading the model ({})",
                status
            ));
        }
        if client
            .get(&capabilities_url)
            .send()
            .await
            .is_ok_and(|response| response.status().is_success())
        {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    if !ready {
        let _ = child.kill().await;
        let _ = child.wait().await;
        return Err("stable-diffusion.cpp did not become ready within five minutes".to_string());
    }
    *managed = Some(ManagedServer {
        key,
        base_url: base_url.clone(),
        child,
    });
    Ok(base_url)
}

pub async fn stop_for_llama() -> Result<(), String> {
    let mut managed = MANAGED_SERVER.lock().await;
    if let Some(server) = managed.as_mut() {
        server
            .child
            .kill()
            .await
            .map_err(|e| format!("Failed to stop stable-diffusion.cpp: {}", e))?;
        let _ = server.child.wait().await;
    }
    *managed = None;
    Ok(())
}

pub async fn generate(
    app: &AppHandle,
    request: &super::types::ImageGenerationRequest,
) -> Result<super::types::ImageGenerationResponse, String> {
    if cfg!(mobile) {
        return Err("Local stable-diffusion.cpp image generation is desktop-only.".to_string());
    }
    let config = installed_model_config(app, &request.model)?;
    let (profile, variant) =
        find_profile_variant(&config.sdcpp_profile_id, &config.sdcpp_variant_id)?;
    let references = request.input_images.clone().unwrap_or_default();
    if let Some(maximum) = profile.max_reference_images {
        if references.len() > maximum as usize {
            return Err(format!(
                "{} accepts at most {} reference images.",
                profile.display_name, maximum
            ));
        }
    }
    if profile.requires_reference_image && references.is_empty() {
        return Err(format!(
            "{} requires at least one reference image.",
            profile.display_name
        ));
    }
    let (width, height) = super::provider_adapter::parse_size_dimensions(
        request.size.as_deref().or_else(|| {
            request
                .advanced_model_settings
                .as_ref()
                .and_then(|settings| settings.sd_size.as_deref())
        }),
        profile.default_width,
        profile.default_height,
    );
    let settings = request.advanced_model_settings.as_ref();
    let steps = settings
        .and_then(|settings| settings.sd_steps)
        .unwrap_or(profile.default_steps as u32);
    let cfg = settings
        .and_then(|settings| settings.sd_cfg_scale)
        .unwrap_or(profile.default_cfg as f64);
    let negative_prompt = settings
        .and_then(|settings| settings.sd_negative_prompt.clone())
        .unwrap_or_default();
    let seed = settings
        .and_then(|settings| settings.sd_seed)
        .map(i64::from)
        .unwrap_or(-1);
    let sample_method = settings
        .and_then(|settings| settings.sd_sampler.clone())
        .unwrap_or_else(|| "euler".to_string());

    let loras = normalize_loras(app, request.loras.as_deref().unwrap_or_default())?;
    let base_url = ensure_server(app, &config, profile, variant).await?;
    let payload = serde_json::json!({
        "prompt": request.prompt,
        "negative_prompt": negative_prompt,
        "width": width,
        "height": height,
        "seed": seed,
        "batch_count": request.n.unwrap_or(1),
        "auto_resize_ref_image": true,
        "ref_images": references,
        "sample_params": {
            "scheduler": "discrete",
            "sample_method": sample_method,
            "sample_steps": steps,
            "guidance": {
                "txt_cfg": cfg,
                "img_cfg": cfg,
                "distilled_guidance": 0.0
            }
        },
        "lora": loras,
        "vae_tiling_params": { "enabled": true },
        "output_format": "png",
        "output_compression": 100
    });
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/sdcpp/v1/img_gen", base_url))
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Failed to submit local image generation: {}", e))?;
    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await.unwrap_or_default();
        return Err(format!(
            "stable-diffusion.cpp rejected image generation ({}): {}",
            status, detail
        ));
    }
    let accepted = response
        .json::<Value>()
        .await
        .map_err(|e| format!("Failed to parse stable-diffusion.cpp job response: {}", e))?;
    let poll_url = accepted
        .get("poll_url")
        .and_then(Value::as_str)
        .ok_or_else(|| "stable-diffusion.cpp response did not include a poll URL".to_string())?;
    let poll_url = if poll_url.starts_with("http://") || poll_url.starts_with("https://") {
        poll_url.to_string()
    } else {
        format!("{}{}", base_url, poll_url)
    };

    for _ in 0..1_200 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let response = client
            .get(&poll_url)
            .send()
            .await
            .map_err(|e| format!("Failed to poll local image generation: {}", e))?;
        if !response.status().is_success() {
            return Err(format!(
                "stable-diffusion.cpp job polling failed with status {}",
                response.status()
            ));
        }
        let job = response
            .json::<Value>()
            .await
            .map_err(|e| format!("Failed to parse local image job: {}", e))?;
        match job.get("status").and_then(Value::as_str) {
            Some("queued") | Some("generating") | Some("running") => continue,
            Some("completed") => {
                let images = job
                    .pointer("/result/images")
                    .and_then(Value::as_array)
                    .ok_or_else(|| "Local image job completed without images".to_string())?;
                let mut generated = Vec::with_capacity(images.len());
                for image in images {
                    let encoded = image
                        .get("b64_json")
                        .and_then(Value::as_str)
                        .ok_or_else(|| "Local image result is missing image data".to_string())?;
                    let source = format!("data:image/png;base64,{}", encoded);
                    let saved = super::storage::save_image(app, &source).await?;
                    generated.push(super::types::GeneratedImage {
                        asset_id: saved.asset_id,
                        file_path: saved.file_path,
                        mime_type: saved.mime_type,
                        url: None,
                        width: saved.width,
                        height: saved.height,
                        text: None,
                    });
                }
                return Ok(super::types::ImageGenerationResponse {
                    images: generated,
                    model: request.model.clone(),
                    provider_id: PROVIDER_ID.to_string(),
                });
            }
            Some("failed") | Some("cancelled") => {
                let message = job
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("Local image generation failed");
                return Err(message.to_string());
            }
            other => {
                return Err(format!(
                    "Unknown stable-diffusion.cpp job status: {:?}",
                    other
                ))
            }
        }
    }
    Err("Local image generation timed out after ten minutes".to_string())
}

pub async fn handle_download_completed(
    app: &AppHandle,
    item: &crate::hf_browser::QueuedDownload,
    path: &str,
) -> Result<(), String> {
    if cfg!(mobile) {
        return Ok(());
    }
    if item.queue_kind.as_deref() != Some("sdcpp") {
        return Ok(());
    }
    let (Some(runtime_release), Some(runtime_asset)) = (
        item.runtime_release.as_deref(),
        item.runtime_asset.as_deref(),
    ) else {
        return Err("Local image download is missing its runtime selection".to_string());
    };
    if item
        .download_role
        .as_deref()
        .is_some_and(|role| role == "runtime" || role == "runtime_dependency")
    {
        extract_runtime(app, PathBuf::from(path), runtime_release, runtime_asset).await?;
    }
    let (Some(profile_id), Some(variant_id)) =
        (item.install_kind.as_deref(), item.variant.as_deref())
    else {
        return Ok(());
    };
    let (profile, variant) = find_profile_variant(profile_id, variant_id)?;
    if is_variant_installed(
        app,
        profile,
        variant,
        Some(runtime_release),
        Some(runtime_asset),
    ) {
        register_installed_model(app, profile, variant, runtime_release, runtime_asset)?;
    }
    Ok(())
}

fn register_installed_model(
    app: &AppHandle,
    profile: &ProfileSpec,
    variant: &VariantSpec,
    runtime_release: &str,
    runtime_asset: &str,
) -> Result<(), String> {
    use rusqlite::OptionalExtension;

    let model_name = format!("sdcpp:{}:{}", profile.id, variant.id);
    let (credential_id, model_id) = {
        let conn = crate::storage_manager::db::open_db(app)?;
        let credential_id = conn
            .query_row(
                "SELECT id FROM provider_credentials WHERE provider_id = ?1 AND label = ?2 LIMIT 1",
                rusqlite::params![PROVIDER_ID, PROVIDER_LABEL],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let model_id = conn
            .query_row(
                "SELECT id FROM models WHERE provider_id = ?1 AND name = ?2 LIMIT 1",
                rusqlite::params![PROVIDER_ID, &model_name],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        (credential_id, model_id)
    };
    crate::storage_manager::providers::provider_upsert(
        app.clone(),
        serde_json::json!({
            "id": &credential_id,
            "providerId": PROVIDER_ID,
            "label": PROVIDER_LABEL,
            "config": { "managed": true }
        })
        .to_string(),
    )?;
    crate::storage_manager::models::model_upsert(
        app.clone(),
        serde_json::json!({
            "id": model_id,
            "name": model_name,
            "providerId": PROVIDER_ID,
            "providerCredentialId": &credential_id,
            "providerLabel": PROVIDER_LABEL,
            "displayName": format!("{} ({})", profile.display_name, variant.label.replace(" (recommended)", "").replace(" (smaller)", "")),
            "inputScopes": if profile.supports_image_edit { serde_json::json!(["text", "image"]) } else { serde_json::json!(["text"]) },
            "outputScopes": ["image"],
            "advancedModelSettings": {
                "sdcppProfileId": profile.id,
                "sdcppVariantId": variant.id,
                "sdcppRuntimeRelease": runtime_release,
                "sdcppRuntimeAsset": runtime_asset,
                "sdcppRuntimeBackend": runtime_backend_for_current_platform(runtime_asset),
                "sdcppMaxReferenceImages": profile.max_reference_images,
                "sdcppSupportsLora": true
            }
        })
        .to_string(),
    )?;
    Ok(())
}

fn find_profile_variant(
    profile_id: &str,
    variant_id: &str,
) -> Result<(&'static ProfileSpec, &'static VariantSpec), String> {
    let profile = PROFILES
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| format!("Unknown local image model: {}", profile_id))?;
    let variant = profile
        .variants
        .iter()
        .find(|variant| variant.id == variant_id)
        .ok_or_else(|| format!("Unknown {} variant: {}", profile.display_name, variant_id))?;
    Ok((profile, variant))
}

fn all_components(profile: &ProfileSpec, variant: &VariantSpec) -> Vec<ComponentSpec> {
    let mut components = vec![variant.diffusion];
    components.extend_from_slice(profile.shared_components);
    components
}

fn image_root(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(crate::utils::lettuce_dir(app)?.join("models").join("image"))
}

fn lora_root(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(crate::utils::lettuce_dir(app)?.join("models").join("loras"))
}

fn normalize_loras(
    app: &AppHandle,
    loras: &[super::types::ImageLora],
) -> Result<Vec<Value>, String> {
    let root = lora_root(app)?;
    loras
        .iter()
        .map(|lora| {
            let requested = PathBuf::from(&lora.path);
            let relative = if requested.is_absolute() {
                requested.strip_prefix(&root).map_err(|_| {
                    format!(
                        "LoRA must be inside the LettuceAI LoRA library: {}",
                        root.display()
                    )
                })?
            } else {
                requested.as_path()
            };
            if relative.as_os_str().is_empty()
                || relative.components().any(|component| {
                    matches!(
                        component,
                        std::path::Component::ParentDir
                            | std::path::Component::RootDir
                            | std::path::Component::Prefix(_)
                    )
                })
            {
                return Err(format!("Invalid LoRA library path: {}", lora.path));
            }
            let full_path = root.join(relative);
            if !full_path.is_file() {
                return Err(format!(
                    "LoRA is not installed in the LettuceAI library: {}",
                    full_path.display()
                ));
            }
            let relative = relative
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            Ok(serde_json::json!({
                "path": relative,
                "multiplier": lora.multiplier,
                "is_high_noise": lora.is_high_noise
            }))
        })
        .collect()
}

fn component_path(app: &AppHandle, component: ComponentSpec) -> Result<PathBuf, String> {
    let basename = Path::new(component.filename)
        .file_name()
        .ok_or_else(|| format!("Invalid component filename: {}", component.filename))?;
    Ok(image_root(app)?
        .join("components")
        .join(component.sha256)
        .join(basename))
}

fn safe_path_segment(value: &str) -> String {
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

fn runtime_root(app: &AppHandle, release: &str, asset: &str) -> Result<PathBuf, String> {
    Ok(crate::utils::lettuce_dir(app)?
        .join("runtimes")
        .join("stable-diffusion.cpp")
        .join(safe_path_segment(release))
        .join(safe_path_segment(asset)))
}

fn runtime_archive_path(app: &AppHandle, runtime: &SelectedRuntime) -> Result<PathBuf, String> {
    Ok(crate::utils::lettuce_dir(app)?
        .join("downloads")
        .join("sdcpp")
        .join(safe_path_segment(&runtime.release))
        .join(&runtime.asset_name))
}

fn runtime_dependency_archive_path(
    app: &AppHandle,
    runtime: &SelectedRuntime,
    dependency: &SelectedRuntimeDependency,
) -> Result<PathBuf, String> {
    Ok(crate::utils::lettuce_dir(app)?
        .join("downloads")
        .join("sdcpp")
        .join(safe_path_segment(&runtime.release))
        .join(&dependency.name))
}

fn runtime_executable(app: &AppHandle, release: &str, asset: &str) -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    let filename = "sd-server.exe";
    #[cfg(not(target_os = "windows"))]
    let filename = "sd-server";
    Ok(runtime_root(app, release, asset)?.join(filename))
}

fn runtime_manifest_path(root: &Path) -> PathBuf {
    root.join(".lettuce-runtime-manifest.json")
}

fn runtime_archive_marker(root: &Path, archive: &str) -> PathBuf {
    root.join(format!(
        ".lettuce-extracted-{}.complete",
        safe_path_segment(archive)
    ))
}

fn write_runtime_manifest(app: &AppHandle, runtime: &SelectedRuntime) -> Result<(), String> {
    let root = runtime_root(app, &runtime.release, &runtime.asset_name)?;
    std::fs::create_dir_all(&root).map_err(|e| {
        format!(
            "Failed to create stable-diffusion.cpp runtime directory: {}",
            e
        )
    })?;
    let mut archives = vec![runtime.asset_name.clone()];
    archives.extend(
        runtime
            .dependencies
            .iter()
            .map(|dependency| dependency.name.clone()),
    );
    let manifest = serde_json::to_vec_pretty(&RuntimeManifest { archives })
        .map_err(|e| format!("Failed to serialize runtime manifest: {}", e))?;
    std::fs::write(runtime_manifest_path(&root), manifest)
        .map_err(|e| format!("Failed to write runtime manifest: {}", e))
}

fn runtime_root_is_complete(root: &Path) -> bool {
    let executable = root.join(if cfg!(target_os = "windows") {
        "sd-server.exe"
    } else {
        "sd-server"
    });
    if !executable.is_file() {
        return false;
    }
    let manifest_path = runtime_manifest_path(root);
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
                    .all(|archive| runtime_archive_marker(root, archive).is_file())
        })
}

fn runtime_is_installed(app: &AppHandle, release: &str, asset: &str) -> bool {
    runtime_root(app, release, asset).is_ok_and(|root| runtime_root_is_complete(&root))
}

fn is_variant_installed(
    app: &AppHandle,
    profile: &ProfileSpec,
    variant: &VariantSpec,
    runtime_release: Option<&str>,
    runtime_asset: Option<&str>,
) -> bool {
    let runtime_installed = match (runtime_release, runtime_asset) {
        (Some(release), Some(asset)) => runtime_is_installed(app, release, asset),
        _ => has_any_runtime(app),
    };
    if !runtime_installed {
        return false;
    }
    all_components(profile, variant).iter().all(|component| {
        component_path(app, *component)
            .ok()
            .and_then(|path| std::fs::metadata(path).ok())
            .is_some_and(|metadata| metadata.len() == component.bytes)
    })
}

fn has_any_runtime(app: &AppHandle) -> bool {
    let Ok(root) = crate::utils::lettuce_dir(app)
        .map(|path| path.join("runtimes").join("stable-diffusion.cpp"))
    else {
        return false;
    };
    walkdir::WalkDir::new(root)
        .max_depth(4)
        .into_iter()
        .filter_map(Result::ok)
        .any(|entry| {
            entry.file_type().is_file()
                && entry.file_name().to_string_lossy().eq_ignore_ascii_case(
                    if cfg!(target_os = "windows") {
                        "sd-server.exe"
                    } else {
                        "sd-server"
                    },
                )
                && entry.path().parent().is_some_and(runtime_root_is_complete)
        })
}

async fn extract_runtime(
    app: &AppHandle,
    archive_path: PathBuf,
    release: &str,
    asset: &str,
) -> Result<(), String> {
    let destination = runtime_root(app, release, asset)?;
    let archive_name = archive_path
        .file_name()
        .ok_or_else(|| "Runtime archive has no filename".to_string())?
        .to_string_lossy()
        .to_string();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        std::fs::create_dir_all(&destination).map_err(|e| {
            format!(
                "Failed to create stable-diffusion.cpp runtime directory: {}",
                e
            )
        })?;
        let file = std::fs::File::open(&archive_path)
            .map_err(|e| format!("Failed to open stable-diffusion.cpp runtime archive: {}", e))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| format!("Failed to read stable-diffusion.cpp runtime archive: {}", e))?;
        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|e| format!("Failed to read runtime archive entry: {}", e))?;
            let Some(relative) = entry.enclosed_name() else {
                continue;
            };
            let output = destination.join(relative);
            if entry.is_dir() {
                std::fs::create_dir_all(&output)
                    .map_err(|e| format!("Failed to create runtime directory: {}", e))?;
                continue;
            }
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create runtime directory: {}", e))?;
            }
            let mut output_file = std::fs::File::create(&output)
                .map_err(|e| format!("Failed to extract runtime file: {}", e))?;
            std::io::copy(&mut entry, &mut output_file)
                .map_err(|e| format!("Failed to extract runtime file: {}", e))?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for executable in ["sd-server", "sd-cli"] {
                let path = destination.join(executable);
                if path.exists() {
                    let mut permissions = std::fs::metadata(&path)
                        .map_err(|e| format!("Failed to inspect runtime executable: {}", e))?
                        .permissions();
                    permissions.set_mode(0o755);
                    std::fs::set_permissions(path, permissions)
                        .map_err(|e| format!("Failed to mark runtime executable: {}", e))?;
                }
            }
        }
        std::fs::write(runtime_archive_marker(&destination, &archive_name), b"ok")
            .map_err(|e| format!("Failed to finalize runtime extraction: {}", e))?;
        Ok(())
    })
    .await
    .map_err(|e| format!("Runtime extraction task failed: {}", e))?
}
