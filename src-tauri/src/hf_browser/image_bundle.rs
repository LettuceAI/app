use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, RANGE};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::Mutex;

use crate::image_generator::sdcpp::{
    self, HfBundleRegistration, RemoteBundleRunnabilityRequest,
};

const TOKEN_META_KEY: &str = "hugging_face_access_token";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfAuthStatus {
    saved: bool,
    valid: bool,
    username: Option<String>,
    error_kind: Option<&'static str>,
}

#[derive(Debug, Deserialize)]
struct WhoAmI {
    name: Option<String>,
}

pub(super) fn hf_token(app: &AppHandle) -> Option<String> {
    let conn = crate::storage_manager::db::open_db(app).ok()?;
    conn.query_row(
        "SELECT value FROM meta WHERE key = ?1",
        params![TOKEN_META_KEY],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
    .filter(|token| !token.trim().is_empty())
}

pub(super) fn authenticated_client(app: &AppHandle) -> Result<reqwest::Client, String> {
    let mut headers = HeaderMap::new();
    if let Some(token) = hf_token(app) {
        let value = HeaderValue::from_str(&format!("Bearer {}", token.trim()))
            .map_err(|_| "The saved Hugging Face token is malformed.".to_string())?;
        headers.insert(AUTHORIZATION, value);
    }
    reqwest::Client::builder()
        .user_agent("LettuceAI/1.0")
        .default_headers(headers)
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|error| format!("Failed to build Hugging Face client: {error}"))
}

async fn validate_token(token: &str) -> Result<String, String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("Enter a Hugging Face token.".to_string());
    }
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| "The Hugging Face token is malformed.".to_string())?,
    );
    let response = reqwest::Client::builder()
        .user_agent("LettuceAI/1.0")
        .default_headers(headers)
        .build()
        .map_err(|error| format!("Failed to create the token validation client: {error}"))?
        .get("https://huggingface.co/api/whoami-v2")
        .send()
        .await
        .map_err(|error| format!("Could not validate the Hugging Face token: {error}"))?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("The Hugging Face token is invalid or expired.".to_string());
    }
    if !response.status().is_success() {
        return Err(format!(
            "Hugging Face token validation failed with status {}.",
            response.status()
        ));
    }
    response
        .json::<WhoAmI>()
        .await
        .map_err(|error| format!("Could not read the Hugging Face account: {error}"))?
        .name
        .ok_or_else(|| "Hugging Face did not return an account name.".to_string())
}

#[tauri::command]
pub async fn hf_auth_status(app: AppHandle) -> Result<HfAuthStatus, String> {
    let Some(token) = hf_token(&app) else {
        return Ok(HfAuthStatus {
            saved: false,
            valid: false,
            username: None,
            error_kind: Some("missingToken"),
        });
    };
    match validate_token(&token).await {
        Ok(username) => Ok(HfAuthStatus {
            saved: true,
            valid: true,
            username: Some(username),
            error_kind: None,
        }),
        Err(_) => Ok(HfAuthStatus {
            saved: true,
            valid: false,
            username: None,
            error_kind: Some("invalidOrExpired"),
        }),
    }
}

#[tauri::command]
pub async fn hf_auth_save(app: AppHandle, token: String) -> Result<HfAuthStatus, String> {
    let username = validate_token(&token).await?;
    let conn = crate::storage_manager::db::open_db(&app)?;
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![TOKEN_META_KEY, token.trim()],
    )
    .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?;
    Ok(HfAuthStatus {
        saved: true,
        valid: true,
        username: Some(username),
        error_kind: None,
    })
}

#[tauri::command]
pub fn hf_auth_clear(app: AppHandle) -> Result<(), String> {
    let conn = crate::storage_manager::db::open_db(&app)?;
    conn.execute("DELETE FROM meta WHERE key = ?1", params![TOKEN_META_KEY])
        .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?;
    Ok(())
}

#[tauri::command]
pub fn hf_image_bundle_profiles() -> Vec<sdcpp::HfBundleProfile> {
    sdcpp::hf_bundle_profiles()
}

#[derive(Debug, Deserialize)]
struct ApiModel {
    #[serde(rename = "modelId")]
    model_id: String,
    #[serde(default)]
    sha: Option<String>,
    #[serde(default)]
    gated: Value,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default, rename = "pipeline_tag")]
    pipeline_tag: Option<String>,
    #[serde(default, rename = "cardData")]
    card_data: Value,
    #[serde(default)]
    likes: i64,
    #[serde(default)]
    downloads: i64,
    #[serde(default, rename = "lastModified")]
    last_modified: Option<String>,
    #[serde(default, rename = "trendingScore")]
    trending_score: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct TreeEntry {
    #[serde(rename = "type")]
    entry_type: String,
    path: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    lfs: Option<LfsInfo>,
}

