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
    recommended: bool,
    smaller: bool,
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
        recommended: false,
        smaller: true,
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
        recommended: true,
        smaller: false,
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
        recommended: false,
        smaller: true,
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
        recommended: false,
        smaller: false,
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
        recommended: true,
        smaller: false,
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
    recommended: true,
    smaller: false,
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
        recommended: false,
        smaller: true,
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
        recommended: true,
        smaller: false,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct ComputePolicy {
    multi_gpu_enabled: bool,
    gpu_device_ids: Vec<usize>,
    single_gpu_device_id: Option<usize>,
    device_budgets_gib: std::collections::BTreeMap<usize, f64>,
    split_mode: String,
}

impl Default for ComputePolicy {
    fn default() -> Self {
        Self {
            multi_gpu_enabled: false,
            gpu_device_ids: Vec::new(),
            single_gpu_device_id: None,
            device_budgets_gib: std::collections::BTreeMap::new(),
            split_mode: "layer".to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyComputePolicy {
    mode: String,
    selected_devices: Vec<String>,
    device_budgets_gib: std::collections::BTreeMap<String, f64>,
    split_mode: String,
}

fn backend_device_index(name: &str) -> Option<usize> {
    let digits = name
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
}

fn migrate_legacy_compute_policy(policy: LegacyComputePolicy) -> ComputePolicy {
    let gpu_device_ids = policy
        .selected_devices
        .iter()
        .filter_map(|name| backend_device_index(name))
        .collect::<Vec<_>>();
    let device_budgets_gib = policy
        .device_budgets_gib
        .into_iter()
        .filter_map(|(name, budget)| backend_device_index(&name).map(|id| (id, budget)))
        .collect();
    ComputePolicy {
        multi_gpu_enabled: policy.mode == "multi",
        single_gpu_device_id: (policy.mode == "single")
            .then(|| gpu_device_ids.first().copied())
            .flatten(),
        gpu_device_ids,
        device_budgets_gib,
        split_mode: policy.split_mode,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePolicyRequest {
    runtime_release: String,
    runtime_asset: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveComputePolicyRequest {
    runtime_release: String,
    runtime_asset: String,
    policy: ComputePolicy,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePolicyInfo {
    runtime_release: String,
    runtime_asset: String,
    backend: String,
    supports_row_split: bool,
    policy: ComputePolicy,
    devices: Vec<RunnabilityDevice>,
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
pub struct RuntimeCatalog {
    runtime_supported: bool,
    unsupported_reason: Option<String>,
    runtime_releases: Vec<RuntimeRelease>,
}

#[tauri::command]
pub async fn sdcpp_runtime_catalog() -> Result<RuntimeCatalog, String> {
    if cfg!(mobile) {
        return Err("Local stable-diffusion.cpp image generation is desktop-only.".to_string());
    }
    let runtime_releases = fetch_runtime_releases()
        .await?
        .into_iter()
        .map(|(release, assets)| RuntimeRelease {
            name: release.name.unwrap_or_else(|| release.tag_name.clone()),
            tag: release.tag_name,
            published_at: release.published_at,
            prerelease: release.prerelease,
            assets,
        })
        .collect::<Vec<_>>();
    let runtime_supported = !runtime_releases.is_empty();
    Ok(RuntimeCatalog {
        runtime_supported,
        unsupported_reason: (!runtime_supported).then(|| {
            "No stable-diffusion.cpp release assets match this operating system and architecture."
                .to_string()
        }),
        runtime_releases,
    })
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
    recommended: bool,
    smaller: bool,
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
                recommended: variant.recommended,
                smaller: variant.smaller,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInstallRequest {
    runtime_release: String,
    runtime_asset: String,
}

async fn queue_runtime_install(
    app: &AppHandle,
    runtime: &SelectedRuntime,
    install_id: &str,
    display_name: &str,
    install_kind: &str,
) -> Result<Vec<String>, String> {
    let image_root = image_root(app)?;
    let mut queue_ids = Vec::new();
    write_runtime_manifest(app, runtime)?;

    let runtime_archive = runtime_archive_path(app, runtime)?;
    queue_ids.push(
        crate::hf_browser::hf_queue_download(
            app.clone(),
            GITHUB_REPOSITORY.to_string(),
            runtime.asset_name.clone(),
            Some(QueueDownloadMetadata {
                install_id: Some(install_id.to_string()),
                display_name: Some(display_name.to_string()),
                download_role: Some("runtime".to_string()),
                queue_kind: Some("sdcpp".to_string()),
                asset_root: Some(image_root.to_string_lossy().to_string()),
                install_kind: Some(install_kind.to_string()),
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
        let destination = runtime_dependency_archive_path(app, runtime, dependency)?;
        queue_ids.push(
            crate::hf_browser::hf_queue_download(
                app.clone(),
                GITHUB_REPOSITORY.to_string(),
                dependency.name.clone(),
                Some(QueueDownloadMetadata {
                    install_id: Some(install_id.to_string()),
                    display_name: Some(display_name.to_string()),
                    download_role: Some("runtime_dependency".to_string()),
                    queue_kind: Some("sdcpp".to_string()),
                    asset_root: Some(image_root.to_string_lossy().to_string()),
                    install_kind: Some(install_kind.to_string()),
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
    Ok(queue_ids)
}

#[tauri::command]
pub async fn sdcpp_runtime_install(
    app: AppHandle,
    request: RuntimeInstallRequest,
) -> Result<Vec<String>, String> {
    if cfg!(mobile) {
        return Err("Local stable-diffusion.cpp image generation is desktop-only.".to_string());
    }
    let runtime =
        resolve_runtime_selection(&request.runtime_release, &request.runtime_asset).await?;
    if runtime_is_installed(&app, &runtime.release, &runtime.asset_name) {
        return Err("This stable-diffusion.cpp engine build is already installed.".to_string());
    }
    let install_id = format!("sdcpp-runtime:{}:{}", runtime.release, runtime.asset_name);
    let display_name = format!("stable-diffusion.cpp {}", runtime.release);
    queue_runtime_install(&app, &runtime, &install_id, &display_name, "runtime").await
}

#[tauri::command]
pub async fn sdcpp_install(app: AppHandle, request: InstallRequest) -> Result<Vec<String>, String> {
    if cfg!(mobile) {
        return Err("Local stable-diffusion.cpp image generation is desktop-only.".to_string());
    }
    if !runtime_is_installed(&app, &request.runtime_release, &request.runtime_asset) {
        return Err(
            "Install a stable-diffusion.cpp engine build before downloading a model.".to_string(),
        );
    }
    let (profile, variant) = find_profile_variant(&request.profile_id, &request.variant_id)?;
    let install_id = format!(
        "sdcpp:{}:{}:{}:{}",
        profile.id, variant.id, request.runtime_release, request.runtime_asset
    );
    let image_root = image_root(&app)?;
    let mut queue_ids = Vec::new();

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
                    runtime_release: Some(request.runtime_release.clone()),
                    runtime_asset: Some(request.runtime_asset.clone()),
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
    reference_images: Vec<String>,
    #[serde(default)]
    loras: Vec<super::types::ImageLora>,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    negative_prompt: Option<String>,
    #[serde(default)]
    sample_steps: Option<u32>,
    #[serde(default)]
    cfg_scale: Option<f64>,
    #[serde(default)]
    seed: Option<i64>,
    #[serde(default)]
    sample_method: Option<String>,
    #[serde(default)]
    batch_count: Option<u32>,
    #[serde(default)]
    full_execution: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Runnability {
    status: String,
    method: &'static str,
    exact: bool,
    scope: &'static str,
    placement_policy: &'static str,
    elapsed_ms: Option<u64>,
    reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    estimate: Option<RunnabilityEstimate>,
}

const MIB: u64 = 1024 * 1024;
// Mirrors stable-diffusion.cpp src/core/backend_fit.cpp. The catalog file sizes
// are conservative stand-ins for the tensor byte counts that are unavailable
// until the model has been downloaded.
const SDCPP_AUTO_FIT_GPU_MARGIN_BYTES: u64 = 512 * MIB;
const SDCPP_AUTO_FIT_DIT_RESERVE_BYTES: u64 = 2048 * MIB;
const SDCPP_AUTO_FIT_VAE_RESERVE_BYTES: u64 = 1024 * MIB;
const SDCPP_AUTO_FIT_CONDITIONER_RESERVE_BYTES: u64 = 2048 * MIB;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HardwareGpuDevice {
    index: usize,
    name: String,
    description: String,
    memory_total: u64,
    memory_free: u64,
}

#[derive(Debug, Clone)]
struct RuntimeDevice {
    name: String,
    description: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunnabilityDevice {
    id: usize,
    name: String,
    description: String,
    total_bytes: u64,
    free_bytes: u64,
    budget_bytes: u64,
}

#[derive(Debug, Clone)]
struct EstimateComponent {
    name: &'static str,
    params_bytes: u64,
    compute_reserve_bytes: u64,
    splittable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunnabilityPlacement {
    component: &'static str,
    params_bytes: u64,
    compute_reserve_bytes: u64,
    targets: Vec<String>,
    cpu: bool,
    split: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunnabilityEstimate {
    model_bytes: u64,
    available_ram_bytes: Option<u64>,
    plan_mode: &'static str,
    device_source: &'static str,
    devices: Vec<RunnabilityDevice>,
    placements: Vec<RunnabilityPlacement>,
}

fn estimate_components(profile: &ProfileSpec, variant: &VariantSpec) -> Vec<EstimateComponent> {
    let mut conditioner_bytes = 0_u64;
    let mut vae_bytes = 0_u64;
    for component in profile.shared_components {
        match component.role {
            "text_encoder" | "vision_encoder" => {
                conditioner_bytes = conditioner_bytes.saturating_add(component.bytes)
            }
            "vae" => vae_bytes = vae_bytes.saturating_add(component.bytes),
            _ => {}
        }
    }
    vec![
        EstimateComponent {
            name: "DiT",
            params_bytes: variant.diffusion.bytes,
            compute_reserve_bytes: SDCPP_AUTO_FIT_DIT_RESERVE_BYTES,
            splittable: true,
        },
        EstimateComponent {
            name: "VAE",
            params_bytes: vae_bytes,
            compute_reserve_bytes: SDCPP_AUTO_FIT_VAE_RESERVE_BYTES,
            splittable: false,
        },
        EstimateComponent {
            name: "Conditioner",
            params_bytes: conditioner_bytes,
            compute_reserve_bytes: SDCPP_AUTO_FIT_CONDITIONER_RESERVE_BYTES,
            splittable: true,
        },
    ]
}

fn compute_auto_fit_estimate(
    components: &[EstimateComponent],
    devices: Vec<RunnabilityDevice>,
    available_ram_bytes: Option<u64>,
    device_source: &'static str,
) -> RunnabilityEstimate {
    let model_bytes = components
        .iter()
        .map(|component| component.params_bytes)
        .sum();
    if devices.is_empty() {
        return RunnabilityEstimate {
            model_bytes,
            available_ram_bytes,
            plan_mode: "defaultBackend",
            device_source,
            devices,
            placements: components
                .iter()
                .filter(|component| component.params_bytes > 0)
                .map(|component| RunnabilityPlacement {
                    component: component.name,
                    params_bytes: component.params_bytes,
                    compute_reserve_bytes: component.compute_reserve_bytes,
                    targets: vec!["CPU".to_string()],
                    cpu: true,
                    split: false,
                })
                .collect(),
        };
    }

    let mut order = (0..components.len()).collect::<Vec<_>>();
    order.sort_by_key(|index| std::cmp::Reverse(components[*index].params_bytes));

    let mut params_sum = vec![0_u64; devices.len()];
    let mut max_reserve = vec![0_u64; devices.len()];
    let mut concurrent_targets = vec![Vec::<usize>::new(); components.len()];
    let mut concurrent = true;
    for component_index in &order {
        let component = &components[*component_index];
        if component.params_bytes == 0 {
            continue;
        }
        let mut best = None;
        for (device_index, device) in devices.iter().enumerate() {
            let need = params_sum[device_index]
                .saturating_add(component.params_bytes)
                .saturating_add(max_reserve[device_index].max(component.compute_reserve_bytes));
            if need > device.budget_bytes {
                continue;
            }
            let remaining = device.budget_bytes.saturating_sub(params_sum[device_index]);
            if best.is_none_or(|current: usize| {
                remaining
                    > devices[current]
                        .budget_bytes
                        .saturating_sub(params_sum[current])
            }) {
                best = Some(device_index);
            }
        }
        let Some(best) = best else {
            concurrent = false;
            break;
        };
        params_sum[best] = params_sum[best].saturating_add(component.params_bytes);
        max_reserve[best] = max_reserve[best].max(component.compute_reserve_bytes);
        concurrent_targets[*component_index].push(best);
    }

    if concurrent {
        return RunnabilityEstimate {
            model_bytes,
            available_ram_bytes,
            plan_mode: "concurrent",
            device_source,
            placements: components
                .iter()
                .enumerate()
                .filter(|(_, component)| component.params_bytes > 0)
                .map(|(index, component)| RunnabilityPlacement {
                    component: component.name,
                    params_bytes: component.params_bytes,
                    compute_reserve_bytes: component.compute_reserve_bytes,
                    targets: concurrent_targets[index]
                        .iter()
                        .map(|device_index| devices[*device_index].name.clone())
                        .collect(),
                    cpu: false,
                    split: false,
                })
                .collect(),
            devices,
        };
    }

    let mut targets = vec![Vec::<usize>::new(); components.len()];
    let mut cpu = vec![false; components.len()];
    for component_index in &order {
        let component = &components[*component_index];
        if component.params_bytes == 0 {
            continue;
        }
        let best = devices
            .iter()
            .enumerate()
            .filter(|(_, device)| {
                component
                    .params_bytes
                    .saturating_add(component.compute_reserve_bytes)
                    <= device.budget_bytes
            })
            .max_by_key(|(_, device)| device.budget_bytes)
            .map(|(index, _)| index);
        if let Some(best) = best {
            targets[*component_index].push(best);
            continue;
        }
        if component.splittable && devices.len() > 1 {
            let capacity = devices
                .iter()
                .map(|device| {
                    device
                        .budget_bytes
                        .saturating_sub(component.compute_reserve_bytes)
                })
                .sum::<u64>();
            if component.params_bytes <= capacity {
                let mut device_order = (0..devices.len()).collect::<Vec<_>>();
                device_order.sort_by_key(|index| std::cmp::Reverse(devices[*index].budget_bytes));
                targets[*component_index] = device_order;
                continue;
            }
        }
        cpu[*component_index] = true;
    }

    RunnabilityEstimate {
        model_bytes,
        available_ram_bytes,
        plan_mode: "timeShare",
        device_source,
        placements: components
            .iter()
            .enumerate()
            .filter(|(_, component)| component.params_bytes > 0)
            .map(|(index, component)| {
                let on_cpu = cpu[index];
                RunnabilityPlacement {
                    component: component.name,
                    params_bytes: component.params_bytes,
                    compute_reserve_bytes: component.compute_reserve_bytes,
                    targets: if on_cpu {
                        vec!["CPU".to_string()]
                    } else {
                        targets[index]
                            .iter()
                            .map(|device_index| devices[*device_index].name.clone())
                            .collect()
                    },
                    cpu: on_cpu,
                    split: targets[index].len() > 1,
                }
            })
            .collect(),
        devices,
    }
}

fn normalized_device_identity(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

async fn runtime_devices(
    app: &AppHandle,
    runtime_release: &str,
    runtime_asset: &str,
) -> Result<Vec<RuntimeDevice>, String> {
    let executable = runtime_executable(app, runtime_release, runtime_asset)?;
    let runtime_dir = runtime_root(app, runtime_release, runtime_asset)?;
    let mut command = tokio::process::Command::new(&executable);
    command
        .current_dir(&runtime_dir)
        .arg("--list-devices")
        .kill_on_drop(true);
    #[cfg(target_os = "linux")]
    {
        let existing = std::env::var_os("LD_LIBRARY_PATH").unwrap_or_default();
        let mut paths = vec![runtime_dir];
        paths.extend(std::env::split_paths(&existing));
        let joined = std::env::join_paths(paths)
            .map_err(|error| format!("Failed to configure engine libraries: {error}"))?;
        command.env("LD_LIBRARY_PATH", joined);
    }
    let output = tokio::time::timeout(Duration::from_secs(10), command.output())
        .await
        .map_err(|_| "Timed out while asking the selected engine for its devices.".to_string())?
        .map_err(|error| format!("Failed to query the selected engine devices: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if detail.is_empty() {
            format!(
                "The selected engine device query exited with {}.",
                output.status
            )
        } else {
            format!("The selected engine device query failed: {detail}")
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let (name, description) = line.split_once('\t')?;
            let name = name.trim();
            let description = description.trim();
            (!name.is_empty() && !name.eq_ignore_ascii_case("cpu")).then(|| RuntimeDevice {
                name: name.to_string(),
                description: description.to_string(),
            })
        })
        .collect())
}

async fn matched_runtime_devices(
    app: &AppHandle,
    runtime_release: &str,
    runtime_asset: &str,
) -> Result<Vec<RunnabilityDevice>, String> {
    let hardware_value = crate::llama_cpp::llamacpp_backend_devices().await?;
    let hardware = serde_json::from_value::<Vec<HardwareGpuDevice>>(hardware_value)
        .map_err(|error| format!("Failed to read GPU memory information: {error}"))?;
    let runtime_backend = runtime_backend_for_current_platform(runtime_asset).ok_or_else(|| {
        "The selected engine variant is not supported on this platform.".to_string()
    })?;

    let matched = if runtime_backend == "cpu" {
        Vec::new()
    } else {
        let runtime_devices = runtime_devices(app, runtime_release, runtime_asset).await?;
        let runtime_has_gpu = !runtime_devices.is_empty();
        let mut matched = Vec::new();
        let mut used_hardware = std::collections::HashSet::new();
        for runtime_device in runtime_devices {
            let runtime_name = normalized_device_identity(&runtime_device.name);
            let runtime_description = normalized_device_identity(&runtime_device.description);
            let hardware_index = hardware
                .iter()
                .enumerate()
                .filter(|(index, _)| !used_hardware.contains(index))
                .find(|(_, device)| normalized_device_identity(&device.name) == runtime_name)
                .map(|(index, _)| index)
                .or_else(|| {
                    hardware
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| !used_hardware.contains(index))
                        .find(|(_, device)| {
                            !runtime_description.is_empty()
                                && normalized_device_identity(&device.description)
                                    == runtime_description
                        })
                        .map(|(index, _)| index)
                });
            if let Some(hardware_index) = hardware_index {
                used_hardware.insert(hardware_index);
                let device = &hardware[hardware_index];
                matched.push(RunnabilityDevice {
                    id: device.index,
                    name: runtime_device.name,
                    description: runtime_device.description,
                    total_bytes: device.memory_total,
                    free_bytes: device.memory_free,
                    budget_bytes: device
                        .memory_free
                        .saturating_sub(SDCPP_AUTO_FIT_GPU_MARGIN_BYTES),
                });
            }
        }
        if runtime_has_gpu && matched.is_empty() {
            return Err(
                "The selected engine reported GPU devices, but their live memory could not be matched to the system GPU inventory."
                    .to_string(),
            );
        }
        matched
    };

    Ok(matched)
}

fn validate_compute_policy(
    policy: &ComputePolicy,
    backend: &str,
    devices: &[RunnabilityDevice],
) -> Result<(), String> {
    if !matches!(policy.split_mode.as_str(), "layer" | "row") {
        return Err("Split mode must be layer or row.".to_string());
    }
    if policy.split_mode == "row" && backend != "cuda" {
        return Err("Row splitting is available only with a CUDA engine build.".to_string());
    }

    let available = devices
        .iter()
        .map(|device| device.id)
        .collect::<std::collections::HashSet<_>>();
    let selected = policy
        .gpu_device_ids
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    if selected.len() != policy.gpu_device_ids.len() {
        return Err("Selected GPU devices must be unique.".to_string());
    }
    if let Some(missing) = selected.iter().find(|id| !available.contains(id)) {
        return Err(format!(
            "GPU device #{missing} is not available to this engine."
        ));
    }
    if let Some(single_gpu_device_id) = policy.single_gpu_device_id {
        if !available.contains(&single_gpu_device_id) {
            return Err(format!(
                "GPU device #{single_gpu_device_id} is not available to this engine."
            ));
        }
        if policy.multi_gpu_enabled {
            return Err(
                "A single-GPU override cannot be active while multi-GPU is enabled.".to_string(),
            );
        }
    }
    if policy.multi_gpu_enabled && policy.gpu_device_ids.len() < 2 {
        return Err("Multi-GPU mode requires at least two selected GPUs.".to_string());
    }

    for (id, budget) in &policy.device_budgets_gib {
        let device = devices
            .iter()
            .find(|device| device.id == *id)
            .ok_or_else(|| format!("A VRAM budget was provided for unavailable GPU #{id}."))?;
        if !budget.is_finite() || *budget <= 0.0 {
            return Err(format!(
                "The VRAM budget for GPU #{id} must be greater than zero."
            ));
        }
        let total_gib = device.total_bytes as f64 / 1024_f64.powi(3);
        if *budget > total_gib {
            return Err(format!(
                "The VRAM budget for GPU #{id} exceeds its total memory ({total_gib:.1} GiB)."
            ));
        }
    }
    Ok(())
}

fn devices_for_policy(
    policy: &ComputePolicy,
    mut devices: Vec<RunnabilityDevice>,
) -> Vec<RunnabilityDevice> {
    if policy.multi_gpu_enabled {
        devices.retain(|device| policy.gpu_device_ids.contains(&device.id));
    } else if let Some(single_gpu_device_id) = policy.single_gpu_device_id {
        devices.retain(|device| device.id == single_gpu_device_id);
    }
    apply_policy_budgets(policy, &mut devices);
    devices
}

#[derive(Debug, Clone)]
struct ResolvedComputePolicy {
    backend: String,
    automatic: bool,
    available_devices: Vec<RunnabilityDevice>,
    effective_devices: Vec<RunnabilityDevice>,
}

async fn resolve_compute_policy(
    app: &AppHandle,
    runtime_release: &str,
    runtime_asset: &str,
    policy: &ComputePolicy,
) -> Result<ResolvedComputePolicy, String> {
    let backend = runtime_backend_for_current_platform(runtime_asset).ok_or_else(|| {
        "The selected engine variant is not supported on this platform.".to_string()
    })?;
    let mut available_devices =
        matched_runtime_devices(app, runtime_release, runtime_asset).await?;
    validate_compute_policy(policy, &backend, &available_devices)?;
    let effective_devices = devices_for_policy(policy, available_devices.clone());
    apply_policy_budgets(policy, &mut available_devices);
    Ok(ResolvedComputePolicy {
        backend,
        automatic: !policy.multi_gpu_enabled && policy.single_gpu_device_id.is_none(),
        available_devices,
        effective_devices,
    })
}

fn apply_policy_budgets(policy: &ComputePolicy, devices: &mut [RunnabilityDevice]) {
    for device in devices.iter_mut() {
        if let Some(gib) = policy.device_budgets_gib.get(&device.id) {
            let requested = (*gib * 1024_f64.powi(3)).round() as u64;
            device.budget_bytes = requested.min(device.free_bytes);
        }
    }
}

fn max_vram_spec(policy: &ComputePolicy, devices: &[RunnabilityDevice]) -> Option<String> {
    let assignments = devices
        .iter()
        .filter_map(|device| {
            let budget = policy.device_budgets_gib.get(&device.id)?;
            let mut value = format!("{budget:.3}");
            while value.contains('.') && value.ends_with('0') {
                value.pop();
            }
            if value.ends_with('.') {
                value.pop();
            }
            Some(format!("{}={value}", device.name.to_ascii_lowercase()))
        })
        .collect::<Vec<_>>();
    (!assignments.is_empty()).then(|| assignments.join(","))
}

fn manual_backend_specs(estimate: &RunnabilityEstimate) -> (String, Option<String>) {
    let mut runtime = Vec::new();
    let mut params = Vec::new();
    for placement in &estimate.placements {
        let module = match placement.component {
            "DiT" => "diffusion",
            "Conditioner" => "te",
            "VAE" => "vae",
            _ => continue,
        };
        let target = if placement.cpu {
            "cpu".to_string()
        } else {
            placement.targets.join("&")
        };
        runtime.push(format!("{module}={target}"));
        if estimate.plan_mode == "timeShare" && !placement.cpu {
            params.push(format!("{module}=disk"));
        }
    }
    (
        runtime.join(","),
        (!params.is_empty()).then(|| params.join(",")),
    )
}

async fn estimate_with_compute_policy(
    app: &AppHandle,
    profile: &ProfileSpec,
    variant: &VariantSpec,
    runtime_release: &str,
    runtime_asset: &str,
    policy: &ComputePolicy,
) -> Result<RunnabilityEstimate, String> {
    let resolved = resolve_compute_policy(app, runtime_release, runtime_asset, policy).await?;

    Ok(compute_auto_fit_estimate(
        &estimate_components(profile, variant),
        resolved.effective_devices,
        crate::llama_cpp::available_memory_bytes(),
        "configuredEnginePolicy",
    ))
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
    if !runtime_is_installed(&app, &request.runtime_release, &request.runtime_asset) {
        return Ok(Runnability {
            status: "notInstalled".to_string(),
            method: "stableDiffusionCppAutoFitEstimate",
            exact: false,
            scope: "engineUnavailable",
            placement_policy: "notRun",
            elapsed_ms: None,
            reason: "Install the selected stable-diffusion.cpp engine build before checking model runnability. The model itself does not need to be installed."
                .to_string(),
            estimate: None,
        });
    }
    let compute_policy =
        load_compute_policy(&app, &request.runtime_release, &request.runtime_asset);
    let model_installed = is_variant_installed(
        &app,
        profile,
        variant,
        Some(&request.runtime_release),
        Some(&request.runtime_asset),
    );
    let refs = if request.reference_images.is_empty() {
        request.reference_image_count.unwrap_or(0) as usize
    } else {
        if model_installed
            && request
                .reference_image_count
                .is_some_and(|count| count as usize != request.reference_images.len())
        {
            return Err(
                "referenceImageCount must match the number of supplied referenceImages."
                    .to_string(),
            );
        }
        request.reference_images.len()
    };
    if model_installed {
        if let Some(maximum) = profile.max_reference_images {
            if refs > maximum as usize {
                return Err(format!(
                    "{} accepts at most {} reference images.",
                    profile.display_name, maximum
                ));
            }
        }
    }
    if model_installed && profile.requires_reference_image && refs == 0 {
        return Err(format!(
            "{} requires at least one reference image.",
            profile.display_name
        ));
    }
    let (width, height) = (
        request.width.unwrap_or(profile.default_width),
        request.height.unwrap_or(profile.default_height),
    );
    if model_installed && (width == 0 || height == 0) {
        return Err("Fit-test width and height must be greater than zero.".to_string());
    }
    let requested_steps = request.sample_steps.unwrap_or(profile.default_steps as u32);
    if model_installed && requested_steps == 0 {
        return Err("Fit-test sampleSteps must be greater than zero.".to_string());
    }
    let batch_count = request.batch_count.unwrap_or(1);
    if model_installed && batch_count == 0 {
        return Err("Fit-test batchCount must be greater than zero.".to_string());
    }
    if !model_installed {
        let started = Instant::now();
        return Ok(
            match estimate_with_compute_policy(
                &app,
                profile,
                variant,
                &request.runtime_release,
                &request.runtime_asset,
                &compute_policy,
            )
            .await
            {
                Ok(estimate) => {
                    let uses_cpu = estimate.placements.iter().any(|placement| placement.cpu);
                    let uses_split = estimate.placements.iter().any(|placement| placement.split);
                    Runnability {
                        status: "estimatedRunnable".to_string(),
                        method: "stableDiffusionCppConfiguredPlacementEstimate",
                        exact: false,
                        scope: "preInstallEstimate",
                        placement_policy: "sdCppConfiguredPolicyEstimate",
                        elapsed_ms: Some(started.elapsed().as_millis() as u64),
                        reason: if uses_cpu {
                            "Estimated runnable using the configured Stable Diffusion compute policy with CPU fallback for at least one model component. This is not a speed guarantee; install the model to run the exact execution probe."
                            .to_string()
                        } else if uses_split {
                            "Estimated to fit by splitting at least one model component across the GPUs selected by the configured compute policy. Install the model to verify with a real execution probe."
                            .to_string()
                        } else if estimate.plan_mode == "timeShare" {
                            "Estimated to fit on the configured GPU devices by loading model components per phase. Install the model to verify with a real execution probe."
                            .to_string()
                        } else {
                            "Estimated to fit concurrently on the GPU devices selected by the configured compute policy. Install the model to verify with a real execution probe."
                            .to_string()
                        },
                        estimate: Some(estimate),
                    }
                }
                Err(error) => Runnability {
                    status: "inconclusive".to_string(),
                    method: "stableDiffusionCppConfiguredPlacementEstimate",
                    exact: false,
                    scope: "preInstallEstimate",
                    placement_policy: "sdCppConfiguredPolicyEstimate",
                    elapsed_ms: Some(started.elapsed().as_millis() as u64),
                    reason: format!(
                        "The pre-install runnability estimate could not be completed: {error}"
                    ),
                    estimate: None,
                },
            },
        );
    }

    let config = InstalledModelConfig {
        sdcpp_profile_id: profile.id.to_string(),
        sdcpp_variant_id: variant.id.to_string(),
        sdcpp_runtime_release: request.runtime_release,
        sdcpp_runtime_asset: request.runtime_asset,
    };
    let automatic_placement =
        !compute_policy.multi_gpu_enabled && compute_policy.single_gpu_device_id.is_none();
    let runtime_placement_policy = if automatic_placement {
        "sdCppAutoFit"
    } else {
        "sdCppConfiguredPolicy"
    };
    let started = Instant::now();
    let base_url = match ensure_server(&app, &config, profile, variant).await {
        Ok(base_url) => base_url,
        Err(error) => {
            return Ok(Runnability {
                status: "inconclusive".to_string(),
                method: "stableDiffusionCppExecutionProbe",
                exact: false,
                scope: "serverStartup",
                placement_policy: runtime_placement_policy,
                elapsed_ms: Some(started.elapsed().as_millis() as u64),
                reason: format!(
                    "The sd.cpp server could not be prepared, so no runnability verdict was made: {}",
                    error
                ),
                estimate: None,
            });
        }
    };
    let supplied_references = !request.reference_images.is_empty();
    let references = if supplied_references {
        request.reference_images
    } else if refs > 0 {
        let reference = blank_reference_data_url(width, height)?;
        vec![reference; refs]
    } else {
        Vec::new()
    };
    let loras = normalize_loras(&app, &request.loras)?;
    let supplied_prompt = request.prompt.is_some();
    let prompt = request
        .prompt
        .unwrap_or_else(|| "runnability probe".to_string());
    let sample_steps = if request.full_execution {
        requested_steps
    } else {
        1
    };
    let payload = build_generation_payload(SdGenerationPayload {
        prompt: &prompt,
        negative_prompt: request.negative_prompt.as_deref().unwrap_or(""),
        width,
        height,
        seed: request.seed.unwrap_or(-1),
        batch_count,
        references: &references,
        sample_method: request.sample_method.as_deref().unwrap_or("euler"),
        sample_steps,
        cfg: request.cfg_scale.unwrap_or(profile.default_cfg as f64),
        loras: &loras,
    });
    let result = run_probe_job(&base_url, payload).await;
    let request_matched =
        request.full_execution && supplied_prompt && (refs == 0 || supplied_references);
    Ok(match result {
        Ok(()) => Runnability {
            status: "passed".to_string(),
            method: "stableDiffusionCppExecutionProbe",
            exact: request_matched,
            scope: if request_matched {
                "fullRequest"
            } else {
                "executionProbe"
            },
            placement_policy: runtime_placement_policy,
            elapsed_ms: Some(started.elapsed().as_millis() as u64),
            reason: if request_matched {
                if automatic_placement {
                    "stable-diffusion.cpp completed the full supplied generation request using its real auto-fit placement."
                        .to_string()
                } else {
                    "stable-diffusion.cpp completed the full supplied generation request using the configured GPU placement policy."
                        .to_string()
                }
            } else if request.full_execution {
                "stable-diffusion.cpp completed a full representative generation, but generated placeholders were used for request data that was not supplied."
                    .to_string()
            } else {
                "stable-diffusion.cpp completed a one-step execution probe at the requested shape. This proves the tested graph ran, but it is not a full-request guarantee."
                    .to_string()
            },
            estimate: None,
        },
        Err(ProbeJobError::Execution(error)) => Runnability {
            status: "failed".to_string(),
            method: "stableDiffusionCppExecutionProbe",
            exact: request_matched,
            scope: if request_matched {
                "fullRequest"
            } else {
                "executionProbe"
            },
            placement_policy: runtime_placement_policy,
            elapsed_ms: Some(started.elapsed().as_millis() as u64),
            reason: error,
            estimate: None,
        },
        Err(ProbeJobError::Infrastructure(error)) => Runnability {
            status: "inconclusive".to_string(),
            method: "stableDiffusionCppExecutionProbe",
            exact: false,
            scope: "probeInfrastructure",
            placement_policy: runtime_placement_policy,
            elapsed_ms: Some(started.elapsed().as_millis() as u64),
            reason: format!(
                "The execution probe could not produce a runnability verdict: {}",
                error
            ),
            estimate: None,
        },
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskUsage {
    components_bytes: u64,
    runtimes_bytes: u64,
    loras_bytes: u64,
    total_bytes: u64,
    has_engine: bool,
    engine_release: Option<String>,
    engine_backend: Option<String>,
}

fn directory_size(root: &Path) -> u64 {
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| entry.metadata().ok())
        .map(|metadata| metadata.len())
        .sum()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ActiveRuntime {
    release: String,
    asset: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledRuntime {
    release: String,
    asset: String,
    backend: String,
    size_bytes: u64,
    active: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInventory {
    installed: Vec<InstalledRuntime>,
    active: Option<ActiveRuntime>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSelectionRequest {
    runtime_release: String,
    runtime_asset: String,
}

fn runtime_storage_root(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(crate::utils::lettuce_dir(app)?
        .join("runtimes")
        .join("stable-diffusion.cpp"))
}

fn active_runtime_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(runtime_storage_root(app)?.join(".active-runtime.json"))
}

fn installed_runtimes(app: &AppHandle) -> Result<Vec<InstalledRuntime>, String> {
    let root = runtime_storage_root(app)?;
    let mut installed = Vec::new();
    let Ok(releases) = std::fs::read_dir(&root) else {
        return Ok(installed);
    };
    for release_entry in releases.flatten() {
        if !release_entry
            .file_type()
            .map(|kind| kind.is_dir())
            .unwrap_or(false)
        {
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
                backend: runtime_backend_for_current_platform(&asset)
                    .unwrap_or_else(|| "unknown".to_string()),
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
    Ok(installed)
}

fn saved_active_runtime(app: &AppHandle) -> Option<ActiveRuntime> {
    let path = active_runtime_path(app).ok()?;
    let bytes = std::fs::read(path).ok()?;
    let selected = serde_json::from_slice::<ActiveRuntime>(&bytes).ok()?;
    runtime_is_installed(app, &selected.release, &selected.asset).then_some(selected)
}

fn effective_active_runtime(
    app: &AppHandle,
    installed: &[InstalledRuntime],
) -> Option<ActiveRuntime> {
    saved_active_runtime(app).or_else(|| {
        installed.first().map(|runtime| ActiveRuntime {
            release: runtime.release.clone(),
            asset: runtime.asset.clone(),
        })
    })
}

fn save_active_runtime(app: &AppHandle, selected: &ActiveRuntime) -> Result<(), String> {
    let path = active_runtime_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create the engine runtime directory: {e}"))?;
    }
    let bytes = serde_json::to_vec_pretty(selected)
        .map_err(|e| format!("Failed to serialize the active engine selection: {e}"))?;
    std::fs::write(path, bytes)
        .map_err(|e| format!("Failed to save the active engine selection: {e}"))
}

fn clear_active_runtime(app: &AppHandle) -> Result<(), String> {
    let path = active_runtime_path(app)?;
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "Failed to clear the active engine selection: {error}"
        )),
    }
}

async fn stop_managed_server() {
    let mut managed = MANAGED_SERVER.lock().await;
    if let Some(mut server) = managed.take() {
        let _ = server.child.kill().await;
        let _ = server.child.wait().await;
    }
}

#[tauri::command]
pub async fn sdcpp_runtime_inventory(app: AppHandle) -> Result<RuntimeInventory, String> {
    if cfg!(mobile) {
        return Err("Local stable-diffusion.cpp image generation is desktop-only.".to_string());
    }
    let mut installed = installed_runtimes(&app)?;
    let saved_active = saved_active_runtime(&app);
    let active = saved_active.clone().or_else(|| {
        installed.first().map(|runtime| ActiveRuntime {
            release: runtime.release.clone(),
            asset: runtime.asset.clone(),
        })
    });
    if saved_active.is_none() {
        if let Some(selected) = &active {
            save_active_runtime(&app, selected)?;
        }
    }
    if let Some(selected) = &active {
        for runtime in &mut installed {
            runtime.active = runtime.release == selected.release && runtime.asset == selected.asset;
        }
    }
    Ok(RuntimeInventory { installed, active })
}

async fn compute_policy_info(
    app: &AppHandle,
    runtime_release: &str,
    runtime_asset: &str,
    policy: ComputePolicy,
) -> Result<ComputePolicyInfo, String> {
    if !runtime_is_installed(app, runtime_release, runtime_asset) {
        return Err("The selected stable-diffusion.cpp engine build is not installed.".to_string());
    }
    let resolved = resolve_compute_policy(app, runtime_release, runtime_asset, &policy).await?;
    Ok(ComputePolicyInfo {
        runtime_release: runtime_release.to_string(),
        runtime_asset: runtime_asset.to_string(),
        supports_row_split: resolved.backend == "cuda",
        backend: resolved.backend,
        policy,
        devices: resolved.available_devices,
    })
}

#[tauri::command]
pub async fn sdcpp_compute_policy(
    app: AppHandle,
    request: ComputePolicyRequest,
) -> Result<ComputePolicyInfo, String> {
    if cfg!(mobile) {
        return Err("Local stable-diffusion.cpp image generation is desktop-only.".to_string());
    }
    let policy = load_compute_policy(&app, &request.runtime_release, &request.runtime_asset);
    compute_policy_info(
        &app,
        &request.runtime_release,
        &request.runtime_asset,
        policy,
    )
    .await
}

#[tauri::command]
pub async fn sdcpp_compute_policy_save(
    app: AppHandle,
    request: SaveComputePolicyRequest,
) -> Result<ComputePolicyInfo, String> {
    if cfg!(mobile) {
        return Err("Local stable-diffusion.cpp image generation is desktop-only.".to_string());
    }
    let info = compute_policy_info(
        &app,
        &request.runtime_release,
        &request.runtime_asset,
        request.policy,
    )
    .await?;
    save_compute_policy(
        &app,
        &request.runtime_release,
        &request.runtime_asset,
        &info.policy,
    )?;
    stop_managed_server().await;
    Ok(info)
}

#[tauri::command]
pub async fn sdcpp_runtime_switch(
    app: AppHandle,
    request: RuntimeSelectionRequest,
) -> Result<(), String> {
    if !runtime_is_installed(&app, &request.runtime_release, &request.runtime_asset) {
        return Err("The selected stable-diffusion.cpp engine build is not installed.".to_string());
    }
    let selected = ActiveRuntime {
        release: request.runtime_release,
        asset: request.runtime_asset,
    };
    save_active_runtime(&app, &selected)?;
    stop_managed_server().await;
    Ok(())
}

#[tauri::command]
pub async fn sdcpp_runtime_delete(
    app: AppHandle,
    request: RuntimeSelectionRequest,
) -> Result<(), String> {
    if !runtime_is_installed(&app, &request.runtime_release, &request.runtime_asset) {
        return Err("The selected stable-diffusion.cpp engine build is not installed.".to_string());
    }
    stop_managed_server().await;
    let selected = ActiveRuntime {
        release: request.runtime_release,
        asset: request.runtime_asset,
    };
    let removed_active = effective_active_runtime(&app, &installed_runtimes(&app)?)
        .is_some_and(|active| active.release == selected.release && active.asset == selected.asset);
    let root = runtime_root(&app, &selected.release, &selected.asset)?;
    std::fs::remove_dir_all(&root)
        .map_err(|e| format!("Failed to delete the stable-diffusion.cpp engine build: {e}"))?;
    if let Some(parent) = root.parent() {
        let _ = std::fs::remove_dir(parent);
    }
    if removed_active {
        let remaining = installed_runtimes(&app)?;
        if let Some(next) = remaining.first() {
            save_active_runtime(
                &app,
                &ActiveRuntime {
                    release: next.release.clone(),
                    asset: next.asset.clone(),
                },
            )?;
        } else {
            clear_active_runtime(&app)?;
        }
    }
    Ok(())
}

fn detect_engine_build(app: &AppHandle) -> Option<(String, String)> {
    let root = crate::utils::lettuce_dir(app)
        .ok()?
        .join("runtimes")
        .join("stable-diffusion.cpp");
    for release_entry in std::fs::read_dir(&root).ok()?.flatten() {
        if !release_entry
            .file_type()
            .map(|kind| kind.is_dir())
            .unwrap_or(false)
        {
            continue;
        }
        let Ok(assets) = std::fs::read_dir(release_entry.path()) else {
            continue;
        };
        for asset_entry in assets.flatten() {
            let asset_path = asset_entry.path();
            if runtime_root_is_complete(&asset_path) {
                return Some((
                    release_entry.file_name().to_string_lossy().to_string(),
                    asset_entry.file_name().to_string_lossy().to_string(),
                ));
            }
        }
    }
    None
}

#[tauri::command]
pub async fn sdcpp_disk_usage(app: AppHandle) -> Result<DiskUsage, String> {
    if cfg!(mobile) {
        return Err("Local stable-diffusion.cpp image generation is desktop-only.".to_string());
    }
    let components_root = image_root(&app)?.join("components");
    let runtimes_root = crate::utils::lettuce_dir(&app)?
        .join("runtimes")
        .join("stable-diffusion.cpp");
    let loras_root = lora_root(&app)?;
    let components_bytes = directory_size(&components_root);
    let runtimes_bytes = directory_size(&runtimes_root);
    let loras_bytes = directory_size(&loras_root);
    let engine = detect_engine_build(&app);
    let (engine_release, engine_backend) = match &engine {
        Some((release, asset)) => (
            Some(release.clone()),
            runtime_backend_for_current_platform(asset),
        ),
        None => (None, None),
    };
    Ok(DiskUsage {
        components_bytes,
        runtimes_bytes,
        loras_bytes,
        total_bytes: components_bytes + runtimes_bytes + loras_bytes,
        has_engine: engine.is_some(),
        engine_release,
        engine_backend,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModel {
    profile_id: &'static str,
    variant_id: &'static str,
    display_name: String,
    runtime_release: Option<String>,
    runtime_asset: Option<String>,
    runtime_backend: Option<String>,
    component_bytes_on_disk: u64,
    model_id: Option<String>,
    supports_text_to_image: bool,
    supports_image_edit: bool,
    recommended_for_scenes: bool,
    requires_reference_image: bool,
}

fn installed_display_name(profile: &ProfileSpec, variant: &VariantSpec) -> String {
    format!(
        "{} ({})",
        profile.display_name,
        variant
            .label
            .replace(" (recommended)", "")
            .replace(" (smaller)", "")
    )
}

fn installed_model_id(app: &AppHandle, model_name: &str) -> Result<Option<String>, String> {
    use rusqlite::OptionalExtension;
    let conn = crate::storage_manager::db::open_db(app)?;
    conn.query_row(
        "SELECT id FROM models WHERE provider_id = ?1 AND name = ?2 LIMIT 1",
        rusqlite::params![PROVIDER_ID, model_name],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub async fn sdcpp_installed(app: AppHandle) -> Result<Vec<InstalledModel>, String> {
    if cfg!(mobile) {
        return Err("Local stable-diffusion.cpp image generation is desktop-only.".to_string());
    }
    use rusqlite::OptionalExtension;
    let conn = crate::storage_manager::db::open_db(&app)?;
    let mut installed = Vec::new();
    for profile in PROFILES {
        for variant in profile.variants {
            if !is_variant_installed(&app, profile, variant, None, None) {
                continue;
            }
            let component_bytes_on_disk = all_components(profile, variant)
                .iter()
                .filter_map(|component| component_path(&app, *component).ok())
                .filter_map(|path| std::fs::metadata(path).ok())
                .map(|metadata| metadata.len())
                .sum();
            let model_name = format!("sdcpp:{}:{}", profile.id, variant.id);
            let row = conn
                .query_row(
                    "SELECT id, advanced_model_settings FROM models WHERE provider_id = ?1 AND name = ?2 LIMIT 1",
                    rusqlite::params![PROVIDER_ID, &model_name],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
                )
                .optional()
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let (model_id, runtime_release, runtime_asset) = match row {
                Some((id, advanced)) => {
                    let settings = advanced
                        .as_deref()
                        .and_then(|raw| serde_json::from_str::<Value>(raw).ok());
                    let release = settings
                        .as_ref()
                        .and_then(|value| value.get("sdcppRuntimeRelease"))
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    let asset = settings
                        .as_ref()
                        .and_then(|value| value.get("sdcppRuntimeAsset"))
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    (Some(id), release, asset)
                }
                None => (None, None, None),
            };
            let runtime_backend = runtime_asset
                .as_deref()
                .and_then(runtime_backend_for_current_platform);
            installed.push(InstalledModel {
                profile_id: profile.id,
                variant_id: variant.id,
                display_name: installed_display_name(profile, variant),
                runtime_release,
                runtime_asset,
                runtime_backend,
                component_bytes_on_disk,
                model_id,
                supports_text_to_image: profile.supports_text_to_image,
                supports_image_edit: profile.supports_image_edit,
                recommended_for_scenes: profile.recommended_for_scenes,
                requires_reference_image: profile.requires_reference_image,
            });
        }
    }
    Ok(installed)
}

#[tauri::command]
pub async fn sdcpp_repair_registration(
    app: AppHandle,
    profile_id: String,
    variant_id: String,
) -> Result<String, String> {
    if cfg!(mobile) {
        return Err("Local stable-diffusion.cpp image generation is desktop-only.".to_string());
    }
    let (profile, variant) = find_profile_variant(&profile_id, &variant_id)?;
    if !is_variant_installed(&app, profile, variant, None, None) {
        return Err(
            "The local image model is incomplete. Retry the installation first.".to_string(),
        );
    }
    let (runtime_release, runtime_asset) = detect_engine_build(&app)
        .ok_or_else(|| "No complete stable-diffusion.cpp engine build is installed.".to_string())?;
    register_installed_model(&app, profile, variant, &runtime_release, &runtime_asset)?;
    let model_name = format!("sdcpp:{}:{}", profile.id, variant.id);
    installed_model_id(&app, &model_name)?
        .ok_or_else(|| "The model was registered but could not be read back.".to_string())
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallOptions {
    #[serde(default)]
    also_remove_engine_if_unused: Option<bool>,
}

#[tauri::command]
pub async fn sdcpp_uninstall(
    app: AppHandle,
    profile_id: String,
    variant_id: String,
    options: Option<UninstallOptions>,
) -> Result<(), String> {
    if cfg!(mobile) {
        return Err("Local stable-diffusion.cpp image generation is desktop-only.".to_string());
    }
    let (profile, variant) = find_profile_variant(&profile_id, &variant_id)?;
    let options = options.unwrap_or_default();

    let survivors: Vec<(&'static ProfileSpec, &'static VariantSpec)> = PROFILES
        .iter()
        .flat_map(|candidate| candidate.variants.iter().map(move |v| (candidate, v)))
        .filter(|(candidate, v)| !(candidate.id == profile.id && v.id == variant.id))
        .filter(|(candidate, v)| is_variant_installed(&app, candidate, v, None, None))
        .collect();

    let mut keep_shas: std::collections::HashSet<&'static str> = std::collections::HashSet::new();
    for (candidate, v) in &survivors {
        for component in all_components(candidate, v) {
            keep_shas.insert(component.sha256);
        }
    }

    for component in all_components(profile, variant) {
        if keep_shas.contains(component.sha256) {
            continue;
        }
        if let Ok(path) = component_path(&app, component) {
            let _ = std::fs::remove_file(&path);
            if let Some(parent) = path.parent() {
                let _ = std::fs::remove_dir(parent);
            }
        }
    }

    let model_name = format!("sdcpp:{}:{}", profile.id, variant.id);
    let row = {
        use rusqlite::OptionalExtension;
        let conn = crate::storage_manager::db::open_db(&app)?;
        conn.query_row(
            "SELECT id, advanced_model_settings FROM models WHERE provider_id = ?1 AND name = ?2 LIMIT 1",
            rusqlite::params![PROVIDER_ID, &model_name],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    };
    let (model_id, target_release, target_asset) = match row {
        Some((id, advanced)) => {
            let settings = advanced
                .as_deref()
                .and_then(|raw| serde_json::from_str::<Value>(raw).ok());
            let release = settings
                .as_ref()
                .and_then(|value| value.get("sdcppRuntimeRelease"))
                .and_then(Value::as_str)
                .map(str::to_string);
            let asset = settings
                .as_ref()
                .and_then(|value| value.get("sdcppRuntimeAsset"))
                .and_then(Value::as_str)
                .map(str::to_string);
            (Some(id), release, asset)
        }
        None => {
            let engine = detect_engine_build(&app);
            (
                None,
                engine.as_ref().map(|(release, _)| release.clone()),
                engine.map(|(_, asset)| asset),
            )
        }
    };

    if let Some(id) = model_id {
        crate::storage_manager::models::model_delete(app.clone(), id)?;
    }

    if options.also_remove_engine_if_unused == Some(true) {
        if let (Some(release), Some(asset)) = (target_release.as_deref(), target_asset.as_deref()) {
            let still_used = survivors.iter().any(|(candidate, v)| {
                let name = format!("sdcpp:{}:{}", candidate.id, v.id);
                installed_model_config(&app, &name)
                    .map(|config| {
                        config.sdcpp_runtime_release == release
                            && config.sdcpp_runtime_asset == asset
                    })
                    .unwrap_or(false)
            });
            if !still_used {
                if let Ok(root) = runtime_root(&app, release, asset) {
                    let _ = std::fs::remove_dir_all(&root);
                }
            }
        }
    }

    Ok(())
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

struct SdGenerationPayload<'a> {
    prompt: &'a str,
    negative_prompt: &'a str,
    width: u32,
    height: u32,
    seed: i64,
    batch_count: u32,
    references: &'a [String],
    sample_method: &'a str,
    sample_steps: u32,
    cfg: f64,
    loras: &'a [Value],
}

fn build_generation_payload(params: SdGenerationPayload<'_>) -> Value {
    serde_json::json!({
        "prompt": params.prompt,
        "negative_prompt": params.negative_prompt,
        "width": params.width,
        "height": params.height,
        "seed": params.seed,
        "batch_count": params.batch_count,
        "auto_resize_ref_image": true,
        "ref_images": params.references,
        "sample_params": {
            "scheduler": "discrete",
            "sample_method": params.sample_method,
            "sample_steps": params.sample_steps,
            "guidance": {
                "txt_cfg": params.cfg,
                "img_cfg": params.cfg,
                "distilled_guidance": 0.0
            }
        },
        "lora": params.loras,
        "vae_tiling_params": { "enabled": true },
        "output_format": "png",
        "output_compression": 100
    })
}

enum ProbeJobError {
    Execution(String),
    Infrastructure(String),
}

async fn run_probe_job(base_url: &str, payload: Value) -> Result<(), ProbeJobError> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/sdcpp/v1/img_gen", base_url))
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            ProbeJobError::Infrastructure(format!(
                "Failed to submit stable-diffusion.cpp fit test: {}",
                e
            ))
        })?;
    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await.unwrap_or_default();
        return Err(ProbeJobError::Execution(format!(
            "Fit test was rejected ({}): {}",
            status, detail
        )));
    }
    let accepted = response.json::<Value>().await.map_err(|e| {
        ProbeJobError::Infrastructure(format!("Failed to parse fit-test response: {}", e))
    })?;
    let poll_path = accepted
        .get("poll_url")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ProbeJobError::Infrastructure(
                "Fit-test response did not include a poll URL".to_string(),
            )
        })?;
    let poll_url = if poll_path.starts_with("http://") || poll_path.starts_with("https://") {
        poll_path.to_string()
    } else {
        format!("{}{}", base_url, poll_path)
    };
    for _ in 0..1_200 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let response = client.get(&poll_url).send().await.map_err(|e| {
            ProbeJobError::Infrastructure(format!(
                "Failed to poll stable-diffusion.cpp fit test: {}",
                e
            ))
        })?;
        let job = response.json::<Value>().await.map_err(|e| {
            ProbeJobError::Infrastructure(format!(
                "Failed to parse stable-diffusion.cpp fit test: {}",
                e
            ))
        })?;
        match job.get("status").and_then(Value::as_str) {
            Some("queued") | Some("generating") | Some("running") => continue,
            Some("completed") => return Ok(()),
            Some("failed") | Some("cancelled") => {
                return Err(ProbeJobError::Execution(
                    job.pointer("/error/message")
                        .and_then(Value::as_str)
                        .unwrap_or("stable-diffusion.cpp fit test failed")
                        .to_string(),
                ));
            }
            status => {
                return Err(ProbeJobError::Infrastructure(format!(
                    "Unknown fit-test job status: {:?}",
                    status
                )))
            }
        }
    }
    Err(ProbeJobError::Infrastructure(
        "stable-diffusion.cpp fit test timed out after ten minutes".to_string(),
    ))
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
    let mut config = serde_json::from_str::<InstalledModelConfig>(&advanced)
        .map_err(|e| format!("Local image model has invalid runtime settings: {}", e))?;
    if let Some(active) = effective_active_runtime(app, &installed_runtimes(app)?) {
        config.sdcpp_runtime_release = active.release;
        config.sdcpp_runtime_asset = active.asset;
    }
    Ok(config)
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
    let compute_policy = load_compute_policy(
        app,
        &config.sdcpp_runtime_release,
        &config.sdcpp_runtime_asset,
    );
    let policy_key = serde_json::to_string(&compute_policy)
        .map_err(|error| format!("Failed to fingerprint the compute policy: {error}"))?;
    let key = format!(
        "{}:{}:{}:{}:{}",
        profile.id,
        variant.id,
        config.sdcpp_runtime_release,
        config.sdcpp_runtime_asset,
        policy_key
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
    let resolved_policy = resolve_compute_policy(
        app,
        &config.sdcpp_runtime_release,
        &config.sdcpp_runtime_asset,
        &compute_policy,
    )
    .await?;
    let manual_estimate = (!resolved_policy.automatic).then(|| {
        compute_auto_fit_estimate(
            &estimate_components(profile, variant),
            resolved_policy.effective_devices.clone(),
            crate::llama_cpp::available_memory_bytes(),
            "configuredEnginePolicy",
        )
    });
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
        .arg("--diffusion-fa")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    if let Some(estimate) = &manual_estimate {
        let (backend_spec, params_backend_spec) = manual_backend_specs(estimate);
        command.arg("--backend").arg(backend_spec);
        if let Some(params_backend_spec) = params_backend_spec {
            command.arg("--params-backend").arg(params_backend_spec);
        }
    } else {
        command.arg("--auto-fit");
    }
    command.arg("--split-mode").arg(&compute_policy.split_mode);
    if let Some(max_vram) = max_vram_spec(&compute_policy, &resolved_policy.effective_devices) {
        command.arg("--max-vram").arg(max_vram);
    }
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
    let payload = build_generation_payload(SdGenerationPayload {
        prompt: &request.prompt,
        negative_prompt: &negative_prompt,
        width,
        height,
        seed,
        batch_count: request.n.unwrap_or(1),
        references: &references,
        sample_method: &sample_method,
        sample_steps: steps,
        cfg,
        loras: &loras,
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
                "sdcppSupportsLora": true,
                "sdcppSupportsTextToImage": profile.supports_text_to_image,
                "sdcppSupportsImageEdit": profile.supports_image_edit,
                "sdcppRecommendedForScenes": profile.recommended_for_scenes,
                "sdcppRequiresReferenceImage": profile.requires_reference_image
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

fn compute_policy_path(app: &AppHandle, release: &str, asset: &str) -> Result<PathBuf, String> {
    Ok(runtime_root(app, release, asset)?.join(".lettuce-compute-policy.json"))
}

fn load_compute_policy(app: &AppHandle, release: &str, asset: &str) -> ComputePolicy {
    compute_policy_path(app, release, asset)
        .ok()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| {
            let value = serde_json::from_slice::<serde_json::Value>(&bytes).ok()?;
            if value.get("mode").is_some() {
                serde_json::from_value::<LegacyComputePolicy>(value)
                    .ok()
                    .map(migrate_legacy_compute_policy)
            } else {
                serde_json::from_value::<ComputePolicy>(value).ok()
            }
        })
        .unwrap_or_default()
}

fn save_compute_policy(
    app: &AppHandle,
    release: &str,
    asset: &str,
    policy: &ComputePolicy,
) -> Result<(), String> {
    let path = compute_policy_path(app, release, asset)?;
    let parent = path
        .parent()
        .ok_or_else(|| "Invalid Stable Diffusion compute policy path.".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("Failed to create the compute policy directory: {error}"))?;
    let bytes = serde_json::to_vec_pretty(policy)
        .map_err(|error| format!("Failed to serialize the compute policy: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("Failed to write the compute policy: {error}"))?;
    std::fs::rename(&temporary, &path)
        .map_err(|error| format!("Failed to finalize the compute policy: {error}"))
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

#[cfg(test)]
mod tests {
    use super::{
        build_generation_payload, compute_auto_fit_estimate, devices_for_policy,
        manual_backend_specs, max_vram_spec, migrate_legacy_compute_policy,
        validate_compute_policy, ComputePolicy, EstimateComponent, LegacyComputePolicy,
        RunnabilityDevice, RunnabilityEstimate, RunnabilityPlacement, SdGenerationPayload, MIB,
    };
    use std::collections::BTreeMap;

    fn gib(value: u64) -> u64 {
        value * 1024 * MIB
    }

    fn device(name: &str, budget_gib: u64) -> RunnabilityDevice {
        let id = name
            .chars()
            .filter(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .unwrap_or(0);
        RunnabilityDevice {
            id,
            name: name.to_string(),
            description: name.to_string(),
            total_bytes: gib(budget_gib + 1),
            free_bytes: gib(budget_gib) + 512 * MIB,
            budget_bytes: gib(budget_gib),
        }
    }

    fn component(
        name: &'static str,
        params_gib: u64,
        reserve_gib: u64,
        splittable: bool,
    ) -> EstimateComponent {
        EstimateComponent {
            name,
            params_bytes: gib(params_gib),
            compute_reserve_bytes: gib(reserve_gib),
            splittable,
        }
    }

    #[test]
    fn auto_fit_estimate_uses_concurrent_placement_when_everything_fits_together() {
        let components = vec![
            component("DiT", 3, 2, true),
            component("VAE", 1, 1, false),
            component("Conditioner", 2, 2, true),
        ];
        let estimate =
            compute_auto_fit_estimate(&components, vec![device("Vulkan0", 8)], None, "test");

        assert_eq!(estimate.plan_mode, "concurrent");
        assert!(estimate
            .placements
            .iter()
            .all(|placement| placement.targets == ["Vulkan0"] && !placement.cpu));
    }

    #[test]
    fn auto_fit_estimate_time_shares_components_that_fit_individually() {
        let components = vec![
            component("DiT", 3, 2, true),
            component("VAE", 1, 1, false),
            component("Conditioner", 2, 2, true),
        ];
        let estimate =
            compute_auto_fit_estimate(&components, vec![device("Vulkan0", 6)], None, "test");

        assert_eq!(estimate.plan_mode, "timeShare");
        assert!(estimate
            .placements
            .iter()
            .all(|placement| placement.targets == ["Vulkan0"] && !placement.cpu));
    }

    #[test]
    fn auto_fit_estimate_splits_only_splittable_components() {
        let components = vec![component("DiT", 7, 2, true), component("VAE", 6, 1, false)];
        let estimate = compute_auto_fit_estimate(
            &components,
            vec![device("Vulkan0", 6), device("Vulkan1", 6)],
            None,
            "test",
        );

        let dit = estimate
            .placements
            .iter()
            .find(|placement| placement.component == "DiT")
            .unwrap();
        let vae = estimate
            .placements
            .iter()
            .find(|placement| placement.component == "VAE")
            .unwrap();
        assert!(dit.split);
        assert_eq!(dit.targets, ["Vulkan0", "Vulkan1"]);
        assert!(vae.cpu);
        assert_eq!(vae.targets, ["CPU"]);
    }

    #[test]
    fn auto_fit_estimate_matches_upstream_default_backend_without_a_gpu() {
        let components = vec![component("DiT", 3, 2, true)];
        let estimate = compute_auto_fit_estimate(&components, Vec::new(), Some(gib(16)), "test");

        assert_eq!(estimate.plan_mode, "defaultBackend");
        assert!(estimate.placements[0].cpu);
        assert_eq!(estimate.placements[0].targets, ["CPU"]);
    }

    #[test]
    fn compute_policy_validates_manual_device_counts_and_split_support() {
        let devices = vec![device("Vulkan0", 8), device("Vulkan1", 8)];
        let conflicting_modes = ComputePolicy {
            multi_gpu_enabled: true,
            gpu_device_ids: vec![0, 1],
            single_gpu_device_id: Some(0),
            device_budgets_gib: BTreeMap::new(),
            split_mode: "layer".to_string(),
        };
        assert!(validate_compute_policy(&conflicting_modes, "vulkan", &devices).is_err());

        let multi_with_one_device = ComputePolicy {
            multi_gpu_enabled: true,
            gpu_device_ids: vec![0],
            single_gpu_device_id: None,
            device_budgets_gib: BTreeMap::new(),
            split_mode: "layer".to_string(),
        };
        assert!(validate_compute_policy(&multi_with_one_device, "vulkan", &devices).is_err());

        let row_split = ComputePolicy {
            multi_gpu_enabled: true,
            gpu_device_ids: vec![0, 1],
            single_gpu_device_id: None,
            device_budgets_gib: BTreeMap::new(),
            split_mode: "row".to_string(),
        };
        assert!(validate_compute_policy(&row_split, "vulkan", &devices).is_err());
        assert!(validate_compute_policy(&row_split, "cuda", &devices).is_ok());
    }

    #[test]
    fn compute_policy_filters_devices_and_applies_per_device_budgets() {
        let policy = ComputePolicy {
            multi_gpu_enabled: true,
            gpu_device_ids: vec![0, 2],
            single_gpu_device_id: None,
            device_budgets_gib: BTreeMap::from([(0, 4.5)]),
            split_mode: "layer".to_string(),
        };
        let selected = devices_for_policy(
            &policy,
            vec![
                device("Vulkan0", 8),
                device("Vulkan1", 8),
                device("Vulkan2", 8),
            ],
        );

        assert_eq!(
            selected
                .iter()
                .map(|device| device.name.as_str())
                .collect::<Vec<_>>(),
            ["Vulkan0", "Vulkan2"]
        );
        assert_eq!(
            selected[0].budget_bytes,
            (4.5 * 1024_f64.powi(3)).round() as u64
        );
        assert_eq!(selected[1].budget_bytes, gib(8));
    }

    #[test]
    fn compute_policy_single_gpu_override_uses_the_hardware_device_id() {
        let policy = ComputePolicy {
            multi_gpu_enabled: false,
            gpu_device_ids: vec![0, 2],
            single_gpu_device_id: Some(2),
            device_budgets_gib: BTreeMap::new(),
            split_mode: "layer".to_string(),
        };
        let selected = devices_for_policy(
            &policy,
            vec![
                device("Vulkan0", 8),
                device("Vulkan1", 8),
                device("Vulkan2", 8),
            ],
        );

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, 2);
        assert_eq!(selected[0].name, "Vulkan2");
    }

    #[test]
    fn max_vram_uses_runtime_names_for_hardware_id_budgets() {
        let policy = ComputePolicy {
            multi_gpu_enabled: true,
            gpu_device_ids: vec![0, 2],
            single_gpu_device_id: None,
            device_budgets_gib: BTreeMap::from([(0, 4.5), (2, 7.0)]),
            split_mode: "layer".to_string(),
        };
        let devices = devices_for_policy(
            &policy,
            vec![device("CUDA0", 8), device("CUDA1", 8), device("CUDA2", 8)],
        );

        assert_eq!(
            max_vram_spec(&policy, &devices).as_deref(),
            Some("cuda0=4.5,cuda2=7")
        );
    }

    #[test]
    fn legacy_name_based_policy_migrates_to_hardware_device_ids() {
        let migrated = migrate_legacy_compute_policy(LegacyComputePolicy {
            mode: "multi".to_string(),
            selected_devices: vec!["Vulkan0".to_string(), "Vulkan2".to_string()],
            device_budgets_gib: BTreeMap::from([("Vulkan2".to_string(), 6.5)]),
            split_mode: "layer".to_string(),
        });

        assert!(migrated.multi_gpu_enabled);
        assert_eq!(migrated.gpu_device_ids, [0, 2]);
        assert_eq!(migrated.single_gpu_device_id, None);
        assert_eq!(migrated.device_budgets_gib.get(&2), Some(&6.5));
    }

    #[test]
    fn manual_backend_specs_preserve_split_targets_and_time_shared_parameters() {
        let estimate = RunnabilityEstimate {
            model_bytes: gib(8),
            available_ram_bytes: Some(gib(16)),
            plan_mode: "timeShare",
            device_source: "test",
            devices: vec![device("Vulkan0", 6), device("Vulkan1", 6)],
            placements: vec![
                RunnabilityPlacement {
                    component: "DiT",
                    params_bytes: gib(7),
                    compute_reserve_bytes: gib(2),
                    targets: vec!["Vulkan0".to_string(), "Vulkan1".to_string()],
                    cpu: false,
                    split: true,
                },
                RunnabilityPlacement {
                    component: "VAE",
                    params_bytes: gib(1),
                    compute_reserve_bytes: gib(1),
                    targets: vec!["CPU".to_string()],
                    cpu: true,
                    split: false,
                },
            ],
        };

        let (backend, params_backend) = manual_backend_specs(&estimate);
        assert_eq!(backend, "diffusion=Vulkan0&Vulkan1,vae=cpu");
        assert_eq!(params_backend.as_deref(), Some("diffusion=disk"));
    }

    #[test]
    fn generation_payload_preserves_every_memory_relevant_request_field() {
        let references = vec!["data:image/png;base64,reference".to_string()];
        let loras = vec![serde_json::json!({
            "path": "style.safetensors",
            "multiplier": 0.75,
            "is_high_noise": false
        })];
        let payload = build_generation_payload(SdGenerationPayload {
            prompt: "a detailed prompt",
            negative_prompt: "blur",
            width: 1280,
            height: 768,
            seed: 42,
            batch_count: 2,
            references: &references,
            sample_method: "dpm++2m",
            sample_steps: 24,
            cfg: 3.5,
            loras: &loras,
        });

        assert_eq!(payload["prompt"], "a detailed prompt");
        assert_eq!(payload["negative_prompt"], "blur");
        assert_eq!(payload["width"], 1280);
        assert_eq!(payload["height"], 768);
        assert_eq!(payload["seed"], 42);
        assert_eq!(payload["batch_count"], 2);
        assert_eq!(payload["ref_images"], serde_json::json!(references));
        assert_eq!(payload["sample_params"]["sample_method"], "dpm++2m");
        assert_eq!(payload["sample_params"]["sample_steps"], 24);
        assert_eq!(payload["sample_params"]["guidance"]["txt_cfg"], 3.5);
        assert_eq!(payload["lora"], serde_json::json!(loras));
        assert_eq!(payload["vae_tiling_params"]["enabled"], true);
        assert_eq!(payload["output_compression"], 100);
    }
}
