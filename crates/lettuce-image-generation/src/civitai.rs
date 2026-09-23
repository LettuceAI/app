//! CivitAI LoRA browsing: the search and model requests, what the app shows
//! of a response (supported base models only, images and models hidden while
//! Pure mode is on) and the checks on a LoRA download.

use std::path::{Component, Path};

use serde::Deserialize;

use crate::sd_runtime::lora_library::{lora_architecture_supported, normalize_lora_architecture};

pub const CIVITAI_API_ENDPOINT: &str = "https://civitai.com";
pub const CIVITAI_PAGE_FETCH_LIMIT: u32 = 100;
pub const CIVITAI_MAX_PAGE_FETCHES: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CivitaiImage {
    pub url: String,
    pub nsfw_level: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CivitaiLoraSummary {
    pub id: u64,
    pub name: String,
    pub nsfw: bool,
    pub nsfw_level: u32,
    pub creator_username: Option<String>,
    pub download_count: u64,
    pub thumbs_up_count: u64,
    pub preview_image: Option<CivitaiImage>,
    pub base_models: Vec<String>,
    pub latest_version_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CivitaiFile {
    pub id: u64,
    pub name: String,
    pub size_kb: f64,
    pub primary: bool,
    pub format: Option<String>,
    pub fp: Option<String>,
    pub sha256: Option<String>,
    pub download_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CivitaiVersion {
    pub id: u64,
    pub name: String,
    pub base_model: Option<String>,
    pub architecture: Option<String>,
    pub published_at: Option<String>,
    pub trained_words: Vec<String>,
    pub images: Vec<CivitaiImage>,
    pub files: Vec<CivitaiFile>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CivitaiModelDetail {
    pub id: u64,
    pub name: String,
    pub description: Option<String>,
    pub nsfw: bool,
    pub nsfw_level: u32,
    pub creator_username: Option<String>,
    pub download_count: u64,
    pub thumbs_up_count: u64,
    pub tags: Vec<String>,
    pub versions: Vec<CivitaiVersion>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiModelsResponse {
    #[serde(default)]
    items: Vec<ApiModel>,
    #[serde(default)]
    metadata: ApiMetadata,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiMetadata {
    #[serde(default)]
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
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

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiStats {
    #[serde(default)]
    download_count: u64,
    #[serde(default)]
    thumbs_up_count: u64,
}

#[derive(Default, Deserialize)]
struct ApiCreator {
    #[serde(default)]
    username: Option<String>,
}

#[derive(Deserialize)]
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

#[derive(Deserialize)]
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

#[derive(Deserialize)]
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

#[derive(Default, Deserialize)]
struct ApiFileMetadata {
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    fp: Option<String>,
}

#[derive(Default, Deserialize)]
struct ApiHashes {
    #[serde(default, rename = "SHA256")]
    sha256: Option<String>,
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

fn supported_base_model(value: &str) -> bool {
    normalize_lora_architecture(value)
        .is_some_and(|architecture| lora_architecture_supported(&architecture))
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
                base_models.push(base_model.to_owned());
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

/// A LoRA search, normalized like the old browser did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CivitaiSearch {
    pub query: Option<String>,
    pub sort: Option<String>,
    pub period: Option<String>,
    pub base_models: Vec<String>,
    pub cursor: Option<String>,
    pub limit: Option<u8>,
}

impl CivitaiSearch {
    /// How many summaries a page gathers before it stops fetching.
    #[must_use]
    pub fn target(&self) -> usize {
        usize::from(self.limit.unwrap_or(30).clamp(1, 100))
    }

    #[must_use]
    pub fn first_cursor(&self) -> Option<String> {
        self.cursor
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    }

    /// The `/api/v1/models` query for one fetch.
    #[must_use]
    pub fn query_parameters(
        &self,
        pure_active: bool,
        cursor: Option<&str>,
    ) -> Vec<(String, String)> {
        let sort = match self.sort.as_deref() {
            Some("Most Downloaded") => "Most Downloaded",
            Some("Newest") => "Newest",
            _ => "Highest Rated",
        };
        let mut query = vec![
            ("types".to_owned(), "LORA".to_owned()),
            ("sort".to_owned(), sort.to_owned()),
            ("limit".to_owned(), CIVITAI_PAGE_FETCH_LIMIT.to_string()),
            (
                "nsfw".to_owned(),
                if pure_active { "false" } else { "true" }.to_owned(),
            ),
        ];
        if let Some(period) = self
            .period
            .as_deref()
            .map(str::trim)
            .filter(|value| matches!(*value, "AllTime" | "Year" | "Month" | "Week" | "Day"))
        {
            query.push(("period".to_owned(), period.to_owned()));
        }
        if let Some(text) = self
            .query
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            query.push(("query".to_owned(), text.to_owned()));
        }
        for base_model in self
            .base_models
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            query.push(("baseModels".to_owned(), base_model.to_owned()));
        }
        if let Some(cursor) = cursor {
            query.push(("cursor".to_owned(), cursor.to_owned()));
        }
        query
    }
}

/// One fetched page: the LoRAs worth showing and the next cursor.
pub fn parse_search_page(
    body: &[u8],
    pure_active: bool,
) -> Result<(Vec<CivitaiLoraSummary>, Option<String>), String> {
    let payload: ApiModelsResponse = serde_json::from_slice(body)
        .map_err(|error| format!("Could not read the CivitAI response: {error}"))?;
    Ok((
        payload
            .items
            .into_iter()
            .filter(|model| model.model_type.eq_ignore_ascii_case("LORA"))
            .filter(|model| !pure_active || !model.nsfw)
            .filter(|model| {
                model.model_versions.iter().any(|version| {
                    version
                        .base_model
                        .as_deref()
                        .is_some_and(supported_base_model)
                })
            })
            .map(|model| summarize(model, pure_active))
            .collect(),
        payload.metadata.next_cursor,
    ))
}

/// A model's versions that target a supported base model.
pub fn parse_model_detail(body: &[u8], pure_active: bool) -> Result<CivitaiModelDetail, String> {
    let model: ApiModel = serde_json::from_slice(body)
        .map_err(|error| format!("Could not read the CivitAI response: {error}"))?;
    if pure_active && model.nsfw {
        return Err("This CivitAI model is not available while Pure mode is on.".to_owned());
    }
    let versions = model
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
                .and_then(normalize_lora_architecture),
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
        .collect::<Vec<_>>();
    if versions.is_empty() {
        return Err(
            "This LoRA targets a base model that local image generation does not support."
                .to_owned(),
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

/// The text for an unsuccessful CivitAI response.
#[must_use]
pub fn civitai_status_error(status: u16, status_text: &str, token_saved: bool) -> String {
    match status {
        429 => "CivitAI is rate limiting requests. Wait a moment and try again.".to_owned(),
        401 if token_saved => "The saved CivitAI token is invalid or expired.".to_owned(),
        401 => "CivitAI requires an API token for this request.".to_owned(),
        _ => format!("CivitAI request failed with status {status_text}."),
    }
}

/// A LoRA file to download from CivitAI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CivitaiLoraDownload {
    pub model_name: String,
    pub version_id: u64,
    pub file_name: String,
    pub sha256: Option<String>,
    pub download_url: Option<String>,
    pub trained_words: Vec<String>,
    pub base_model: Option<String>,
}

impl CivitaiLoraDownload {
    /// The library file name and the https CivitAI URL to fetch it from.
    pub fn target(&self) -> Result<(String, String), String> {
        let filename = self.file_name.trim().to_owned();
        let relative = Path::new(&filename);
        let single = relative.components().count() == 1
            && relative
                .components()
                .all(|component| matches!(component, Component::Normal(_)));
        if filename.is_empty() || relative.is_absolute() || !single {
            return Err("The selected LoRA file has an unsafe name.".to_owned());
        }
        if !filename.to_ascii_lowercase().ends_with(".safetensors") {
            return Err("Only safetensors LoRA files can be downloaded.".to_owned());
        }
        let url = match self.download_url.as_deref().map(str::trim) {
            Some(url) if !url.is_empty() => {
                if !is_civitai_https_url(url) {
                    return Err("The CivitAI download link is invalid.".to_owned());
                }
                url.to_owned()
            }
            _ => format!(
                "https://civitai.com/api/download/models/{}",
                self.version_id
            ),
        };
        Ok((filename, url))
    }

    #[must_use]
    pub fn normalized_sha256(&self) -> Option<String> {
        self.sha256
            .as_deref()
            .map(str::trim)
            .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .map(str::to_lowercase)
    }
}

/// An https URL on civitai.com or one of its subdomains.
#[must_use]
pub fn is_civitai_https_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.contains('@') {
        return false;
    }
    let host = authority
        .rsplit_once(':')
        .map_or(authority, |(host, port)| {
            if port.bytes().all(|byte| byte.is_ascii_digit()) {
                host
            } else {
                authority
            }
        })
        .to_ascii_lowercase();
    host == "civitai.com" || host.ends_with(".civitai.com")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn searches_keep_supported_loras_and_hide_nsfw_in_pure_mode() {
        let body = br#"{"items": [
            {"id": 1, "name": "A", "type": "LORA", "nsfw": false,
             "modelVersions": [{"id": 11, "baseModel": "ZImageTurbo", "images": [{"url": "x", "nsfwLevel": 4}, {"url": "y", "nsfwLevel": 1}]},
                               {"id": 12, "baseModel": "Illustrious"}]},
            {"id": 2, "name": "B", "type": "LORA", "nsfw": true, "modelVersions": [{"id": 21, "baseModel": "ZImageTurbo"}]},
            {"id": 3, "name": "C", "type": "Checkpoint", "modelVersions": [{"id": 31, "baseModel": "ZImageTurbo"}]}
        ], "metadata": {"nextCursor": "n"}}"#;
        let search = CivitaiSearch {
            period: Some("Week".to_owned()),
            base_models: vec![" Flux.1 D ".to_owned(), " ".to_owned()],
            ..CivitaiSearch::default()
        };
        assert_eq!(
            search.query_parameters(true, Some("c")),
            [
                ("types", "LORA"),
                ("sort", "Highest Rated"),
                ("limit", "100"),
                ("nsfw", "false"),
                ("period", "Week"),
                ("baseModels", "Flux.1 D"),
                ("cursor", "c"),
            ]
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
        );
        let (pure, next) = parse_search_page(body, true).expect("page");
        assert_eq!(next.as_deref(), Some("n"));
        let supported =
            |models: &[CivitaiLoraSummary]| models.iter().map(|model| model.id).collect::<Vec<_>>();
        assert_eq!(
            pure[0]
                .preview_image
                .as_ref()
                .map(|image| image.url.as_str()),
            Some("y")
        );
        let (open, _) = parse_search_page(body, false).expect("page");
        assert_eq!(
            open[0]
                .preview_image
                .as_ref()
                .map(|image| image.url.as_str()),
            Some("x")
        );
        assert!(supported(&open).contains(&2) || !supported(&pure).contains(&2));
        assert!(!supported(&pure).contains(&2));
        assert!(!supported(&open).contains(&3));
    }

    #[test]
    fn downloads_stay_on_civitai_and_in_the_library() {
        let download = |file_name: &str, url: Option<&str>| CivitaiLoraDownload {
            file_name: file_name.to_owned(),
            download_url: url.map(str::to_owned),
            version_id: 7,
            ..CivitaiLoraDownload::default()
        };
        assert_eq!(
            download(" style.safetensors ", None).target(),
            Ok((
                "style.safetensors".to_owned(),
                "https://civitai.com/api/download/models/7".to_owned()
            ))
        );
        assert_eq!(
            download("../x.safetensors", None).target(),
            Err("The selected LoRA file has an unsafe name.".to_owned())
        );
        assert_eq!(
            download("x.ckpt", None).target(),
            Err("Only safetensors LoRA files can be downloaded.".to_owned())
        );
        for url in [
            "http://civitai.com/x",
            "https://evil.com/civitai.com",
            "https://civitai.com.evil.com/x",
            "https://user@civitai.com/x",
        ] {
            assert_eq!(
                download("x.safetensors", Some(url)).target(),
                Err("The CivitAI download link is invalid.".to_owned()),
                "{url}"
            );
        }
        assert!(
            download("x.safetensors", Some("https://cdn.civitai.com:443/f"))
                .target()
                .is_ok()
        );
        assert_eq!(
            civitai_status_error(401, "401 Unauthorized", false),
            "CivitAI requires an API token for this request."
        );
    }
}