#[derive(Debug, Deserialize)]
struct LfsInfo {
    #[serde(default)]
    oid: Option<String>,
    #[serde(default)]
    size: u64,
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
                .map(str::to_string)
                .collect(),
            _ => Vec::new(),
        })
        .collect()
}

fn repo_compatibility(profile: &sdcpp::HfBundleProfile, detail: &ApiModel) -> (bool, String) {
    let mut evidence = vec![detail.model_id.to_ascii_lowercase()];
    evidence.extend(detail.tags.iter().map(|value| value.to_ascii_lowercase()));
    evidence.extend(
        ancestry_values(&detail.card_data)
            .into_iter()
            .map(|value| value.to_ascii_lowercase()),
    );
    let joined = evidence.join(" ");
    let explicitly_wrong_variant = match profile.id {
        "z-image" => joined.contains("z-image-turbo") || joined.contains("z_image_turbo"),
        "flux-2-klein-9b" => joined.contains("klein-base-9b"),
        "krea-2-turbo" => joined.contains("krea-2-raw") || joined.contains("krea-2-base"),
        "krea-2-raw" => joined.contains("krea-2-turbo"),
        _ => false,
    };
    if explicitly_wrong_variant {
        return (false, "The repository belongs to a different variant of this model family.".to_string());
    }
    if profile
        .diffusion_markers
        .iter()
        .any(|marker| evidence.iter().any(|value| value.contains(marker)))
    {
        return (true, "Architecture or declared base-model ancestry matches this recipe.".to_string());
    }
    (
        false,
        "This repository does not declare compatible architecture or base-model ancestry."
            .to_string(),
    )
}

async fn fetch_detail(client: &reqwest::Client, model_id: &str) -> Result<ApiModel, String> {
    let response = client
        .get(format!("https://huggingface.co/api/models/{model_id}"))
        .send()
        .await
        .map_err(|error| format!("Failed to fetch Hugging Face model metadata: {error}"))?;
    match response.status() {
        reqwest::StatusCode::UNAUTHORIZED => {
            Err("Hugging Face authentication is missing or invalid.".to_string())
        }
        reqwest::StatusCode::FORBIDDEN => Err(
            "This gated model requires accepting its license on Hugging Face before retrying."
                .to_string(),
        ),
        status if !status.is_success() => Err(format!(
            "Hugging Face model lookup failed with status {status}."
        )),
        _ => response
            .json::<ApiModel>()
            .await
            .map_err(|error| format!("Failed to parse Hugging Face model metadata: {error}")),
    }
}

fn parse_model_id(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_end_matches('/');
    let path = trimmed
        .strip_prefix("https://huggingface.co/")
        .unwrap_or(trimmed)
        .split(['?', '#'])
        .next()?;
    let parts = path.split('/').filter(|part| !part.is_empty()).collect::<Vec<_>>();
    (parts.len() >= 2).then(|| format!("{}/{}", parts[0], parts[1]))
}

fn to_search_result(detail: ApiModel) -> super::HfSearchResult {
    super::HfSearchResult {
        author: detail.model_id.split('/').next().unwrap_or("unknown").to_string(),
        likes: detail.likes,
        downloads: detail.downloads,
        tags: detail.tags,
        pipeline_tag: detail.pipeline_tag,
        last_modified: detail.last_modified,
        trending_score: detail.trending_score,
        model_id: detail.model_id,
    }
}

fn vae_family_term(profile: &sdcpp::HfBundleProfile) -> &'static str {
    if profile.id.starts_with("krea") {
        "wan"
    } else if profile.id == "qwen-image-edit-2511" {
        "qwen"
    } else {
        "flux"
    }
}

fn role_default_query(profile: &sdcpp::HfBundleProfile, role: &str) -> String {
    match role {
        "diffusion_model" => profile.diffusion_markers.first().copied().unwrap_or_default().to_string(),
        "text_encoder" | "vision_encoder" => {
            profile.encoder_markers.first().copied().unwrap_or_default().to_string()
        }
        _ => format!("{} vae", vae_family_term(profile)),
    }
}

fn role_search_matches(
    profile: &sdcpp::HfBundleProfile,
    role: &str,
    model_id: &str,
    tags: &[String],
) -> bool {
    let mut evidence = vec![model_id.to_ascii_lowercase()];
    evidence.extend(tags.iter().map(|tag| tag.to_ascii_lowercase()));
    let matches_any =
        |markers: &[&str]| markers.iter().any(|marker| evidence.iter().any(|value| value.contains(marker)));
    match role {
        "diffusion_model" => matches_any(&profile.diffusion_markers),
        "text_encoder" => matches_any(&profile.encoder_markers),
        "vision_encoder" => matches_any(&profile.encoder_markers) || matches_any(&["mmproj"]),
        _ => matches_any(&["vae", vae_family_term(profile), "ae."]),
    }
}

