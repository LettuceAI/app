//! The local LoRA library: files below the LoRA folder with their trigger
//! keywords and base architecture, found in safetensors metadata or on
//! CivitAI, or set by the user (legacy `image_loras` and `sdcpp_*lora*`).

use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};

use lettuce_models::StableDiffusionLora;
use lettuce_network::{BulkHttpClient, JsonAuth, JsonStaticHeader};
use lettuce_types::TimestampMillis;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::loras::{LORA_COMPAT_CACHE_DIR, lora_file_fingerprint};
use crate::diffusion_catalog;

const MAX_KEYWORD_BYTES: usize = 160;
const MAX_KEYWORDS: usize = 32;
const MAX_SAFETENSORS_HEADER_BYTES: u64 = 64 * 1024 * 1024;
const LORA_EXTENSIONS: [&str; 3] = ["safetensors", "ckpt", "pt"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoraKeywordSource {
    None,
    Metadata,
    Civitai,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoraArchitectureSource {
    None,
    Metadata,
    Civitai,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoraCompatibility {
    Compatible,
    Incompatible,
    Unknown,
}

/// One library file's stored discovery, keyed by its library-relative path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoraRecord {
    pub path: String,
    pub filename: String,
    pub bytes_on_disk: u64,
    pub modified_at: u64,
    pub sha256: Option<String>,
    pub keywords: Vec<String>,
    pub keyword_source: LoraKeywordSource,
    pub architecture: Option<String>,
    pub architecture_source: LoraArchitectureSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LoraLibraryRepositoryError {
    #[error("stored LoRA library data are invalid")]
    InvalidData,
    #[error("LoRA library storage failed")]
    Storage,
}

pub trait LoraLibraryRepository: Send + Sync {
    fn lora(&self, path: &str) -> Result<Option<LoraRecord>, LoraLibraryRepositoryError>;
    /// The newest other record of the same file content that found keywords
    /// or an architecture.
    fn lora_by_hash(&self, sha256: &str) -> Result<Option<LoraRecord>, LoraLibraryRepositoryError>;
    /// Records a file; a changed size or modification time forgets its hash,
    /// keywords and architecture.
    fn record_lora_file(
        &self,
        path: &str,
        filename: &str,
        bytes_on_disk: u64,
        modified_at: u64,
        now: TimestampMillis,
    ) -> Result<LoraRecord, LoraLibraryRepositoryError>;
    fn save_lora(
        &self,
        record: &LoraRecord,
        now: TimestampMillis,
    ) -> Result<(), LoraLibraryRepositoryError>;
    fn delete_lora(&self, path: &str) -> Result<(), LoraLibraryRepositoryError>;
    /// How many local image models list the LoRA in their settings.
    fn lora_model_references(&self, path: &str) -> Result<u64, LoraLibraryRepositoryError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledLora {
    pub filename: String,
    pub path: String,
    pub bytes_on_disk: u64,
    pub keywords: Vec<String>,
    pub keyword_source: LoraKeywordSource,
    pub architecture: Option<String>,
    pub architecture_source: LoraArchitectureSource,
    pub compatibility: LoraCompatibility,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoraKeywordDiscovery {
    pub keywords: Vec<String>,
    pub source: LoraKeywordSource,
    pub sha256: Option<String>,
    pub architecture: Option<String>,
    pub architecture_source: LoraArchitectureSource,
    pub compatibility: LoraCompatibility,
}

#[derive(Debug, Default)]
struct LocalLoraMetadata {
    keywords: Vec<String>,
    architecture: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CivitaiModelVersion {
    #[serde(default, rename = "trainedWords")]
    trained_words: Vec<String>,
    #[serde(default, rename = "baseModel")]
    base_model: Option<String>,
}

/// Trimmed keywords of at most 160 bytes, deduplicated ignoring case, at
/// most 32.
#[must_use]
pub fn normalize_lora_keywords(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut keywords = Vec::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() || value.len() > MAX_KEYWORD_BYTES {
            continue;
        }
        if seen.insert(value.to_lowercase()) {
            keywords.push(value.to_owned());
        }
        if keywords.len() == MAX_KEYWORDS {
            break;
        }
    }
    keywords
}

fn parse_keyword_metadata_value(value: &str) -> Vec<String> {
    if let Ok(values) = serde_json::from_str::<Vec<String>>(value) {
        return normalize_lora_keywords(values);
    }
    normalize_lora_keywords(
        value
            .split([',', '\n', ';'])
            .map(str::trim)
            .map(str::to_owned),
    )
}

/// Explicit trigger keys first; a training-tag frequency table counts only
/// when it holds exactly one tag.
#[must_use]
pub fn keywords_from_safetensors_metadata(metadata: &Map<String, Value>) -> Vec<String> {
    const EXPLICIT_KEYS: [&str; 7] = [
        "ss_activation_tags",
        "modelspec.trigger_phrase",
        "modelspec.trigger_phrases",
        "trigger_words",
        "trained_words",
        "activation_tags",
        "civitai.trainedWords",
    ];
    for key in EXPLICIT_KEYS {
        if let Some(value) = metadata.get(key).and_then(Value::as_str) {
            let keywords = parse_keyword_metadata_value(value);
            if !keywords.is_empty() {
                return keywords;
            }
        }
    }
    let Some(tag_frequency) = metadata
        .get("ss_tag_frequency")
        .and_then(Value::as_str)
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .and_then(|value| value.as_object().cloned())
    else {
        return Vec::new();
    };
    let mut tags = HashSet::new();
    for dataset in tag_frequency.values().filter_map(Value::as_object) {
        for tag in dataset.keys() {
            let tag = tag.trim();
            if !tag.is_empty() {
                tags.insert(tag.to_owned());
            }
        }
    }
    if tags.len() == 1 {
        normalize_lora_keywords(tags)
    } else {
        Vec::new()
    }
}

/// Legacy's base-model names: the first family whose marker the value
/// contains, with or without separators.
#[must_use]
pub fn normalize_lora_architecture(value: &str) -> Option<String> {
    let value = value
        .trim()
        .to_ascii_lowercase()
        .replace(['_', '-', '.', '/'], " ");
    let compact = value.split_whitespace().collect::<String>();
    let contains = |needle: &str| value.contains(needle) || compact.contains(needle);
    let architecture = if contains("flux2") {
        if contains("klein4b") || (contains("klein") && contains("4b")) {
            "flux2-klein-4b"
        } else if contains("klein9b") || (contains("klein") && contains("9b")) {
            "flux2-klein-9b"
        } else {
            "flux2"
        }
    } else if contains("zimage") {
        "z-image"
    } else if contains("krea2") {
        "krea-2"
    } else if contains("qwenimageedit2511") {
        "qwen-image-edit-2511"
    } else if contains("qwenimageedit") {
        "qwen-image-edit"
    } else if contains("qwenimage") || compact == "qwen" {
        "qwen-image"
    } else if contains("flux1") || contains("flux dev") || contains("flux schnell") {
        "flux1"
    } else if contains("lora flux") || contains("loraflux") {
        "flux"
    } else if contains("illustrious") {
        "illustrious"
    } else if contains("noobai") {
        "noobai"
    } else if contains("pony") {
        "pony"
    } else if contains("sdxl") || contains("stablediffusionxl") {
        "sdxl"
    } else if contains("sd35") || contains("sd3") || contains("stablediffusion3") {
        "sd3"
    } else if contains("sd21") || contains("sd2") || contains("stablediffusion2") {
        "sd2"
    } else if contains("sd15")
        || contains("sd14")
        || contains("sd1")
        || contains("stablediffusionv1")
    {
        "sd1"
    } else {
        return None;
    };
    Some(architecture.to_owned())
}

#[must_use]
pub fn architecture_from_safetensors_metadata(metadata: &Map<String, Value>) -> Option<String> {
    const ARCHITECTURE_KEYS: [&str; 8] = [
        "modelspec.architecture",
        "modelspec.base_model",
        "ss_base_model_version",
        "ss_network_module",
        "base_model",
        "baseModel",
        "architecture",
        "model_type",
    ];
    ARCHITECTURE_KEYS.iter().find_map(|key| {
        metadata
            .get(*key)
            .and_then(Value::as_str)
            .and_then(normalize_lora_architecture)
    })
}

fn read_local_lora_metadata(path: &Path) -> Result<LocalLoraMetadata, String> {
    if !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("safetensors"))
    {
        return Ok(LocalLoraMetadata::default());
    }
    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("Failed to read LoRA metadata: {error}"))?;
    let mut length_bytes = [0_u8; 8];
    file.read_exact(&mut length_bytes)
        .map_err(|error| format!("Failed to read the LoRA metadata header: {error}"))?;
    let header_length = u64::from_le_bytes(length_bytes);
    let file_length = file
        .metadata()
        .map_err(|error| format!("Failed to inspect the LoRA file: {error}"))?
        .len();
    if header_length == 0
        || header_length > MAX_SAFETENSORS_HEADER_BYTES
        || header_length + 8 > file_length
    {
        return Err("The LoRA has an invalid safetensors metadata header.".to_owned());
    }
    let mut header = vec![
        0_u8;
        usize::try_from(header_length).map_err(|_| {
            "The LoRA has an invalid safetensors metadata header.".to_owned()
        })?
    ];
    file.read_exact(&mut header)
        .map_err(|error| format!("Failed to read the LoRA metadata: {error}"))?;
    let header: Value = serde_json::from_slice(&header)
        .map_err(|error| format!("Failed to parse the LoRA metadata: {error}"))?;
    let metadata = header.get("__metadata__").and_then(Value::as_object);
    Ok(LocalLoraMetadata {
        keywords: metadata
            .map(keywords_from_safetensors_metadata)
            .unwrap_or_default(),
        architecture: metadata.and_then(architecture_from_safetensors_metadata),
    })
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("Failed to open the LoRA for hashing: {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("Failed to hash the LoRA: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// The LoRA architecture a catalog profile trains on.
#[must_use]
pub fn profile_lora_architecture(profile_id: Option<&str>) -> Option<&'static str> {
    match profile_id? {
        "z-image-turbo" | "z-image" => Some("z-image"),
        "flux-2-klein-4b" => Some("flux2-klein-4b"),
        "flux-2-klein-9b" | "flux-2-klein-base-9b" => Some("flux2-klein-9b"),
        "krea-2-turbo" | "krea-2-raw" => Some("krea-2"),
        "qwen-image-edit-2511" => Some("qwen-image-edit-2511"),
        _ => None,
    }
}

/// Whether any catalog model can load LoRAs of this architecture (CivitAI
/// search filtering).
#[must_use]
pub fn lora_architecture_supported(architecture: &str) -> bool {
    diffusion_catalog()
        .profiles
        .iter()
        .filter_map(|profile| profile_lora_architecture(Some(&profile.id)))
        .any(|target| target == architecture)
        || matches!(
            architecture,
            "flux" | "flux2" | "qwen-image" | "qwen-image-edit"
        )
}

#[must_use]
pub fn lora_compatibility(
    architecture: Option<&str>,
    profile_id: Option<&str>,
) -> LoraCompatibility {
    let (Some(target), Some(architecture)) = (profile_lora_architecture(profile_id), architecture)
    else {
        return LoraCompatibility::Unknown;
    };
    if architecture == target {
        return LoraCompatibility::Compatible;
    }
    if matches!(
        architecture,
        "flux" | "flux2" | "qwen-image" | "qwen-image-edit"
    ) && (target.starts_with("flux") || target.starts_with("qwen-image-edit"))
    {
        return LoraCompatibility::Unknown;
    }
    LoraCompatibility::Incompatible
}

fn discovery(record: &LoraRecord, profile_id: Option<&str>) -> LoraKeywordDiscovery {
    LoraKeywordDiscovery {
        keywords: record.keywords.clone(),
        source: record.keyword_source,
        sha256: record.sha256.clone(),
        architecture: record.architecture.clone(),
        architecture_source: record.architecture_source,
        compatibility: lora_compatibility(record.architecture.as_deref(), profile_id),
    }
}

/// Stored keywords replace a LoRA's own when the user set them, or when any
/// were found.
pub fn apply_stored_lora_keywords(lora: &mut StableDiffusionLora, record: &LoraRecord) {
    if record.keyword_source == LoraKeywordSource::Manual || !record.keywords.is_empty() {
        lora.keywords.clone_from(&record.keywords);
    }
}

/// Fills each LoRA's keywords from the library before the prompt is
/// composed (legacy `hydrate_lora_keywords`).
pub fn hydrate_lora_keywords<R: LoraLibraryRepository + ?Sized>(
    repository: &R,
    loras: &mut [StableDiffusionLora],
) -> Result<(), LoraLibraryRepositoryError> {
    for lora in loras {
        if let Some(record) = repository.lora(&lora.path)? {
            apply_stored_lora_keywords(lora, &record);
        }
    }
    Ok(())
}

/// The LoRA folder and the records kept about its files.
pub struct LoraLibrary<'a, R: ?Sized> {
    root: &'a Path,
    repository: &'a R,
}

impl<R: ?Sized> std::fmt::Debug for LoraLibrary<'_, R> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LoraLibrary")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

fn storage_error(error: LoraLibraryRepositoryError) -> String {
    error.to_string()
}

impl<'a, R: LoraLibraryRepository + ?Sized> LoraLibrary<'a, R> {
    #[must_use]
    pub const fn new(root: &'a Path, repository: &'a R) -> Self {
        Self { root, repository }
    }

    fn resolve(&self, requested: &str) -> Result<(PathBuf, PathBuf, String), String> {
        let root = self
            .root
            .canonicalize()
            .map_err(|error| format!("Failed to access the local LoRA library: {error}"))?;
        let requested_path = PathBuf::from(requested);
        let candidate = if requested_path.is_absolute() {
            requested_path
        } else {
            root.join(requested_path)
        };
        let candidate = candidate
            .canonicalize()
            .map_err(|error| format!("LoRA file does not exist: {error}"))?;
        if !candidate.starts_with(&root) || !candidate.is_file() {
            return Err("The selected LoRA is outside the local LoRA library.".to_owned());
        }
        let relative = candidate
            .strip_prefix(&root)
            .map_err(|_| "Failed to resolve the LoRA library path.".to_owned())?
            .to_string_lossy()
            .replace('\\', "/");
        Ok((root, candidate, relative))
    }

    fn record(
        &self,
        relative: &str,
        file_path: &Path,
        now: TimestampMillis,
    ) -> Result<LoraRecord, String> {
        let (bytes_on_disk, modified_at) = lora_file_fingerprint(file_path)?;
        let filename = file_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(relative)
            .to_owned();
        self.repository
            .record_lora_file(relative, &filename, bytes_on_disk, modified_at, now)
            .map_err(storage_error)
    }

    fn collect(&self, directory: &Path, files: &mut Vec<(String, String, PathBuf)>) {
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().and_then(|name| name.to_str()) == Some(LORA_COMPAT_CACHE_DIR) {
                    continue;
                }
                self.collect(&path, files);
                continue;
            }
            let supported = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    LORA_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
                });
            let Ok(relative) = path.strip_prefix(self.root) else {
                continue;
            };
            if !supported {
                continue;
            }
            files.push((
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_owned(),
                relative.to_string_lossy().replace('\\', "/"),
                path.clone(),
            ));
        }
    }

    /// Every library file with its keywords and compatibility with a
    /// profile; files not hashed yet get their safetensors metadata.
    pub fn list(
        &self,
        profile_id: Option<&str>,
        now: TimestampMillis,
    ) -> Result<Vec<InstalledLora>, String> {
        std::fs::create_dir_all(self.root)
            .map_err(|error| format!("Failed to create the local LoRA library: {error}"))?;
        let mut files = Vec::new();
        self.collect(self.root, &mut files);
        let mut installed = Vec::with_capacity(files.len());
        for (filename, relative, path) in files {
            let Ok((bytes_on_disk, modified_at)) = lora_file_fingerprint(&path) else {
                installed.push(InstalledLora {
                    filename,
                    path: relative,
                    bytes_on_disk: 0,
                    keywords: Vec::new(),
                    keyword_source: LoraKeywordSource::None,
                    architecture: None,
                    architecture_source: LoraArchitectureSource::None,
                    compatibility: LoraCompatibility::Unknown,
                });
                continue;
            };
            let mut record = self
                .repository
                .record_lora_file(&relative, &filename, bytes_on_disk, modified_at, now)
                .map_err(storage_error)?;
            let manual = record.keyword_source == LoraKeywordSource::Manual;
            if record.sha256.is_none()
                && ((!manual && record.keywords.is_empty()) || record.architecture.is_none())
                && let Ok(local) = read_local_lora_metadata(&path)
            {
                if !manual && record.keywords.is_empty() && !local.keywords.is_empty() {
                    record.keywords = local.keywords;
                    record.keyword_source = LoraKeywordSource::Metadata;
                }
                if record.architecture.is_none() && local.architecture.is_some() {
                    record.architecture = local.architecture;
                    record.architecture_source = LoraArchitectureSource::Metadata;
                }
                if !record.keywords.is_empty() || record.architecture.is_some() {
                    self.repository
                        .save_lora(&record, now)
                        .map_err(storage_error)?;
                }
            }
            installed.push(InstalledLora {
                filename,
                path: relative,
                bytes_on_disk,
                compatibility: lora_compatibility(record.architecture.as_deref(), profile_id),
                keywords: record.keywords,
                keyword_source: record.keyword_source,
                architecture: record.architecture,
                architecture_source: record.architecture_source,
            });
        }
        installed.sort_by_key(|lora| lora.filename.to_lowercase());
        Ok(installed)
    }

    /// Finds a LoRA's keywords and architecture: its metadata, another copy
    /// of the same file, then CivitAI by file hash. A user's keywords are
    /// never replaced.
    pub async fn discover(
        &self,
        path: &str,
        profile_id: Option<&str>,
        civitai: &BulkHttpClient,
        now: TimestampMillis,
    ) -> Result<LoraKeywordDiscovery, String> {
        let (_, file_path, relative) = self.resolve(path)?;
        let stored = self.record(&relative, &file_path, now)?;
        if stored.sha256.is_some() {
            return Ok(discovery(&stored, profile_id));
        }
        let local_path = file_path.clone();
        let local = tokio::task::spawn_blocking(move || read_local_lora_metadata(&local_path))
            .await
            .map_err(|error| format!("LoRA metadata task failed: {error}"))??;
        let hash_path = file_path.clone();
        let sha256 = tokio::task::spawn_blocking(move || sha256_file(&hash_path))
            .await
            .map_err(|error| format!("LoRA hashing task failed: {error}"))??;
        let manual = stored.keyword_source == LoraKeywordSource::Manual;
        let keep_keywords = manual || !stored.keywords.is_empty();
        let mut record = LoraRecord {
            sha256: Some(sha256.clone()),
            keyword_source: if keep_keywords {
                stored.keyword_source
            } else if !local.keywords.is_empty() {
                LoraKeywordSource::Metadata
            } else {
                LoraKeywordSource::None
            },
            architecture_source: if stored.architecture.is_some() {
                stored.architecture_source
            } else if local.architecture.is_some() {
                LoraArchitectureSource::Metadata
            } else {
                LoraArchitectureSource::None
            },
            keywords: if keep_keywords {
                stored.keywords.clone()
            } else {
                local.keywords
            },
            architecture: stored.architecture.clone().or(local.architecture),
            ..stored
        };
        if let Some(cached) = self
            .repository
            .lora_by_hash(&sha256)
            .map_err(storage_error)?
        {
            if !manual && record.keywords.is_empty() && !cached.keywords.is_empty() {
                record.keywords = cached.keywords;
                record.keyword_source = cached.keyword_source;
            }
            if record.architecture.is_none() && cached.architecture.is_some() {
                record.architecture = cached.architecture;
                record.architecture_source = cached.architecture_source;
            }
        }
        if (!manual && record.keywords.is_empty()) || record.architecture.is_none() {
            match civitai
                .get(
                    "https://civitai.com",
                    &format!("/api/v1/model-versions/by-hash/{sha256}"),
                    &[],
                    &[JsonStaticHeader {
                        name: "user-agent",
                        value: "LettuceAI LoRA metadata discovery",
                    }],
                    JsonAuth::None,
                    Vec::new(),
                    false,
                )
                .await
            {
                Ok(response) if (200..300).contains(&response.status) => {
                    if let Ok(version) =
                        serde_json::from_slice::<CivitaiModelVersion>(&response.body)
                    {
                        let remote_keywords = normalize_lora_keywords(version.trained_words);
                        if !manual && record.keywords.is_empty() && !remote_keywords.is_empty() {
                            record.keywords = remote_keywords;
                            record.keyword_source = LoraKeywordSource::Civitai;
                        }
                        if record.architecture.is_none() {
                            record.architecture = version
                                .base_model
                                .as_deref()
                                .and_then(normalize_lora_architecture);
                            if record.architecture.is_some() {
                                record.architecture_source = LoraArchitectureSource::Civitai;
                            }
                        }
                    }
                }
                Ok(response) if response.status == 404 => {}
                Ok(response) => tracing::warn!(
                    component = "sdcpp",
                    "LoRA metadata lookup failed with status {}",
                    lettuce_network::status_text(response.status)
                ),
                Err(error) => {
                    tracing::warn!(component = "sdcpp", "LoRA metadata lookup failed: {error}");
                }
            }
        }
        self.repository
            .save_lora(&record, now)
            .map_err(storage_error)?;
        Ok(discovery(&record, profile_id))
    }

    /// Sets the user's keywords for a LoRA.
    pub fn update_keywords(
        &self,
        path: &str,
        keywords: Vec<String>,
        profile_id: Option<&str>,
        now: TimestampMillis,
    ) -> Result<LoraKeywordDiscovery, String> {
        let keywords = normalize_lora_keywords(keywords);
        let (_, file_path, relative) = self.resolve(path)?;
        let mut record = self.record(&relative, &file_path, now)?;
        record.keywords = keywords;
        record.keyword_source = LoraKeywordSource::Manual;
        self.repository
            .save_lora(&record, now)
            .map_err(storage_error)?;
        Ok(discovery(&record, profile_id))
    }

    /// Copies a LoRA into the library root; a different file with the same
    /// name is refused.
    pub async fn import(&self, source_path: &str) -> Result<InstalledLora, String> {
        let source = PathBuf::from(source_path);
        if !source.is_file() {
            return Err(format!("LoRA file does not exist: {}", source.display()));
        }
        let extension = source
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !LORA_EXTENSIONS.contains(&extension.as_str()) {
            return Err("Choose a .safetensors, .ckpt, or .pt LoRA file.".to_owned());
        }
        let filename = source
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .ok_or_else(|| "The selected LoRA has an invalid filename.".to_owned())?
            .to_owned();
        std::fs::create_dir_all(self.root)
            .map_err(|error| format!("Failed to create the local LoRA library: {error}"))?;
        let destination = self.root.join(&filename);
        if destination.exists() {
            if source.canonicalize().ok() != destination.canonicalize().ok() {
                let source_size = std::fs::metadata(&source).map(|value| value.len()).ok();
                let destination_size = std::fs::metadata(&destination)
                    .map(|value| value.len())
                    .ok();
                let identical = if source_size.is_some() && source_size == destination_size {
                    let source_for_hash = source.clone();
                    let destination_for_hash = destination.clone();
                    tokio::task::spawn_blocking(move || {
                        Ok::<_, String>(
                            sha256_file(&source_for_hash)? == sha256_file(&destination_for_hash)?,
                        )
                    })
                    .await
                    .map_err(|error| format!("LoRA comparison task failed: {error}"))??
                } else {
                    false
                };
                if !identical {
                    return Err(format!(
                        "A different LoRA named {filename} is already in the library. Remove or rename it before importing this file."
                    ));
                }
            }
        } else {
            std::fs::copy(&source, &destination)
                .map_err(|error| format!("Failed to import {filename}: {error}"))?;
        }
        Ok(InstalledLora {
            bytes_on_disk: std::fs::metadata(&destination).map_or(0, |metadata| metadata.len()),
            path: filename.clone(),
            filename,
            keywords: Vec::new(),
            keyword_source: LoraKeywordSource::None,
            architecture: None,
            architecture_source: LoraArchitectureSource::None,
            compatibility: LoraCompatibility::Unknown,
        })
    }

    /// Deletes a LoRA no local image model uses, with its record and the
    /// compatibility cache.
    pub fn delete(&self, path: &str) -> Result<(), String> {
        let (root, file_path, relative) = self.resolve(path)?;
        let references = self
            .repository
            .lora_model_references(&relative)
            .map_err(storage_error)?;
        if references > 0 {
            return Err(format!(
                "This LoRA is used by {references} local image model configuration(s). Remove it from those models first."
            ));
        }
        std::fs::remove_file(&file_path)
            .map_err(|error| format!("Failed to delete the LoRA file: {error}"))?;
        self.repository
            .delete_lora(&relative)
            .map_err(storage_error)?;
        std::fs::remove_dir_all(root.join(LORA_COMPAT_CACHE_DIR)).ok();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(value: Value) -> Map<String, Value> {
        value.as_object().expect("object").clone()
    }

    #[test]
    fn explicit_safetensors_activation_tags_are_used_as_lora_keywords() {
        assert_eq!(
            keywords_from_safetensors_metadata(&metadata(serde_json::json!({
                "ss_activation_tags": "ArsMovieStill, cinematic still",
            }))),
            vec!["ArsMovieStill", "cinematic still"]
        );
        assert_eq!(
            keywords_from_safetensors_metadata(&metadata(serde_json::json!({
                "modelspec.trigger_phrase": "[\"alpha\", \"Alpha\", \" beta \"]",
            }))),
            vec!["alpha", "beta"]
        );
    }

    #[test]
    fn unambiguous_single_training_tag_is_used_as_a_lora_keyword() {
        assert_eq!(
            keywords_from_safetensors_metadata(&metadata(serde_json::json!({
                "ss_tag_frequency": "{\"1_ArsMovieStill\":{\"ArsMovieStill\":12}}",
            }))),
            vec!["ArsMovieStill"]
        );
    }

    #[test]
    fn ambiguous_training_tags_are_not_guessed_as_lora_keywords() {
        assert!(
            keywords_from_safetensors_metadata(&metadata(serde_json::json!({
                "ss_tag_frequency": "{\"dataset\": {\"one\": 1, \"two\": 2}}",
            })))
            .is_empty()
        );
    }

    #[test]
    fn explicit_base_model_metadata_detects_lora_architecture() {
        assert_eq!(
            architecture_from_safetensors_metadata(&metadata(serde_json::json!({
                "ss_base_model_version": "flux.2-klein-4b",
            }))),
            Some("flux2-klein-4b".to_owned())
        );
        assert_eq!(
            architecture_from_safetensors_metadata(&metadata(serde_json::json!({
                "ss_base_model_version": "flux2_klein_4b",
            })))
            .as_deref(),
            Some("flux2-klein-4b")
        );
        assert_eq!(
            normalize_lora_architecture("Z-Image Turbo").as_deref(),
            Some("z-image")
        );
        assert_eq!(
            normalize_lora_architecture("SDXL 1.0").as_deref(),
            Some("sdxl")
        );
        assert_eq!(normalize_lora_architecture("unknown"), None);
    }

    #[test]
    fn exact_lora_architecture_is_compatible_with_the_selected_profile() {
        assert_eq!(
            lora_compatibility(Some("flux2-klein-4b"), Some("flux-2-klein-4b")),
            LoraCompatibility::Compatible
        );
        assert_eq!(
            lora_compatibility(Some("flux1"), Some("flux-2-klein-4b")),
            LoraCompatibility::Incompatible
        );
        assert!(lora_architecture_supported("krea-2"));
        assert!(!lora_architecture_supported("sdxl"));
    }

    #[test]
    fn incomplete_family_metadata_does_not_claim_compatibility() {
        assert_eq!(
            lora_compatibility(Some("flux2"), Some("flux-2-klein-9b")),
            LoraCompatibility::Unknown
        );
        assert_eq!(
            lora_compatibility(Some("qwen-image"), Some("qwen-image-edit-2511")),
            LoraCompatibility::Unknown
        );
        assert_eq!(
            lora_compatibility(None, Some("flux-2-klein-9b")),
            LoraCompatibility::Unknown
        );
        assert_eq!(
            lora_compatibility(Some("flux2"), None),
            LoraCompatibility::Unknown
        );
    }

    #[test]
    fn manual_keywordless_lora_clears_stale_model_keywords() {
        let mut lora = StableDiffusionLora {
            path: "style.safetensors".to_owned(),
            multiplier: 0.8,
            is_high_noise: false,
            keywords: vec!["stale trigger".to_owned()],
        };
        let record = LoraRecord {
            path: "style.safetensors".to_owned(),
            filename: "style.safetensors".to_owned(),
            bytes_on_disk: 1,
            modified_at: 1,
            sha256: None,
            keywords: Vec::new(),
            keyword_source: LoraKeywordSource::Manual,
            architecture: None,
            architecture_source: LoraArchitectureSource::None,
        };
        apply_stored_lora_keywords(&mut lora, &record);
        assert!(lora.keywords.is_empty());
        let unresolved = LoraRecord {
            keyword_source: LoraKeywordSource::None,
            ..record
        };
        let mut kept = StableDiffusionLora {
            keywords: vec!["request trigger".to_owned()],
            ..lora
        };
        apply_stored_lora_keywords(&mut kept, &unresolved);
        assert_eq!(kept.keywords, vec!["request trigger"]);
    }
}
