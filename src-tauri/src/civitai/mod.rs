use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Component, Path};
use tauri::AppHandle;

const API_BASE: &str = "https://civitai.com/api/v1";
const TOKEN_META_KEY: &str = "civitai_access_token";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CivitaiAuthStatus {
    saved: bool,
    valid: bool,
    error_kind: Option<&'static str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CivitaiImage {
    url: String,
    nsfw_level: u32,
    width: u32,
    height: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CivitaiLoraSummary {
    id: u64,
    name: String,
    nsfw: bool,
    nsfw_level: u32,
    creator_username: Option<String>,
    download_count: u64,
    thumbs_up_count: u64,
    preview_image: Option<CivitaiImage>,
    base_models: Vec<String>,
    latest_version_id: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CivitaiSearchPage {
    items: Vec<CivitaiLoraSummary>,
    next_cursor: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CivitaiFile {
    id: u64,
    name: String,
    size_kb: f64,
    primary: bool,
    format: Option<String>,
    fp: Option<String>,
    sha256: Option<String>,
    download_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CivitaiVersion {
    id: u64,
    name: String,
    base_model: Option<String>,
    architecture: Option<String>,
    published_at: Option<String>,
    trained_words: Vec<String>,
    images: Vec<CivitaiImage>,
    files: Vec<CivitaiFile>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CivitaiModelDetail {
    id: u64,
    name: String,
    description: Option<String>,
    nsfw: bool,
    nsfw_level: u32,
    creator_username: Option<String>,
    download_count: u64,
    thumbs_up_count: u64,
    tags: Vec<String>,
    versions: Vec<CivitaiVersion>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiModelsResponse {
    #[serde(default)]
    items: Vec<ApiModel>,
    #[serde(default)]
    metadata: ApiMetadata,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiMetadata {
    #[serde(default)]
    next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiModel {
    id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default, rename = "type")]
    model_type: String,
    #[serde(default)]
    nsfw: bool,
    #[serde(default)]
    nsfw_level: u32,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    stats: ApiStats,
    #[serde(default)]
    creator: ApiCreator,
    #[serde(default)]
    model_versions: Vec<ApiVersion>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiStats {
    #[serde(default)]
    download_count: u64,
    #[serde(default)]
    thumbs_up_count: u64,
}

#[derive(Debug, Default, Deserialize)]
struct ApiCreator {
    #[serde(default)]
    username: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiVersion {
    id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    base_model: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    trained_words: Vec<String>,
    #[serde(default)]
    images: Vec<ApiImage>,
    #[serde(default)]
    files: Vec<ApiFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiImage {
    #[serde(default)]
    url: String,
    #[serde(default)]
    nsfw_level: u32,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiFile {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    name: String,
    #[serde(default, rename = "sizeKB")]
    size_kb: f64,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    metadata: ApiFileMetadata,
    #[serde(default)]
    hashes: ApiHashes,
    #[serde(default)]
    download_url: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ApiFileMetadata {
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    fp: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ApiHashes {
    #[serde(default, rename = "SHA256")]
    sha256: Option<String>,
}

pub(crate) fn civitai_token(app: &AppHandle) -> Option<String> {
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

fn civitai_client(app: &AppHandle) -> Result<reqwest::Client, String> {
    let mut headers = HeaderMap::new();
    if let Some(token) = civitai_token(app) {
        let value = HeaderValue::from_str(&format!("Bearer {}", token.trim()))
            .map_err(|_| "The saved CivitAI token is malformed.".to_string())?;
        headers.insert(AUTHORIZATION, value);
    }
    reqwest::Client::builder()
        .user_agent("LettuceAI/1.0")
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|error| format!("Failed to build CivitAI client: {error}"))
}

fn read_pure_mode_level(app: &AppHandle) -> String {
    if let Ok(Some(raw)) = crate::storage_manager::internal_read_settings(app) {
        if let Ok(json) = serde_json::from_str::<Value>(&raw) {
            return crate::content_filter::level_from_app_state(json.get("appState"))
                .as_str()
                .to_string();
        }
    }
    "standard".to_string()
}

fn status_error(app: &AppHandle, status: reqwest::StatusCode) -> String {
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return "CivitAI is rate limiting requests. Wait a moment and try again.".to_string();
    }
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return if civitai_token(app).is_some() {
            "The saved CivitAI token is invalid or expired.".to_string()
        } else {
            "CivitAI requires an API token for this request.".to_string()
        };
    }
    format!("CivitAI request failed with status {status}.")
}

fn image_allowed(image: &ApiImage, pure_active: bool) -> bool {
    !image.url.is_empty() && (!pure_active || image.nsfw_level <= 1)
}

fn to_image(image: &ApiImage) -> CivitaiImage {
    CivitaiImage {
        url: image.url.clone(),
        nsfw_level: image.nsfw_level,
        width: image.width,
        height: image.height,
    }
}

fn model_allowed(model: &ApiModel, pure_active: bool) -> bool {
    !pure_active || !model.nsfw
}

fn supported_base_model(value: &str) -> bool {
    crate::image_generator::sdcpp::normalize_lora_architecture(value).is_some()
}

fn model_supported(model: &ApiModel) -> bool {
    model.model_versions.iter().any(|version| {
        version
            .base_model
            .as_deref()
            .is_some_and(supported_base_model)
    })
}

fn summarize(model: ApiModel, pure_active: bool) -> CivitaiLoraSummary {
    let mut base_models: Vec<String> = Vec::new();
    let mut latest_version_id = None;
    let mut preview_image = None;
    for version in &model.model_versions {
        if latest_version_id.is_none() {
            latest_version_id = Some(version.id);
        }
        if let Some(base_model) = version.base_model.as_deref() {
            let base_model = base_model.trim();
            if !base_model.is_empty()
                && supported_base_model(base_model)
                && !base_models.iter().any(|value| value == base_model)
            {
                base_models.push(base_model.to_string());
            }
        }
        if preview_image.is_none() {
            preview_image = version
                .images
                .iter()
                .find(|image| image_allowed(image, pure_active))
                .map(to_image);
        }
    }
    CivitaiLoraSummary {
        id: model.id,
        name: model.name,
        nsfw: model.nsfw,
        nsfw_level: model.nsfw_level,
        creator_username: model.creator.username,
        download_count: model.stats.download_count,
        thumbs_up_count: model.stats.thumbs_up_count,
        preview_image,
        base_models,
        latest_version_id,
    }
}

#[tauri::command]
pub async fn civitai_search_loras(
    app: AppHandle,
    query: Option<String>,
    sort: Option<String>,
    period: Option<String>,
    base_models: Option<Vec<String>>,
    cursor: Option<String>,
    limit: Option<u8>,
) -> Result<CivitaiSearchPage, String> {
    let pure_active = read_pure_mode_level(&app) != "off";
    let client = civitai_client(&app)?;
    let limit = limit.unwrap_or(30).clamp(1, 100);
    let sort = match sort.as_deref() {
        Some("Most Downloaded") => "Most Downloaded",
        Some("Newest") => "Newest",
        _ => "Highest Rated",
    };
    let mut request = client
        .get(format!("{API_BASE}/models"))
        .query(&[("types", "LORA"), ("sort", sort)])
        .query(&[("limit", limit.to_string())])
        .query(&[("nsfw", if pure_active { "false" } else { "true" })]);
    if let Some(period) = period.as_deref().map(str::trim) {
        if matches!(period, "AllTime" | "Year" | "Month" | "Week" | "Day") {
            request = request.query(&[("period", period)]);
        }
    }
    if let Some(query) = query.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        request = request.query(&[("query", query)]);
    }
    for base_model in base_models.unwrap_or_default() {
        let base_model = base_model.trim();
        if !base_model.is_empty() {
            request = request.query(&[("baseModels", base_model)]);
        }
    }
    if let Some(cursor) = cursor.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        request = request.query(&[("cursor", cursor)]);
    }

    let response = request
        .send()
        .await
        .map_err(|error| format!("Could not reach CivitAI: {error}"))?;
    if !response.status().is_success() {
        return Err(status_error(&app, response.status()));
    }
    let payload: ApiModelsResponse = response
        .json()
        .await
        .map_err(|error| format!("Could not read the CivitAI response: {error}"))?;
    let items = payload
        .items
        .into_iter()
        .filter(|model| model.model_type.eq_ignore_ascii_case("LORA"))
        .filter(|model| model_allowed(model, pure_active))
        .filter(model_supported)
        .map(|model| summarize(model, pure_active))
        .collect();
    Ok(CivitaiSearchPage {
        items,
        next_cursor: payload.metadata.next_cursor,
    })
}

#[tauri::command]
pub async fn civitai_get_model(app: AppHandle, model_id: u64) -> Result<CivitaiModelDetail, String> {
    let pure_active = read_pure_mode_level(&app) != "off";
    let client = civitai_client(&app)?;
    let response = client
        .get(format!("{API_BASE}/models/{model_id}"))
        .send()
        .await
        .map_err(|error| format!("Could not reach CivitAI: {error}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err("This CivitAI model no longer exists.".to_string());
    }
    if !response.status().is_success() {
        return Err(status_error(&app, response.status()));
    }
    let model: ApiModel = response
        .json()
        .await
        .map_err(|error| format!("Could not read the CivitAI response: {error}"))?;
    if !model_allowed(&model, pure_active) {
        return Err("This CivitAI model is not available while Pure mode is on.".to_string());
    }
    let versions: Vec<CivitaiVersion> = model
        .model_versions
        .into_iter()
        .filter(|version| {
            version
                .base_model
                .as_deref()
                .is_some_and(supported_base_model)
        })
        .map(|version| CivitaiVersion {
            id: version.id,
            architecture: version
                .base_model
                .as_deref()
                .and_then(crate::image_generator::sdcpp::normalize_lora_architecture),
            name: version.name,
            base_model: version.base_model,
            published_at: version.published_at,
            trained_words: version.trained_words,
            images: version
                .images
                .iter()
                .filter(|image| image_allowed(image, pure_active))
                .map(to_image)
                .collect(),
            files: version
                .files
                .into_iter()
                .map(|file| CivitaiFile {
                    id: file.id,
                    name: file.name,
                    size_kb: file.size_kb,
                    primary: file.primary,
                    format: file.metadata.format,
                    fp: file.metadata.fp,
                    sha256: file.hashes.sha256,
                    download_url: file.download_url,
                })
                .collect(),
        })
        .collect();
    if versions.is_empty() {
        return Err(
            "This LoRA targets a base model that local image generation does not support."
                .to_string(),
        );
    }
    Ok(CivitaiModelDetail {
        id: model.id,
        name: model.name,
        description: model.description,
        nsfw: model.nsfw,
        nsfw_level: model.nsfw_level,
        creator_username: model.creator.username,
        download_count: model.stats.download_count,
        thumbs_up_count: model.stats.thumbs_up_count,
        tags: model.tags,
        versions,
    })
}

enum TokenCheck {
    Valid,
    Unverified,
}

async fn validate_token(token: &str) -> Result<TokenCheck, String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("Enter a CivitAI API token.".to_string());
    }
    let response = reqwest::Client::builder()
        .user_agent("LettuceAI/1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|error| format!("Failed to create the token validation client: {error}"))?
        .get(format!("{API_BASE}/models"))
        .query(&[("limit", "1"), ("hidden", "true")])
        .bearer_auth(token)
        .send()
        .await
        .map_err(|error| format!("Could not validate the CivitAI token: {error}"))?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("The CivitAI token is invalid or expired.".to_string());
    }
    if response.status().is_success() {
        Ok(TokenCheck::Valid)
    } else {
        Ok(TokenCheck::Unverified)
    }
}

#[tauri::command]
pub async fn civitai_auth_status(app: AppHandle) -> Result<CivitaiAuthStatus, String> {
    let Some(token) = civitai_token(&app) else {
        return Ok(CivitaiAuthStatus {
            saved: false,
            valid: false,
            error_kind: Some("missingToken"),
        });
    };
    match validate_token(&token).await {
        Ok(TokenCheck::Valid) => Ok(CivitaiAuthStatus {
            saved: true,
            valid: true,
            error_kind: None,
        }),
        Ok(TokenCheck::Unverified) => Ok(CivitaiAuthStatus {
            saved: true,
            valid: false,
            error_kind: Some("unverified"),
        }),
        Err(_) => Ok(CivitaiAuthStatus {
            saved: true,
            valid: false,
            error_kind: Some("invalidOrExpired"),
        }),
    }
}

#[tauri::command]
pub async fn civitai_auth_save(app: AppHandle, token: String) -> Result<CivitaiAuthStatus, String> {
    let check = validate_token(&token).await?;
    let conn = crate::storage_manager::db::open_db(&app)?;
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![TOKEN_META_KEY, token.trim()],
    )
    .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?;
    Ok(match check {
        TokenCheck::Valid => CivitaiAuthStatus {
            saved: true,
            valid: true,
            error_kind: None,
        },
        TokenCheck::Unverified => CivitaiAuthStatus {
            saved: true,
            valid: false,
            error_kind: Some("unverified"),
        },
    })
}

#[tauri::command]
pub fn civitai_auth_clear(app: AppHandle) -> Result<(), String> {
    let conn = crate::storage_manager::db::open_db(&app)?;
    conn.execute("DELETE FROM meta WHERE key = ?1", params![TOKEN_META_KEY])
        .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?;
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CivitaiLoraDownloadRequest {
    model_name: String,
    version_id: u64,
    file_name: String,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default)]
    trained_words: Vec<String>,
    #[serde(default)]
    base_model: Option<String>,
}

#[tauri::command]
pub async fn civitai_queue_lora_download(
    app: AppHandle,
    request: CivitaiLoraDownloadRequest,
) -> Result<String, String> {
    if cfg!(mobile) {
        return Err("Local stable-diffusion.cpp image generation is desktop-only.".to_string());
    }
    let filename = request.file_name.trim().to_string();
    let relative = Path::new(&filename);
    let is_single_normal_component = relative.components().count() == 1
        && relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if filename.is_empty() || relative.is_absolute() || !is_single_normal_component {
        return Err("The selected LoRA file has an unsafe name.".to_string());
    }
    if !filename.to_ascii_lowercase().ends_with(".safetensors") {
        return Err("Only safetensors LoRA files can be downloaded.".to_string());
    }

    let download_url = match request.download_url.as_deref().map(str::trim) {
        Some(url) if !url.is_empty() => {
            let parsed = reqwest::Url::parse(url)
                .map_err(|_| "The CivitAI download link is invalid.".to_string())?;
            let civitai_host = parsed
                .host_str()
                .is_some_and(|host| host == "civitai.com" || host.ends_with(".civitai.com"));
            if parsed.scheme() != "https" || !civitai_host {
                return Err("The CivitAI download link is invalid.".to_string());
            }
            url.to_string()
        }
        _ => format!("https://civitai.com/api/download/models/{}", request.version_id),
    };

    let root = crate::image_generator::sdcpp::lora_root(&app)?;
    std::fs::create_dir_all(&root)
        .map_err(|error| format!("Failed to create the local LoRA library: {error}"))?;
    let destination = root.join(&filename);

    let metadata = crate::hf_browser::QueueDownloadMetadata {
        display_name: Some(request.model_name.clone()),
        download_role: Some("lora".to_string()),
        queue_kind: Some("civitai_lora".to_string()),
        download_url: Some(download_url),
        destination_path: Some(destination.to_string_lossy().to_string()),
        sha256: request
            .sha256
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_lowercase),
        lora_keywords: Some(request.trained_words),
        lora_base_model: request.base_model,
        ..Default::default()
    };
    crate::hf_browser::hf_queue_download(
        app,
        format!("civitai/{}", request.model_name),
        filename,
        Some(metadata),
    )
    .await
}