pub(crate) async fn bundle_role_search(
    app: &AppHandle,
    profile_id: &str,
    role: &str,
    query: &str,
    sort_field: &str,
    author: Option<&str>,
) -> Result<Vec<super::HfSearchResult>, String> {
    let profile = sdcpp::hf_bundle_profile(profile_id)?;
    if !profile.required_roles.contains(&role) {
        return Err(format!("{role} is not used by the selected architecture."));
    }
    if query.contains('/') && !query.contains(char::is_whitespace) {
        if let Some(model_id) = parse_model_id(query) {
            let client = authenticated_client(app)?;
            let detail = fetch_detail(&client, &model_id).await?;
            if role == "diffusion_model" {
                let (compatible, reason) = repo_compatibility(&profile, &detail);
                if !compatible {
                    return Err(reason);
                }
            }
            return Ok(vec![to_search_result(detail)]);
        }
    }
    let effective_query = if query.trim().is_empty() {
        role_default_query(&profile, role)
    } else {
        query.trim().to_string()
    };
    let mut base = format!("https://huggingface.co/api/models?limit=100&sort={sort_field}&direction=-1");
    if let Some(author) = author.map(str::trim).filter(|author| !author.is_empty()) {
        base.push_str(&format!("&author={}", urlencoding::encode(author)));
    }
    if !effective_query.is_empty() {
        base.push_str(&format!("&search={}", urlencoding::encode(&effective_query)));
    }
    let format_filter = if role == "vae" { "safetensors" } else { "gguf" };
    let mut results = super::fetch_models_list(app, &format!("{base}&filter={format_filter}")).await?;
    if results.is_empty() && role == "vae" {
        results = super::fetch_models_list(app, &base).await?;
    }
    let (matched, unmatched): (Vec<_>, Vec<_>) = results
        .into_iter()
        .partition(|result| role_search_matches(&profile, role, &result.model_id, &result.tags));
    if role == "diffusion_model" {
        return Ok(matched);
    }
    Ok(if matched.is_empty() { unmatched } else { matched })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatedAsset {
    selection_id: String,
    profile_id: String,
    role: String,
    model_id: String,
    revision: String,
    relative_path: String,
    format: String,
    quantization: Option<String>,
    size: u64,
    sha256: String,
    architecture: Option<String>,
    gated: bool,
}

lazy_static::lazy_static! {
    static ref VALIDATED_ASSETS: Arc<Mutex<HashMap<String, ValidatedAsset>>> =
        Arc::new(Mutex::new(HashMap::new()));
    static ref MANIFEST_LOCK: Arc<Mutex<()>> = Arc::new(Mutex::new(()));
}

fn safe_relative_path(path: &str) -> bool {
    let path = Path::new(path);
    !path.is_absolute()
        && path.components().all(|component| matches!(component, Component::Normal(_)))
}

fn excluded_artifact(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    [
        "lora", "adapter", "optimizer", "training", "checkpoint", "global_step",
        "scheduler", "random_state", ".ckpt", ".pt", ".pth",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn format_of(path: &str) -> Option<&'static str> {
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

fn quantization(path: &str) -> Option<String> {
    let upper = path.to_ascii_uppercase();
    ["Q2_K", "Q3_K_S", "Q3_K_M", "Q3_K", "Q4_0", "Q4_K_M", "Q5_K_M", "Q6_K", "Q8_0", "BF16", "F16"]
        .into_iter()
        .find(|value| upper.contains(value))
        .map(str::to_string)
}

fn role_compatible(profile: &sdcpp::HfBundleProfile, role: &str, model_id: &str, path: &str) -> bool {
    let evidence = format!("{} {}", model_id, path).to_ascii_lowercase();
    match role {
        "diffusion_model" => profile.diffusion_markers.iter().any(|marker| evidence.contains(marker)),
        "text_encoder" => profile.encoder_markers.iter().any(|marker| evidence.contains(marker)),
        "vision_encoder" => profile.id == "qwen-image-edit-2511"
            && (evidence.contains("mmproj") || evidence.contains("vision")),
        "vae" if profile.id.starts_with("krea") => evidence.contains("wan") && evidence.contains("vae"),
        "vae" if profile.id == "qwen-image-edit-2511" => evidence.contains("qwen") && evidence.contains("vae"),
        "vae" => (evidence.contains("flux") || evidence.contains("ae.")) && (evidence.contains("vae") || evidence.contains("ae.")),
        _ => false,
    }
}

async fn gguf_architecture_hint(
    client: &reqwest::Client,
    model_id: &str,
    revision: &str,
    path: &str,
) -> Option<String> {
    let response = client
        .get(format!("https://huggingface.co/{model_id}/resolve/{revision}/{path}"))
        .header(RANGE, "bytes=0-1048575")
        .send()
        .await
        .ok()?;
    if !(response.status().is_success() || response.status() == reqwest::StatusCode::PARTIAL_CONTENT) {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    let text = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
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
        .map(str::to_string)
}

async fn recursive_tree(
    client: &reqwest::Client,
    model_id: &str,
    revision: &str,
) -> Result<Vec<TreeEntry>, String> {
    let response = client
        .get(format!(
            "https://huggingface.co/api/models/{model_id}/tree/{revision}?recursive=true"
        ))
        .send()
        .await
        .map_err(|error| format!("Failed to fetch the recursive repository tree: {error}"))?;
    if response.status() == reqwest::StatusCode::FORBIDDEN {
        return Err("Accept this model's license on Hugging Face, then retry.".to_string());
    }
    if !response.status().is_success() {
        return Err(format!("Repository tree lookup failed with status {}.", response.status()));
    }
    response
        .json::<Vec<TreeEntry>>()
        .await
        .map_err(|error| format!("Failed to parse the repository tree: {error}"))
}

#[tauri::command]
pub async fn hf_image_bundle_files(
    app: AppHandle,
    profile_id: String,
    model_id: String,
    role: String,
) -> Result<Vec<ValidatedAsset>, String> {
    let profile = sdcpp::hf_bundle_profile(&profile_id)?;
    if !profile.required_roles.contains(&role.as_str()) {
        return Err(format!("{role} is not used by the selected architecture."));
    }
    let model_id = parse_model_id(&model_id).ok_or_else(|| "Enter a valid Hugging Face model ID or URL.".to_string())?;
    let client = authenticated_client(&app)?;
    let detail = fetch_detail(&client, &model_id).await?;
    let revision = detail
        .sha
        .clone()
        .filter(|sha| sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| "Hugging Face did not return an immutable commit revision.".to_string())?;
    if role == "diffusion_model" && !repo_compatibility(&profile, &detail).0 {
        return Err("The selected diffusion repository is not compatible with this architecture recipe.".to_string());
    }
    let gated = !detail.gated.is_null() && detail.gated != Value::Bool(false);
    let tree = recursive_tree(&client, &model_id, &revision).await?;
    let mut prevalidated = Vec::new();
    for entry in tree {
        if entry.entry_type != "file" || !safe_relative_path(&entry.path) || excluded_artifact(&entry.path) {
            continue;
        }
        let Some(format) = format_of(&entry.path) else { continue };
        if role != "diffusion_model" && !role_compatible(&profile, &role, &model_id, &entry.path) {
            continue;
        }
        let size = entry.lfs.as_ref().map(|lfs| lfs.size).unwrap_or(entry.size);
        let sha256 = entry
            .lfs
            .as_ref()
            .and_then(|lfs| lfs.oid.clone())
            .filter(|oid| oid.len() == 64 && oid.bytes().all(|byte| byte.is_ascii_hexdigit()));
        let Some(sha256) = sha256 else { continue };
        if size == 0 { continue }
        prevalidated.push((entry.path, format, size, sha256));
    }
    let architectures: Vec<Option<String>> = if role == "text_encoder" {
        let mut hint_futures = Vec::new();
        for (path, format, _, _) in &prevalidated {
            let client = &client;
            let model_id = model_id.as_str();
            let revision = revision.as_str();
            let path = path.as_str();
            let is_gguf = *format == "gguf";
            hint_futures.push(async move {
                if is_gguf {
                    gguf_architecture_hint(client, model_id, revision, path).await
                } else {
                    None
                }
            });
        }
        futures_util::stream::iter(hint_futures)
            .buffered(6)
            .collect()
            .await
    } else {
        vec![None; prevalidated.len()]
    };
    let mut candidates = Vec::new();
    for ((asset_path, format, size, sha256), architecture) in
        prevalidated.into_iter().zip(architectures)
    {
        if role == "text_encoder"
            && architecture.as_ref().is_some_and(|architecture| {
                !profile
                    .encoder_markers
                    .iter()
                    .any(|marker| architecture.contains(marker))
            })
        {
            continue;
        }
        let asset = ValidatedAsset {
            selection_id: uuid::Uuid::new_v4().to_string(),
            profile_id: profile_id.clone(),
            role: role.clone(),
            model_id: model_id.clone(),
            revision: revision.clone(),
            relative_path: asset_path.clone(),
            format: format.to_string(),
            quantization: quantization(&asset_path),
            size,
            sha256,
            architecture,
            gated,
        };
        VALIDATED_ASSETS.lock().await.insert(asset.selection_id.clone(), asset.clone());
        candidates.push(asset);
    }
    candidates.sort_by_key(|asset| asset.size);
    Ok(candidates)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueBundleRequest {
    profile_id: String,
    display_name: String,
    runtime_release: String,
    runtime_asset: String,
    selection_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueBundleResult {
    bundle_id: String,
    queue_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestAsset {
    #[serde(flatten)]
    asset: ValidatedAsset,
    local_path: String,
    verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundleManifest {
    bundle_id: String,
    profile_id: String,
    display_name: String,
    runtime_release: String,
    runtime_asset: String,
    runnability: Option<Value>,
    assets: Vec<ManifestAsset>,
    registration_state: String,
    model_id: Option<String>,
    setup_error: Option<String>,
}

fn bundle_root(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(crate::utils::lettuce_dir(app)?
        .join("models")
        .join("image")
        .join("huggingface"))
}

fn manifest_path(app: &AppHandle, bundle_id: &str) -> Result<PathBuf, String> {
    Ok(bundle_root(app)?.join("bundles").join(format!("{bundle_id}.json")))
}

fn write_manifest(app: &AppHandle, manifest: &BundleManifest) -> Result<(), String> {
    let path = manifest_path(app, &manifest.bundle_id)?;
    let parent = path.parent().ok_or_else(|| "Invalid bundle manifest path.".to_string())?;
    std::fs::create_dir_all(parent).map_err(|error| format!("Failed to create the bundle manifest directory: {error}"))?;
    let temp = path.with_extension("json.tmp");
    std::fs::write(
        &temp,
        serde_json::to_vec_pretty(manifest).map_err(|error| format!("Failed to serialize the bundle manifest: {error}"))?,
    )
    .map_err(|error| format!("Failed to write the bundle manifest: {error}"))?;
    std::fs::rename(temp, path).map_err(|error| format!("Failed to finalize the bundle manifest: {error}"))
}

fn read_manifest(app: &AppHandle, bundle_id: &str) -> Result<BundleManifest, String> {
    serde_json::from_slice(
        &std::fs::read(manifest_path(app, bundle_id)?)
            .map_err(|error| format!("Failed to read the bundle manifest: {error}"))?,
    )
    .map_err(|error| format!("Failed to parse the bundle manifest: {error}"))
}

fn reuse_verified_asset(app: &AppHandle, asset: &ManifestAsset) {
    let Ok(directory) = manifest_path(app, "placeholder").and_then(|path| {
        path.parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "Invalid bundle manifest directory.".to_string())
    }) else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(directory) else { return };
    let source = entries.flatten().find_map(|entry| {
        let bytes = std::fs::read(entry.path()).ok()?;
        let manifest = serde_json::from_slice::<BundleManifest>(&bytes).ok()?;
        manifest
            .assets
            .into_iter()
            .find(|candidate| {
                candidate.verified
                    && candidate.asset.sha256 == asset.asset.sha256
                    && Path::new(&candidate.local_path).is_file()
            })
            .map(|candidate| candidate.local_path)
    });
    let Some(source) = source else { return };
    let destination = Path::new(&asset.local_path);
    if destination.exists() {
        return;
    }
    if let Some(parent) = destination.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::hard_link(source, destination);
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleStatus {
    bundle_id: String,
    registration_state: String,
    model_id: Option<String>,
    setup_error: Option<String>,
}

#[tauri::command]
pub fn hf_image_bundle_status(app: AppHandle, bundle_id: String) -> Result<BundleStatus, String> {
    let manifest = read_manifest(&app, &bundle_id)?;
    Ok(BundleStatus {
        bundle_id: manifest.bundle_id,
        registration_state: manifest.registration_state,
        model_id: manifest.model_id,
        setup_error: manifest.setup_error,
    })
}

async fn refetch_asset(app: &AppHandle, asset: &ValidatedAsset) -> Result<ValidatedAsset, String> {
    let client = authenticated_client(app)?;
    let detail = fetch_detail(&client, &asset.model_id).await?;
    if detail.sha.as_deref() != Some(asset.revision.as_str()) {
        return Err("The repository revision changed after selection. Select the component again.".to_string());
    }
    let tree = recursive_tree(&client, &asset.model_id, &asset.revision).await?;
    let entry = tree
        .into_iter()
        .find(|entry| entry.entry_type == "file" && entry.path == asset.relative_path)
        .ok_or_else(|| "A selected component no longer exists at the pinned revision.".to_string())?;
    let size = entry.lfs.as_ref().map(|lfs| lfs.size).unwrap_or(entry.size);
    let sha256 = entry.lfs.and_then(|lfs| lfs.oid).unwrap_or_default();
    if size != asset.size || sha256 != asset.sha256 {
        return Err("A selected component no longer matches its validated size and hash.".to_string());
    }
    Ok(asset.clone())
}

fn asset_destination(app: &AppHandle, asset: &ValidatedAsset) -> Result<PathBuf, String> {
    let mut parts = asset.model_id.split('/');
    let author = parts.next().ok_or_else(|| "Invalid Hugging Face author.".to_string())?;
    let repository = parts.next().ok_or_else(|| "Invalid Hugging Face repository.".to_string())?;
    if parts.next().is_some()
        || !safe_relative_path(author)
        || !safe_relative_path(repository)
        || !safe_relative_path(&asset.revision)
        || !safe_relative_path(&asset.relative_path)
    {
        return Err("The selected repository contains an unsafe destination path.".to_string());
    }
    Ok(bundle_root(app)?
        .join(author)
        .join(repository)
        .join(&asset.revision)
        .join(&asset.relative_path))
}

#[tauri::command]
pub async fn hf_queue_image_bundle(
    app: AppHandle,
    request: QueueBundleRequest,
) -> Result<QueueBundleResult, String> {
    let profile = sdcpp::hf_bundle_profile(&request.profile_id)?;
    let display_name = request.display_name.trim();
    if display_name.is_empty() {
        return Err("Enter a display name for the model.".to_string());
    }
    if request.selection_ids.is_empty() {
        return Err("Select every required bundle component before downloading.".to_string());
    }
    let selected = VALIDATED_ASSETS.lock().await;
    let mut assets = Vec::new();
    for selection_id in &request.selection_ids {
        let asset = selected
            .get(selection_id)
            .filter(|asset| asset.profile_id == request.profile_id)
            .cloned()
            .ok_or_else(|| "A component selection expired. Select it again before downloading.".to_string())?;
        assets.push(asset);
    }
    drop(selected);
    let roles = assets.iter().map(|asset| asset.role.as_str()).collect::<HashSet<_>>();
    for role in &profile.required_roles {
        if !roles.contains(role) {
            return Err(format!("The required {role} component is missing."));
        }
    }
    if roles.len() != assets.len() {
        return Err("Select exactly one component for each bundle role.".to_string());
    }
    let size_for = |role: &str| {
        assets
            .iter()
            .find(|asset| asset.role == role)
            .map(|asset| asset.size)
            .unwrap_or(0)
    };
    let backend_estimate = sdcpp::sdcpp_estimate_remote_bundle(
        app.clone(),
        RemoteBundleRunnabilityRequest {
            profile_id: request.profile_id.clone(),
            runtime_release: request.runtime_release.clone(),
            runtime_asset: request.runtime_asset.clone(),
            diffusion_bytes: size_for("diffusion_model"),
            text_encoder_bytes: size_for("text_encoder"),
            vae_bytes: size_for("vae"),
            vision_encoder_bytes: size_for("vision_encoder"),
        },
    )
    .await?;
    if matches!(backend_estimate.status.as_str(), "notInstalled" | "incompatibleRuntime") {
        return Err("Install or select a compatible stable-diffusion.cpp engine before downloading this bundle.".to_string());
    }
    let mut refreshed = Vec::new();
    for asset in assets {
        if asset.role != "diffusion_model"
            && !role_compatible(&profile, &asset.role, &asset.model_id, &asset.relative_path)
        {
            return Err(format!("The selected {} is not compatible with {}.", asset.role, profile.display_name));
        }
        refreshed.push(refetch_asset(&app, &asset).await?);
    }
    let bundle_id = uuid::Uuid::new_v4().to_string();
    let mut manifest = BundleManifest {
        bundle_id: bundle_id.clone(),
        profile_id: request.profile_id.clone(),
        display_name: display_name.to_string(),
        runtime_release: request.runtime_release.clone(),
        runtime_asset: request.runtime_asset.clone(),
        runnability: Some(serde_json::to_value(&backend_estimate).map_err(|error| format!("Failed to save the runnability estimate: {error}"))?),
        assets: Vec::new(),
        registration_state: "downloading".to_string(),
        model_id: None,
        setup_error: None,
    };
    for asset in &refreshed {
        manifest.assets.push(ManifestAsset {
            local_path: asset_destination(&app, asset)?.to_string_lossy().to_string(),
            asset: asset.clone(),
            verified: false,
        });
    }
    for asset in &manifest.assets {
        reuse_verified_asset(&app, asset);
    }
    let (active_destinations, active_hashes) = {
        let state = super::HF_DOWNLOAD_QUEUE.lock().await;
        let destinations = state
            .queue
            .iter()
            .filter(|item| matches!(item.status.as_str(), "queued" | "downloading"))
            .filter_map(|item| item.destination_path.clone())
            .collect::<HashSet<_>>();
        let hashes = state
            .queue
            .iter()
            .filter(|item| matches!(item.status.as_str(), "queued" | "downloading"))
            .filter_map(|item| item.sha256.clone())
            .collect::<HashSet<_>>();
        (destinations, hashes)
    };
    for asset in &refreshed {
        let destination = asset_destination(&app, asset)?.to_string_lossy().to_string();
        if active_destinations.contains(&destination) {
            return Err(format!("{} is already being downloaded.", asset.relative_path));
        }
        if active_hashes.contains(&asset.sha256) {
            return Err(format!(
                "An identical copy of {} is already being downloaded. Retry this bundle after it finishes.",
                asset.relative_path
            ));
        }
    }
    write_manifest(&app, &manifest)?;
    let mut queue_ids = Vec::new();
    for asset in refreshed {
        let destination = asset_destination(&app, &asset)?.to_string_lossy().to_string();
        let url = format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            asset.model_id, asset.revision, asset.relative_path
        );
        queue_ids.push(
            super::hf_queue_download(
                app.clone(),
                asset.model_id.clone(),
                asset.relative_path.clone(),
                Some(super::QueueDownloadMetadata {
                    install_id: Some(bundle_id.clone()),
                    display_name: Some(display_name.to_string()),
                    download_role: Some(asset.role.clone()),
                    queue_kind: Some("hf_image".to_string()),
                    asset_root: Some(bundle_root(&app)?.to_string_lossy().to_string()),
                    install_kind: Some(request.profile_id.clone()),
                    download_url: Some(url),
                    destination_path: Some(destination),
                    expected_size: Some(asset.size),
                    sha256: Some(asset.sha256.clone()),
                    runtime_release: Some(request.runtime_release.clone()),
                    runtime_asset: Some(request.runtime_asset.clone()),
                    ..Default::default()
                }),
            )
            .await?,
        );
    }
    Ok(QueueBundleResult { bundle_id, queue_ids })
}

fn register_manifest(app: &AppHandle, manifest: &mut BundleManifest) -> Result<String, String> {
    let by_role = manifest
        .assets
        .iter()
        .map(|asset| (asset.asset.role.as_str(), asset.local_path.as_str()))
        .collect::<HashMap<_, _>>();
    sdcpp::register_hf_bundle_model(
        app,
        HfBundleRegistration {
            profile_id: &manifest.profile_id,
            display_name: &manifest.display_name,
            diffusion_path: by_role.get("diffusion_model").copied().ok_or_else(|| "The diffusion model is missing.".to_string())?,
            text_encoder_path: by_role.get("text_encoder").copied().ok_or_else(|| "The text encoder is missing.".to_string())?,
            vae_path: by_role.get("vae").copied().ok_or_else(|| "The VAE is missing.".to_string())?,
            vision_encoder_path: by_role.get("vision_encoder").copied(),
            runtime_release: &manifest.runtime_release,
            runtime_asset: &manifest.runtime_asset,
        },
    )
}

pub(super) async fn handle_download_completed(
    app: &AppHandle,
    item: &super::QueuedDownload,
    path: &str,
) {
    if item.queue_kind.as_deref() != Some("hf_image") {
        return;
    }
    let Some(bundle_id) = item.install_id.as_deref() else { return };
    let _guard = MANIFEST_LOCK.lock().await;
    let Ok(mut manifest) = read_manifest(app, bundle_id) else { return };
    if let Some(asset) = manifest.assets.iter_mut().find(|asset| asset.local_path == path) {
        asset.verified = true;
    }
    if manifest.assets.iter().all(|asset| asset.verified && Path::new(&asset.local_path).is_file()) {
        match register_manifest(app, &mut manifest) {
            Ok(model_id) => {
                manifest.registration_state = "registered".to_string();
                manifest.model_id = Some(model_id);
                manifest.setup_error = None;
            }
            Err(error) => {
                manifest.registration_state = "setupFailed".to_string();
                manifest.setup_error = Some(error);
            }
        }
    }
    let _ = write_manifest(app, &manifest);
}

#[tauri::command]
pub async fn hf_retry_image_bundle_registration(
    app: AppHandle,
    bundle_id: String,
) -> Result<String, String> {
    let _guard = MANIFEST_LOCK.lock().await;
    let mut manifest = read_manifest(&app, &bundle_id)?;
    if !manifest.assets.iter().all(|asset| asset.verified && Path::new(&asset.local_path).is_file()) {
        return Err("Every bundle component must be verified before model creation can be retried.".to_string());
    }
    match register_manifest(&app, &mut manifest) {
        Ok(model_id) => {
            manifest.registration_state = "registered".to_string();
            manifest.model_id = Some(model_id.clone());
            manifest.setup_error = None;
            write_manifest(&app, &manifest)?;
            Ok(model_id)
        }
        Err(error) => {
            manifest.registration_state = "setupFailed".to_string();
            manifest.setup_error = Some(error.clone());
            write_manifest(&app, &manifest)?;
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn hf_retry_image_bundle_downloads(
    app: AppHandle,
    bundle_id: String,
) -> Result<Vec<String>, String> {
    let _guard = MANIFEST_LOCK.lock().await;
    let mut manifest = read_manifest(&app, &bundle_id)?;
    let mut queue_ids = Vec::new();
    for manifest_asset in &mut manifest.assets {
        if manifest_asset.verified && Path::new(&manifest_asset.local_path).is_file() {
            continue;
        }
        let asset = refetch_asset(&app, &manifest_asset.asset).await?;
        let url = format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            asset.model_id, asset.revision, asset.relative_path
        );
        queue_ids.push(
            super::hf_queue_download(
                app.clone(),
                asset.model_id.clone(),
                asset.relative_path.clone(),
                Some(super::QueueDownloadMetadata {
                    install_id: Some(bundle_id.clone()),
                    display_name: Some(manifest.display_name.clone()),
                    download_role: Some(asset.role.clone()),
                    queue_kind: Some("hf_image".to_string()),
                    asset_root: Some(bundle_root(&app)?.to_string_lossy().to_string()),
                    install_kind: Some(manifest.profile_id.clone()),
                    download_url: Some(url),
                    destination_path: Some(manifest_asset.local_path.clone()),
                    expected_size: Some(asset.size),
                    sha256: Some(asset.sha256.clone()),
                    runtime_release: Some(manifest.runtime_release.clone()),
                    runtime_asset: Some(manifest.runtime_asset.clone()),
                    ..Default::default()
                }),
            )
            .await?,
        );
    }
    manifest.registration_state = "downloading".to_string();
    manifest.setup_error = None;
    write_manifest(&app, &manifest)?;
    Ok(queue_ids)
}

#[cfg(test)]
mod tests {
    use super::{
        excluded_artifact, repo_compatibility, role_compatible, role_search_matches,
        safe_relative_path, ApiModel,
    };
    use crate::image_generator::sdcpp::hf_bundle_profile;
    use serde_json::json;

    #[test]
    fn rejects_traversal_and_training_artifacts() {
        assert!(!safe_relative_path("../model.gguf"));
        assert!(!safe_relative_path("/tmp/model.gguf"));
        assert!(excluded_artifact("optimizer/model.safetensors"));
        assert!(excluded_artifact("flux-lora.safetensors"));
    }

    #[test]
    fn rejects_cross_family_components() {
        let z_image = hf_bundle_profile("z-image-turbo").unwrap();
        assert!(!role_compatible(
            &z_image,
            "text_encoder",
            "unsloth/Qwen3-8B-GGUF",
            "Qwen3-8B-Q4_K_M.gguf"
        ));
        assert!(role_compatible(
            &z_image,
            "text_encoder",
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
            let detail = ApiModel {
                model_id: model_id.to_string(),
                sha: None,
                gated: json!(false),
                tags: Vec::new(),
                pipeline_tag: None,
                card_data: json!({}),
                likes: 0,
                downloads: 0,
                last_modified: None,
                trending_score: None,
            };
            assert!(
                repo_compatibility(&hf_bundle_profile(profile_id).unwrap(), &detail).0,
                "{profile_id} did not classify {model_id}"
            );
        }
    }

    #[test]
    fn accepts_declared_fine_tune_ancestry_but_rejects_cross_family_ancestry() {
        let profile = hf_bundle_profile("flux-2-klein-4b").unwrap();
        let fine_tune = |base_model: &str| ApiModel {
            model_id: "artist/custom-style".to_string(),
            sha: None,
            gated: json!(false),
            tags: Vec::new(),
            pipeline_tag: Some("text-to-image".to_string()),
            card_data: json!({ "base_model": base_model }),
            likes: 0,
            downloads: 0,
            last_modified: None,
            trending_score: None,
        };
        assert!(repo_compatibility(&profile, &fine_tune("black-forest-labs/FLUX.2-klein-4B")).0);
        assert!(!repo_compatibility(&profile, &fine_tune("Tongyi-MAI/Z-Image")).0);
    }

    #[test]
    fn role_search_filters_listings_by_marker() {
        let z_image = hf_bundle_profile("z-image-turbo").unwrap();
        assert!(role_search_matches(&z_image, "diffusion_model", "vendor/Z-Image-Turbo-GGUF", &[]));
        assert!(!role_search_matches(&z_image, "diffusion_model", "vendor/FLUX.2-klein-4B-GGUF", &[]));
        assert!(role_search_matches(&z_image, "text_encoder", "unsloth/Qwen3-4B-GGUF", &[]));
        assert!(!role_search_matches(&z_image, "text_encoder", "unsloth/Qwen3-8B-GGUF", &[]));
        assert!(role_search_matches(&z_image, "vae", "vendor/flux-vae-collection", &[]));
        let qwen_edit = hf_bundle_profile("qwen-image-edit-2511").unwrap();
        assert!(role_search_matches(
            &qwen_edit,
            "vision_encoder",
            "vendor/some-projector",
            &["mmproj".to_string()]
        ));
    }

    #[test]
    fn qwen_edit_recipe_requires_a_vision_encoder() {
        let profile = hf_bundle_profile("qwen-image-edit-2511").unwrap();
        assert!(profile.required_roles.contains(&"vision_encoder"));
        assert!(!hf_bundle_profile("z-image").unwrap().required_roles.contains(&"vision_encoder"));
    }
}
