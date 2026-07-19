use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex as TokioMutex;

use crate::utils::{log_info, log_info_global, log_warn_global};

mod sprout;
pub(crate) mod image_bundle;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct HfModelEntry {
    #[serde(rename = "modelId")]
    model_id: String,
    id: String,
    #[serde(default)]
    likes: i64,
    #[serde(default)]
    downloads: i64,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default, rename = "pipeline_tag")]
    pipeline_tag: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default, rename = "lastModified")]
    last_modified: Option<String>,
    #[serde(default, rename = "trendingScore")]
    trending_score: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct HfModelDetail {
    #[serde(rename = "modelId")]
    model_id: String,
    id: String,
    #[serde(default)]
    likes: i64,
    #[serde(default)]
    downloads: i64,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    siblings: Vec<HfSibling>,
    #[serde(default)]
    gguf: Option<HfGgufMeta>,
    #[serde(default, rename = "lastModified")]
    last_modified: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
struct HfGgufMeta {
    #[serde(default)]
    total: Option<u64>,
    #[serde(default)]
    architecture: Option<String>,
    #[serde(default)]
    context_length: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct HfSibling {
    rfilename: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct HfTreeEntry {
    #[serde(rename = "type")]
    entry_type: String,
    path: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    lfs: Option<HfLfsInfo>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct HfLfsInfo {
    #[serde(default)]
    size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfSearchResult {
    pub model_id: String,
    pub author: String,
    pub likes: i64,
    pub downloads: i64,
    pub tags: Vec<String>,
    pub pipeline_tag: Option<String>,
    pub last_modified: Option<String>,
    pub trending_score: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
struct HfOverviewResponse {
    #[serde(default, alias = "user")]
    name: Option<String>,
    #[serde(default)]
    fullname: Option<String>,
    #[serde(default, rename = "avatarUrl")]
    avatar_url: Option<String>,
    #[serde(default)]
    details: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default, rename = "isPro")]
    is_pro: Option<bool>,
    #[serde(default, rename = "numModels")]
    num_models: Option<u64>,
    #[serde(default, rename = "numDatasets")]
    num_datasets: Option<u64>,
    #[serde(default, rename = "numSpaces")]
    num_spaces: Option<u64>,
    #[serde(default, rename = "numLikes")]
    num_likes: Option<u64>,
    #[serde(default, rename = "numFollowers")]
    num_followers: Option<u64>,
    #[serde(default, rename = "numFollowing")]
    num_following: Option<u64>,
    #[serde(default, rename = "createdAt")]
    created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfAuthorOverview {
    pub name: String,
    pub fullname: Option<String>,
    pub avatar_url: Option<String>,
    pub details: Option<String>,
    pub kind: Option<String>,
    pub is_pro: bool,
    pub num_models: u64,
    pub num_datasets: u64,
    pub num_spaces: u64,
    pub num_likes: u64,
    pub num_followers: u64,
    pub num_following: u64,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfModelFile {
    pub filename: String,
    pub size: u64,
    pub quantization: String,
    pub is_mmproj: bool,
    pub is_mtp: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadedGgufModel {
    pub model_id: String,
    pub filename: String,
    pub path: String,
    pub size: u64,
    pub quantization: String,
    pub is_mmproj: bool,
    pub is_mtp: bool,
    pub architecture: Option<String>,
    pub context_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_role: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfModelInfo {
    pub model_id: String,
    pub author: String,
    pub likes: i64,
    pub downloads: i64,
    pub tags: Vec<String>,
    pub architecture: Option<String>,
    pub context_length: Option<u64>,
    pub parameter_count: Option<u64>,
    pub files: Vec<HfModelFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueuedDownload {
    pub id: String,
    pub model_id: String,
    pub filename: String,
    pub status: String, // "queued" | "downloading" | "complete" | "error" | "cancelled"
    pub phase: Option<String>, // null | "verifying"
    pub downloaded: u64,
    pub total: u64,
    pub speed_bytes_per_sec: u64,
    pub error: Option<String>,
    pub result_path: Option<String>,
    pub create_model_when_finished: bool,
    pub mmproj_file: MmprojFileLink,
    pub mtp_file: MmprojFileLink,
    pub mtp_bundled: bool,
    pub install_id: Option<String>,
    pub display_name: Option<String>,
    pub context_length: Option<u64>,
    pub kv_type: Option<String>,
    pub llama_offload_kqv: Option<bool>,
    pub llama_gpu_layers: Option<u32>,
    pub llama_model_offload_mode: Option<String>,
    pub download_role: Option<String>,
    pub queue_kind: Option<String>,
    pub asset_root: Option<String>,
    pub install_kind: Option<String>,
    pub variant: Option<String>,
    pub voice_id: Option<String>,
    pub download_url: Option<String>,
    pub destination_path: Option<String>,
    pub expected_size: Option<u64>,
    pub sha256: Option<String>,
    pub runtime_release: Option<String>,
    pub runtime_asset: Option<String>,
    pub force_redownload: bool,
    pub lora_keywords: Option<Vec<String>>,
    pub lora_base_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MmprojFileLink {
    Filename(String),
    Disabled(bool),
}

impl Default for MmprojFileLink {
    fn default() -> Self {
        Self::Disabled(false)
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueueDownloadMetadata {
    #[serde(default)]
    pub create_model_when_finished: bool,
    #[serde(default)]
    pub mmproj_file: MmprojFileLink,
    #[serde(default)]
    pub mtp_file: MmprojFileLink,
    #[serde(default)]
    pub mtp_bundled: bool,
    #[serde(default)]
    pub install_id: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub context_length: Option<u64>,
    #[serde(default)]
    pub kv_type: Option<String>,
    #[serde(default)]
    pub llama_offload_kqv: Option<bool>,
    #[serde(default)]
    pub llama_gpu_layers: Option<u32>,
    #[serde(default)]
    pub llama_model_offload_mode: Option<String>,
    #[serde(default)]
    pub download_role: Option<String>,
    #[serde(default)]
    pub queue_kind: Option<String>,
    #[serde(default)]
    pub asset_root: Option<String>,
    #[serde(default)]
    pub install_kind: Option<String>,
    #[serde(default)]
    pub variant: Option<String>,
    #[serde(default)]
    pub voice_id: Option<String>,
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub destination_path: Option<String>,
    #[serde(default)]
    pub expected_size: Option<u64>,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub runtime_release: Option<String>,
    #[serde(default)]
    pub runtime_asset: Option<String>,
    #[serde(default)]
    pub force_redownload: bool,
    #[serde(default)]
    pub lora_keywords: Option<Vec<String>>,
    #[serde(default)]
    pub lora_base_model: Option<String>,
}

struct DownloadQueueState {
    queue: Vec<QueuedDownload>,
    cancel_ids: std::collections::HashSet<String>,
    processing: bool,
    last_speed_sample: std::time::Instant,
    speed_bytes_window: u64,
}

lazy_static::lazy_static! {
    static ref HF_DOWNLOAD_QUEUE: Arc<TokioMutex<DownloadQueueState>> = Arc::new(TokioMutex::new(DownloadQueueState {
        queue: Vec::new(),
        cancel_ids: std::collections::HashSet::new(),
        processing: false,
        last_speed_sample: std::time::Instant::now(),
        speed_bytes_window: 0,
    }));

    static ref HF_AVATAR_CACHE: Arc<TokioMutex<HashMap<String, String>>> =
        Arc::new(TokioMutex::new(HashMap::new()));
}

#[derive(Debug, Deserialize)]
struct HfAvatarResponse {
    #[serde(rename = "avatarUrl")]
    avatar_url: String,
}

async fn fetch_avatar_url(client: &reqwest::Client, author: &str) -> String {
    let org_url = format!("https://huggingface.co/api/organizations/{}/avatar", author);
    if let Ok(resp) = client.get(&org_url).send().await {
        if resp.status().is_success() {
            if let Ok(parsed) = resp.json::<HfAvatarResponse>().await {
                return parsed.avatar_url;
            }
        }
    }

    let user_url = format!("https://huggingface.co/api/users/{}/avatar", author);
    if let Ok(resp) = client.get(&user_url).send().await {
        if resp.status().is_success() {
            if let Ok(parsed) = resp.json::<HfAvatarResponse>().await {
                return parsed.avatar_url;
            }
        }
    }

    String::new()
}

#[tauri::command]
pub async fn hf_get_avatars(app: AppHandle, authors: Vec<String>) -> Result<HashMap<String, String>, String> {
    let client = build_hf_client(&app)?;
    let cache = HF_AVATAR_CACHE.lock().await;
    let mut result: HashMap<String, String> = HashMap::new();

    let mut to_fetch: Vec<String> = Vec::new();
    for author in &authors {
        if let Some(url) = cache.get(author) {
            result.insert(author.clone(), url.clone());
        } else if !to_fetch.contains(author) {
            to_fetch.push(author.clone());
        }
    }

    drop(cache);

    let semaphore = Arc::new(tokio::sync::Semaphore::new(6));
    let client = Arc::new(client);
    let mut handles = Vec::new();

    for author in to_fetch {
        let sem = semaphore.clone();
        let c = client.clone();
        let a = author.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await;
            let url = fetch_avatar_url(&c, &a).await;
            (a, url)
        }));
    }

    let mut fetched: HashMap<String, String> = HashMap::new();
    for handle in handles {
        if let Ok((author, url)) = handle.await {
            fetched.insert(author, url);
        }
    }

    let mut cache = HF_AVATAR_CACHE.lock().await;
    for (author, url) in &fetched {
        cache.insert(author.clone(), url.clone());
        result.insert(author.clone(), url.clone());
    }

    for author in &authors {
        result.entry(author.clone()).or_default();
    }

    Ok(result)
}

fn emit_queue(app: &AppHandle, queue: &[QueuedDownload]) {
    let _ = app.emit("hf_download_queue", queue);
}

pub(crate) async fn enqueue_external_item(app: &AppHandle, item: QueuedDownload) {
    let mut state = HF_DOWNLOAD_QUEUE.lock().await;
    state.queue.push(item);
    emit_queue(app, &state.queue);
}

pub(crate) async fn update_external_progress(
    app: &AppHandle,
    queue_id: &str,
    status_label: &str,
    downloaded: u64,
    total: u64,
) {
    let mut state = HF_DOWNLOAD_QUEUE.lock().await;
    if let Some(item) = state.queue.iter_mut().find(|d| d.id == queue_id) {
        item.status = status_label.to_string();
        item.downloaded = downloaded;
        item.total = total;
    }
    emit_queue(app, &state.queue);
}

pub(crate) async fn update_external_speed(
    app: &AppHandle,
    queue_id: &str,
    speed_bytes_per_sec: u64,
) {
    let mut state = HF_DOWNLOAD_QUEUE.lock().await;
    if let Some(item) = state.queue.iter_mut().find(|d| d.id == queue_id) {
        item.speed_bytes_per_sec = speed_bytes_per_sec;
    }
    emit_queue(app, &state.queue);
}

pub(crate) async fn finish_external_item(
    app: &AppHandle,
    queue_id: &str,
    success: bool,
    error: Option<String>,
) {
    let mut state = HF_DOWNLOAD_QUEUE.lock().await;
    if let Some(item) = state.queue.iter_mut().find(|d| d.id == queue_id) {
        item.status = if success { "complete" } else { "error" }.to_string();
        item.phase = None;
        item.error = error;
        if success && item.total > 0 {
            item.downloaded = item.total;
        }
        item.speed_bytes_per_sec = 0;
    }
    state.cancel_ids.remove(queue_id);
    emit_queue(app, &state.queue);
}

pub(crate) async fn cancel_external_item(app: &AppHandle, queue_id: &str) {
    let mut state = HF_DOWNLOAD_QUEUE.lock().await;
    if let Some(item) = state.queue.iter_mut().find(|d| d.id == queue_id) {
        item.status = "cancelled".to_string();
        item.phase = None;
        item.speed_bytes_per_sec = 0;
    }
    state.cancel_ids.remove(queue_id);
    emit_queue(app, &state.queue);
}

pub(crate) async fn external_item_cancel_requested(queue_id: &str) -> bool {
    let state = HF_DOWNLOAD_QUEUE.lock().await;
    state.cancel_ids.contains(queue_id)
}

fn default_hf_models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let lettuce_dir = crate::utils::lettuce_dir(app)?;
    Ok(lettuce_dir.join("models").join("gguf"))
}

fn custom_llm_models_dir(app: &AppHandle) -> Option<PathBuf> {
    crate::infra::model_storage::read_custom_dir(app, "customLlmModelsDir")
}

fn hf_models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = match custom_llm_models_dir(app) {
        Some(custom) => custom,
        None => default_hf_models_dir(app)?,
    };
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to create GGUF models dir: {}", e),
            )
        })?;
    }
    Ok(dir)
}

// GGUF header parser
#[derive(Debug, Default)]
struct GgufModelMeta {
    architecture: Option<String>,
    block_count: Option<u64>,
    embedding_length: Option<u64>,
    head_count: Option<u64>,
    head_count_kv: Option<u64>,
    context_length: Option<u64>,
    feed_forward_length: Option<u64>,
    file_type: Option<u32>,
    /// Sliding window size for SWA architectures (Gemma 2, Cohere)
    sliding_window: Option<u64>,
    /// KV LoRA rank for MLA architectures (DeepSeek V2/V3)
    kv_lora_rank: Option<u64>,
    /// Per-head key dimension (used for MLA KV cache sizing)
    key_length: Option<u64>,
    /// Per-head value dimension (used for MLA KV cache sizing)
    value_length: Option<u64>,
    /// Total routed experts per MoE layer (e.g. 8 for Mixtral, 60 for Qwen3-MoE)
    expert_count: Option<u64>,
    /// Routed experts actually executed per token (top-k)
    expert_used_count: Option<u64>,
    /// Shared (always-on) experts per layer (DeepSeek V2/V3)
    expert_shared_count: Option<u64>,
    /// Per-expert FFN hidden dim (usually smaller than dense feed_forward_length)
    expert_feed_forward_length: Option<u64>,
    /// MTP/NextN speculative layers included in block_count (GLM-4.x, DeepSeek V3.x)
    nextn_predict_layers: Option<u64>,
    /// Number of metadata KV pairs declared in header (for truncation detection)
    metadata_kv_count: u64,
    /// Number of KV pairs actually parsed before buffer ran out
    parsed_kv_count: u64,
}

struct GgufReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> GgufReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn read_bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.pos + n > self.data.len() {
            return None;
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Some(slice)
    }

    fn read_u8(&mut self) -> Option<u8> {
        self.read_bytes(1).map(|b| b[0])
    }

    fn read_u32(&mut self) -> Option<u32> {
        self.read_bytes(4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn read_i32(&mut self) -> Option<i32> {
        self.read_bytes(4)
            .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn read_u64(&mut self) -> Option<u64> {
        self.read_bytes(8)
            .map(|b| u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    }

    fn read_i64(&mut self) -> Option<i64> {
        self.read_bytes(8)
            .map(|b| i64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    }

    fn read_f32(&mut self) -> Option<f32> {
        self.read_bytes(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn read_f64(&mut self) -> Option<f64> {
        self.read_bytes(8)
            .map(|b| f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    }

    fn read_string(&mut self) -> Option<String> {
        let len = self.read_u64()? as usize;
        if len > self.remaining() {
            return None;
        }
        let bytes = self.read_bytes(len)?;
        String::from_utf8(bytes.to_vec()).ok()
    }

    fn read_bool(&mut self) -> Option<bool> {
        self.read_u8().map(|b| b != 0)
    }

    fn skip_value(&mut self, value_type: u32) -> Option<()> {
        match value_type {
            0 => {
                self.read_u8()?;
            } // UINT8
            1 => {
                self.read_u8()?;
            } // INT8
            2 => {
                self.read_bytes(2)?;
            } // UINT16
            3 => {
                self.read_bytes(2)?;
            } // INT16
            4 => {
                self.read_u32()?;
            } // UINT32
            5 => {
                self.read_i32()?;
            } // INT32
            6 => {
                self.read_f32()?;
            } // FLOAT32
            7 => {
                self.read_bool()?;
            } // BOOL
            8 => {
                self.read_string()?;
            } // STRING
            9 => {
                // ARRAY
                let arr_type = self.read_u32()?;
                let arr_len = self.read_u64()?;
                for _ in 0..arr_len {
                    self.skip_value(arr_type)?;
                }
            }
            10 => {
                self.read_u64()?;
            } // UINT64
            11 => {
                self.read_i64()?;
            } // INT64
            12 => {
                self.read_f64()?;
            } // FLOAT64
            _ => return None, // Unknown type
        }
        Some(())
    }

    fn read_value_as_u64(&mut self, value_type: u32) -> Option<u64> {
        match value_type {
            0 => self.read_u8().map(|v| v as u64),
            1 => self.read_u8().map(|v| v as i8 as u64),
            2 => self
                .read_bytes(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]) as u64),
            3 => self
                .read_bytes(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]) as u64),
            4 => self.read_u32().map(|v| v as u64),
            5 => self.read_i32().map(|v| v as u64),
            10 => self.read_u64(),
            11 => self.read_i64().map(|v| v as u64),
            9 => {
                let arr_type = self.read_u32()?;
                let arr_len = self.read_u64()?;
                let mut first: Option<u64> = None;
                for i in 0..arr_len {
                    if i == 0 {
                        first = self.read_value_as_u64(arr_type);
                        if first.is_none() {
                            self.skip_value(arr_type)?;
                        }
                    } else {
                        self.skip_value(arr_type)?;
                    }
                }
                first
            }
            _ => None,
        }
    }

    /// Read a GGUF value as u32 (coercing integer types)
    fn read_value_as_u32(&mut self, value_type: u32) -> Option<u32> {
        self.read_value_as_u64(value_type).map(|v| v as u32)
    }
}

fn parse_gguf_meta(data: &[u8]) -> Option<GgufModelMeta> {
    log_info_global(
        "gguf_parser",
        format!("parse start: buffer={} bytes", data.len()),
    );
    let mut reader = GgufReader::new(data);

    let magic = reader.read_bytes(4)?;
    if magic != b"GGUF" {
        log_warn_global("gguf_parser", format!("magic mismatch: got {:02x?}", magic));
        return None;
    }

    let version = reader.read_u32()?;
    if !(2..=3).contains(&version) {
        log_warn_global("gguf_parser", format!("unsupported version: {}", version));
        return None;
    }

    let tensor_count = reader.read_u64()?;
    let metadata_kv_count = reader.read_u64()?;
    log_info_global(
        "gguf_parser",
        format!(
            "header: version={} tensors={} kv_pairs={} body_offset={}",
            version, tensor_count, metadata_kv_count, reader.pos
        ),
    );

    let mut meta = GgufModelMeta {
        metadata_kv_count,
        ..Default::default()
    };

    let start_pos = reader.pos;
    let mut pass1_scanned: u64 = 0;
    for _ in 0..metadata_kv_count {
        if reader.remaining() < 8 {
            log_warn_global(
                "gguf_parser",
                format!(
                    "pass1 short: after {} keys, remaining={}, pos={}",
                    pass1_scanned,
                    reader.remaining(),
                    reader.pos
                ),
            );
            break;
        }
        let key_pos = reader.pos;
        let key = match reader.read_string() {
            Some(k) => k,
            None => {
                log_warn_global(
                    "gguf_parser",
                    format!(
                        "pass1 key read fail at pos={} after {} keys",
                        key_pos, pass1_scanned
                    ),
                );
                break;
            }
        };
        let value_type = match reader.read_u32() {
            Some(t) => t,
            None => {
                log_warn_global(
                    "gguf_parser",
                    format!(
                        "pass1 value_type read fail at key='{}' pos={}",
                        key, reader.pos
                    ),
                );
                break;
            }
        };

        if key == "general.architecture" && value_type == 8 {
            meta.architecture = reader.read_string();
            log_info_global(
                "gguf_parser",
                format!(
                    "pass1 found architecture={:?} after {} keys",
                    meta.architecture, pass1_scanned
                ),
            );
            break;
        } else if reader.skip_value(value_type).is_none() {
            log_warn_global(
                "gguf_parser",
                format!(
                    "pass1 skip_value failed: key='{}' type={} pos={} remaining={}",
                    key,
                    value_type,
                    reader.pos,
                    reader.remaining()
                ),
            );
            break;
        }
        pass1_scanned += 1;
    }
    if meta.architecture.is_none() {
        log_warn_global(
            "gguf_parser",
            format!(
                "pass1 finished without finding architecture (scanned {} keys)",
                pass1_scanned
            ),
        );
    }

    let arch = meta
        .architecture
        .clone()
        .unwrap_or_else(|| "llama".to_string());

    let key_block_count = format!("{}.block_count", arch);
    let key_embedding_length = format!("{}.embedding_length", arch);
    let key_head_count = format!("{}.attention.head_count", arch);
    let key_head_count_kv = format!("{}.attention.head_count_kv", arch);
    let key_context_length = format!("{}.context_length", arch);
    let key_feed_forward = format!("{}.feed_forward_length", arch);
    let key_sliding_window = format!("{}.attention.sliding_window", arch);
    let key_kv_lora_rank = format!("{}.attention.kv_lora_rank", arch);
    let key_key_length = format!("{}.attention.key_length", arch);
    let key_value_length = format!("{}.attention.value_length", arch);
    let key_expert_count = format!("{}.expert_count", arch);
    let key_expert_used_count = format!("{}.expert_used_count", arch);
    let key_expert_shared_count = format!("{}.expert_shared_count", arch);
    let key_expert_ffn = format!("{}.expert_feed_forward_length", arch);
    let key_nextn = format!("{}.nextn_predict_layers", arch);

    reader.pos = start_pos;
    let mut parsed: u64 = 0;
    log_info_global(
        "gguf_parser",
        format!("pass2 start at pos={} arch={}", start_pos, arch),
    );
    for kv_index in 0..metadata_kv_count {
        if reader.remaining() < 8 {
            log_warn_global(
                "gguf_parser",
                format!(
                    "pass2 short at kv #{}: remaining={} pos={} buffer_total={}",
                    kv_index,
                    reader.remaining(),
                    reader.pos,
                    data.len()
                ),
            );
            break;
        }
        let key_pos = reader.pos;
        let key = match reader.read_string() {
            Some(k) => k,
            None => {
                log_warn_global(
                    "gguf_parser",
                    format!(
                        "pass2 key read failed at kv #{} pos={} remaining={}",
                        kv_index,
                        key_pos,
                        reader.remaining()
                    ),
                );
                break;
            }
        };
        let value_type = match reader.read_u32() {
            Some(t) => t,
            None => {
                log_warn_global(
                    "gguf_parser",
                    format!(
                        "pass2 value_type read failed at kv #{} key='{}' pos={}",
                        kv_index, key, reader.pos
                    ),
                );
                break;
            }
        };

        let pos_before_value = reader.pos;
        let mut matched = "";
        if key == "general.file_type" {
            meta.file_type = reader.read_value_as_u32(value_type);
            matched = "general.file_type";
        } else if key == key_block_count {
            meta.block_count = reader.read_value_as_u64(value_type);
            matched = "block_count";
        } else if key == key_embedding_length {
            meta.embedding_length = reader.read_value_as_u64(value_type);
            matched = "embedding_length";
        } else if key == key_head_count {
            meta.head_count = reader.read_value_as_u64(value_type);
            matched = "head_count";
        } else if key == key_head_count_kv {
            meta.head_count_kv = reader.read_value_as_u64(value_type);
            matched = "head_count_kv";
        } else if key == key_context_length {
            meta.context_length = reader.read_value_as_u64(value_type);
            matched = "context_length";
        } else if key == key_feed_forward {
            meta.feed_forward_length = reader.read_value_as_u64(value_type);
            matched = "feed_forward_length";
        } else if key == key_sliding_window {
            meta.sliding_window = reader.read_value_as_u64(value_type);
            matched = "sliding_window";
        } else if key == key_kv_lora_rank {
            meta.kv_lora_rank = reader.read_value_as_u64(value_type);
            matched = "kv_lora_rank";
        } else if key == key_key_length {
            meta.key_length = reader.read_value_as_u64(value_type);
            matched = "key_length";
        } else if key == key_value_length {
            meta.value_length = reader.read_value_as_u64(value_type);
            matched = "value_length";
        } else if key == key_expert_count {
            meta.expert_count = reader.read_value_as_u64(value_type);
            matched = "expert_count";
        } else if key == key_expert_used_count {
            meta.expert_used_count = reader.read_value_as_u64(value_type);
            matched = "expert_used_count";
        } else if key == key_expert_shared_count {
            meta.expert_shared_count = reader.read_value_as_u64(value_type);
            matched = "expert_shared_count";
        } else if key == key_expert_ffn {
            meta.expert_feed_forward_length = reader.read_value_as_u64(value_type);
            matched = "expert_feed_forward_length";
        } else if key == key_nextn {
            meta.nextn_predict_layers = reader.read_value_as_u64(value_type);
            matched = "nextn_predict_layers";
        }

        if !matched.is_empty() {
            log_info_global(
                "gguf_parser",
                format!(
                    "pass2 kv #{} key='{}' matched={} type={} pos_before={} pos_after={}",
                    kv_index, key, matched, value_type, pos_before_value, reader.pos
                ),
            );
        }

        if reader.pos == pos_before_value {
            if reader.skip_value(value_type).is_none() {
                log_warn_global(
                    "gguf_parser",
                    format!(
                        "pass2 skip_value failed: kv #{} key='{}' type={} pos={} remaining={}",
                        kv_index,
                        key,
                        value_type,
                        reader.pos,
                        reader.remaining()
                    ),
                );
                break;
            }
        }

        parsed += 1;
    }
    meta.parsed_kv_count = parsed;
    log_info_global(
        "gguf_parser",
        format!(
            "parse done: parsed={}/{} arch={:?} blocks={:?} heads_kv={:?} expert_count={:?}",
            parsed,
            metadata_kv_count,
            meta.architecture,
            meta.block_count,
            meta.head_count_kv,
            meta.expert_count
        ),
    );

    Some(meta)
}

fn read_local_gguf_meta(path: &Path) -> Option<GgufModelMeta> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut primary = vec![0u8; 524_288];
    let primary_read = file.read(&mut primary).ok()?;
    primary.truncate(primary_read);

    let primary_meta = parse_gguf_meta(&primary);
    let essentials_ok = |m: &GgufModelMeta| {
        m.architecture.is_some()
            && m.block_count.is_some()
            && m.embedding_length.is_some()
            && m.head_count.is_some()
            && m.context_length.is_some()
    };
    if primary_meta.as_ref().is_some_and(essentials_ok) {
        return primary_meta;
    }

    let mut file = std::fs::File::open(path).ok()?;
    let mut fallback = vec![0u8; 10_485_760];
    let fallback_read = file.read(&mut fallback).ok()?;
    fallback.truncate(fallback_read);

    parse_gguf_meta(&fallback).or(primary_meta)
}

// Runability scoring
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunabilityScore {
    pub filename: String,
    pub score: u32,
    pub label: String,
    pub fits_in_ram: bool,
    pub fits_in_vram: bool,
    pub gpu_mode: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunabilityFileInput {
    pub filename: String,
    pub size: u64,
    pub quantization: String,
}

fn quant_quality_score(quant: &str) -> f64 {
    match quant.to_uppercase().as_str() {
        "F32" | "BF16" | "F16" => 100.0,
        "UD-Q8_K_XL" => 96.0,
        "Q8_K" | "Q8_K_S" | "Q8_K_L" | "Q8_K_XL" => 95.0,
        "UD-Q6_K_XL" => 92.0,
        "Q8_0" => 90.0,
        "Q6_K" | "Q6_K_S" | "Q6_K_L" | "Q6_K_XL" => 90.0,
        "UD-Q5_K_XL" => 88.0,
        "Q5_K_M" | "Q5_K_L" | "Q5_K_XL" | "Q5_K" => 85.0,
        "Q5_K_S" => 80.0,
        "UD-Q4_K_XL" => 82.0,
        "Q5_0" | "Q5_1" => 70.0,
        "Q4_K_M" | "Q4_K_L" | "Q4_K_XL" | "Q4_K" => 75.0,
        "Q4_K_S" => 70.0,
        "UD-IQ4_XS" | "UD-IQ4_NL" => 80.0,
        "IQ4_XS" | "IQ4_NL" => 72.0,
        "Q4_0" | "Q4_1" => 60.0,
        "UD-Q3_K_XL" => 70.0,
        "Q3_K_M" | "Q3_K_L" | "Q3_K_XL" | "Q3_K" => 60.0,
        "Q3_K_S" => 50.0,
        "UD-IQ3_XXS" => 60.0,
        "IQ3_M" | "IQ3_S" => 52.0,
        "IQ3_XS" | "IQ3_XXS" => 45.0,
        "UD-Q2_K_XL" => 55.0,
        "Q2_K" | "Q2_K_S" | "Q2_K_M" | "Q2_K_L" | "Q2_K_XL" => 35.0,
        "UD-IQ2_M" => 50.0,
        "UD-IQ2_XXS" => 40.0,
        "IQ2_M" | "IQ2_S" | "IQ2_XS" | "IQ2_XXS" => 25.0,
        "UD-IQ1_M" => 30.0,
        "UD-IQ1_S" => 22.0,
        "IQ1_M" | "IQ1_S" => 15.0,
        "MXFP4_MOE" => 70.0,
        _ => 50.0,
    }
}

/// Compute KV cache base cost per token: block_count × effective_kv_dim × 2 (K+V).
/// Multiply by bytes_per_value (F16=2.0, Q8_0=1.0, Q4_0=0.5) and context_length
/// to get actual KV cache bytes.
///
/// Handles architecture-specific optimizations:
/// - **Gemma 2 (SWA)**: Uses sliding window attention — KV cache is capped at
///   the window size instead of growing with context length.
/// - **DeepSeek V2/V3 (MLA)**: Multi-Head Latent Attention compresses KV cache
///   using a low-rank projection, dramatically reducing per-token cost.
fn kv_base_per_token(meta: &GgufModelMeta) -> Option<f64> {
    let nextn = meta.nextn_predict_layers.unwrap_or(0);
    let blocks = meta.block_count?.saturating_sub(nextn).max(1) as f64;
    let embd = meta.embedding_length? as f64;
    let heads = meta.head_count.filter(|&h| h > 0)? as f64;
    let heads_kv = meta.head_count_kv.unwrap_or(meta.head_count?) as f64;

    let arch = meta
        .architecture
        .as_deref()
        .unwrap_or("llama")
        .to_lowercase();

    // DeepSeek MLA: KV cache uses compressed latent dimension instead of full head dim
    if let (true, Some(lora_rank)) = (
        arch.starts_with("deepseek") || arch == "deepseek2",
        meta.kv_lora_rank,
    ) {
        let lora_rank = lora_rank as f64;
        // MLA stores compressed KV: block_count × lora_rank × 2 (K+V)
        // Key uses rope_dim + lora_rank, value uses lora_rank
        // Simplified: use lora_rank for both as a conservative estimate
        return Some(blocks * lora_rank * 2.0);
    }

    Some(blocks * (embd * heads_kv / heads) * 2.0)
}

/// For architectures with sliding window attention (Gemma 2, Cohere),
/// the effective context for KV cache is capped at the window size.
/// Returns the effective context to use for KV cache calculation.
fn effective_kv_context(meta: &GgufModelMeta, requested_ctx: u64) -> u64 {
    let arch = meta
        .architecture
        .as_deref()
        .unwrap_or("llama")
        .to_lowercase();
    // Gemma 2 and Cohere use sliding window attention
    if arch == "gemma2" || arch == "cohere" {
        if let Some(window) = meta.sliding_window {
            return requested_ctx.min(window);
        }
    }
    requested_ctx
}

#[allow(dead_code)]
fn kv_bytes_per_value(kv_type: &str) -> f64 {
    match kv_type {
        "f32" => 4.0,
        "f16" => 2.0,
        "q8_1" => 1.0625,
        "q8_0" => 1.0,
        "q5_1" => 0.75,
        "q5_0" => 0.6875,
        "iq4_nl" => 0.5625,
        "q4_0" => 0.5,
        _ => 2.0,
    }
}

fn score_label(score: u32) -> String {
    match score {
        80..=100 => "excellent",
        60..=79 => "good",
        40..=59 => "marginal",
        20..=39 => "poor",
        _ => "unrunnable",
    }
    .to_string()
}

fn is_moe(meta: &GgufModelMeta) -> bool {
    meta.expert_count.unwrap_or(0) > 0 && meta.expert_used_count.unwrap_or(0) > 0
}

fn active_weight_ratio(meta: &GgufModelMeta) -> f64 {
    if !is_moe(meta) {
        return 1.0;
    }
    let expert_count = meta.expert_count.unwrap_or(0).max(1) as f64;
    let used = meta.expert_used_count.unwrap_or(0) as f64;
    let shared = meta.expert_shared_count.unwrap_or(0) as f64;
    let active_experts = used + shared;
    let total_experts = expert_count + shared;
    let attn_frac = 0.10_f64;
    let ffn_frac = 1.0 - attn_frac;
    let active_ffn = ffn_frac * (active_experts / total_experts.max(1.0));
    (attn_frac + active_ffn).clamp(0.05, 1.0)
}

fn effective_model_size(file_size: u64, meta: Option<&GgufModelMeta>) -> u64 {
    let Some(meta) = meta else {
        return file_size;
    };
    let nextn = meta.nextn_predict_layers.unwrap_or(0);
    let Some(blocks) = meta.block_count.filter(|&b| b > 0) else {
        return file_size;
    };
    if nextn == 0 {
        return file_size;
    }
    let share =
        (file_size as f64 * (nextn as f64 / blocks as f64) * 1.5).min(file_size as f64 * 0.10);
    file_size.saturating_sub(share as u64)
}

fn is_qat_name(name: &str) -> bool {
    name.to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|part| part == "qat")
}

fn quant_quality_with_qat(quant: &str, qat: bool) -> f64 {
    let base = quant_quality_score(quant);
    if qat {
        base.max(90.0)
    } else {
        base
    }
}

fn is_mtp_asset(name: &str) -> bool {
    let lower = name.to_lowercase();
    let basename = lower.rsplit('/').next().unwrap_or(&lower);
    basename.starts_with("mtp-")
        || basename.contains("-mtp.")
        || basename.contains("_mtp.")
        || lower.contains("/mtp/")
        || lower.starts_with("mtp/")
}

fn is_draft_file(name: &str) -> bool {
    name.to_lowercase().contains("draft") || is_mtp_asset(name)
}

fn compute_overhead(model_size: u64, active_ratio: f64) -> u64 {
    let active_size = (model_size as f64 * active_ratio.clamp(0.05, 1.0)) as u64;
    let five_pct = (active_size as f64 * 0.05) as u64;
    five_pct.max(200_000_000)
}

/// Core scoring for a single configuration. All parameters are concrete values.
/// `total_available` should already account for unified memory (use max, not sum).
///
/// Returns (score, fits_in_ram, fits_in_vram, memory_score, gpu_score, kv_score, gpu_mode).
fn score_configuration(
    model_size: u64,
    quant_quality: f64,
    kv_cache_bytes: u64,
    available_ram: u64,
    total_available: u64,
    available_vram: u64,
    active_ratio: f64,
) -> (u32, bool, bool, u32, u32, u32, &'static str) {
    let overhead = compute_overhead(model_size, active_ratio);
    let total_needed = model_size
        .saturating_add(kv_cache_bytes)
        .saturating_add(overhead);
    let fits_in_ram = total_available > 0 && total_needed <= total_available;
    let ram_budget = (available_ram as f64 * 0.90) as u64;
    let vram_budget = (available_vram as f64 * 0.90) as u64;
    let model_fits_ram = available_ram > 0 && model_size.saturating_add(overhead) <= ram_budget;
    let kv_fits_vram = available_vram > 0 && kv_cache_bytes.saturating_add(overhead) <= vram_budget;

    // Memory fitness (25%)
    let memory_score = if total_available == 0 {
        50.0
    } else if total_needed > total_available {
        0.0
    } else {
        let ratio = total_available as f64 / total_needed as f64;
        if ratio < 1.05 {
            40.0
        } else if ratio < 1.2 {
            60.0
        } else if ratio < 1.5 {
            75.0
        } else if ratio < 2.0 {
            85.0
        } else if ratio < 3.0 {
            95.0
        } else {
            100.0
        }
    };

    //   GPU acceleration (35%)
    //   1. Everything fits in VRAM → 100 (blazing fast)
    //   2. Model fits, KV/compute spills → 70-95 (fast inference, slow prefill)
    //   3. Model fits in RAM but not VRAM → 62-92 (RAM-backed model with VRAM context)
    //   4. Model spills to RAM → 10-70 (partial layer offload via -ngl)
    let (gpu_score, fits_in_vram, gpu_mode) = if available_vram > 0 {
        if total_needed <= vram_budget {
            // Full offload: model + KV + compute all fit in VRAM
            (100.0, true, "full")
        } else if model_size == 0 {
            (10.0, false, "cpu")
        } else if model_size <= vram_budget {
            // Model weights fit, KV/compute spills to system RAM
            let remaining = vram_budget.saturating_sub(model_size);
            let spill = kv_cache_bytes.saturating_add(overhead);
            let fit_ratio = if spill > 0 {
                (remaining as f64 / spill as f64).min(1.0)
            } else {
                1.0
            };
            let mode = if fit_ratio >= 0.8 {
                "nearFull"
            } else if fit_ratio >= 0.4 {
                "kvSpill"
            } else {
                "kvHeavySpill"
            };
            // 70-95: good experience, layers all on GPU
            (70.0 + fit_ratio * 25.0, true, mode)
        } else if model_fits_ram && kv_fits_vram {
            let ram_fit_ratio = if model_size.saturating_add(overhead) > 0 {
                (ram_budget as f64 / model_size.saturating_add(overhead) as f64).min(1.0)
            } else {
                1.0
            };
            let kv_fit_ratio = if kv_cache_bytes.saturating_add(overhead) > 0 {
                (vram_budget as f64 / kv_cache_bytes.saturating_add(overhead) as f64).min(1.0)
            } else {
                1.0
            };
            (
                78.0 + ram_fit_ratio * 12.0 + kv_fit_ratio * 10.0,
                false,
                "ramModelVramCtx",
            )
        } else if model_fits_ram {
            let ram_fit_ratio = if model_size.saturating_add(overhead) > 0 {
                (ram_budget as f64 / model_size.saturating_add(overhead) as f64).min(1.0)
            } else {
                1.0
            };
            (62.0 + ram_fit_ratio * 18.0, false, "ramModelRamCtx")
        } else {
            // Model doesn't fit: partial layer offload
            let offload_ratio = (vram_budget as f64 / model_size as f64).min(1.0);
            let mode = if offload_ratio >= 0.75 {
                "mostLayers"
            } else if offload_ratio >= 0.5 {
                "halfLayers"
            } else if offload_ratio >= 0.2 {
                "fewLayers"
            } else {
                "cpu"
            };
            // 10-70: scales with how many layers can be offloaded
            (10.0 + offload_ratio * 60.0, false, mode)
        }
    } else {
        (0.0, false, "cpu")
    };

    // KV headroom (15%)
    let kv_score = if kv_cache_bytes > 0 {
        let headroom = total_available
            .saturating_sub(model_size)
            .saturating_sub(overhead);
        if headroom == 0 {
            0.0
        } else if headroom >= kv_cache_bytes {
            let ratio = headroom as f64 / kv_cache_bytes as f64;
            if ratio >= 2.0 {
                100.0
            } else {
                50.0 + 50.0 * (ratio - 1.0)
            }
        } else {
            50.0 * (headroom as f64 / kv_cache_bytes as f64)
        }
    } else {
        50.0
    };

    let raw = memory_score * 0.25 + gpu_score * 0.35 + kv_score * 0.15 + quant_quality * 0.25;
    let capped = if memory_score == 0.0 {
        raw.min(10.0)
    } else {
        raw
    };
    let score = (capped.round() as u32).min(100);

    (
        score,
        fits_in_ram,
        fits_in_vram,
        (memory_score.round() as u32).min(100),
        (gpu_score.round() as u32).min(100),
        (kv_score.round() as u32).min(100),
        gpu_mode,
    )
}

fn resolve_total_available(available_ram: u64, available_vram: u64, unified: bool) -> u64 {
    if unified && available_vram > 0 {
        available_ram.max(available_vram)
    } else {
        available_ram.saturating_add(available_vram)
    }
}

pub(crate) struct LlamaRuntimeDefaults {
    pub context_length: u64,
    pub kv_bytes_per_value: f64,
}

pub(crate) fn llama_runtime_defaults(app: &AppHandle) -> LlamaRuntimeDefaults {
    let settings: Option<crate::chat_manager::types::Settings> =
        crate::storage_manager::settings::read_settings_typed(app)
            .ok()
            .flatten();
    let advanced = settings.as_ref().and_then(|s| s.advanced_settings.as_ref());
    let context_length = advanced
        .and_then(|a| a.llama_default_context_length)
        .map(u64::from)
        .filter(|value| *value >= 512)
        .unwrap_or(8192);
    let kv_type = advanced
        .and_then(|a| a.llama_default_kv_cache_type.clone())
        .unwrap_or_else(|| "auto".to_string());
    let kv_bytes_per_value = if kv_type == "auto" {
        2.0
    } else {
        kv_bytes_per_value(&kv_type)
    };
    LlamaRuntimeDefaults {
        context_length,
        kv_bytes_per_value,
    }
}

async fn resolve_runnability_hardware(
    sprout_url: Option<&str>,
    sprout_api_key: Option<&str>,
) -> Result<(Option<u64>, Option<u64>, bool, Option<bool>), String> {
    if let Some(url) = sprout_url.filter(|url| !url.trim().is_empty()) {
        let hardware = sprout::fetch_sprout_hardware(url, sprout_api_key).await?;
        return Ok((
            Some(hardware.available_ram),
            hardware.available_vram,
            hardware.supports_gpu_offload,
            Some(hardware.unified),
        ));
    }

    let supports_gpu_offload = crate::llama_cpp::supports_gpu_offload();
    let available_vram = if supports_gpu_offload {
        crate::llama_cpp::available_vram_bytes()
    } else {
        None
    };
    Ok((
        crate::llama_cpp::available_memory_bytes(),
        available_vram,
        supports_gpu_offload,
        None,
    ))
}

fn compute_scores(
    files: &[RunabilityFileInput],
    model_id: &str,
    meta: Option<&GgufModelMeta>,
    available_ram: Option<u64>,
    available_vram: Option<u64>,
    unified_override: Option<bool>,
    defaults: &LlamaRuntimeDefaults,
) -> Vec<RunabilityScore> {
    let ram = available_ram.unwrap_or(0);
    let vram = available_vram.unwrap_or(0);
    let unified = unified_override.unwrap_or_else(crate::llama_cpp::is_unified_memory);
    let total_available = resolve_total_available(ram, vram, unified);

    let default_ctx = defaults.context_length;
    let effective_ctx = meta
        .map(|m| effective_kv_context(m, default_ctx))
        .unwrap_or(default_ctx);
    let kv_8k: u64 = meta
        .and_then(kv_base_per_token)
        .map(|base| (base * defaults.kv_bytes_per_value * effective_ctx as f64) as u64)
        .unwrap_or(0);
    let active_ratio = meta.map(active_weight_ratio).unwrap_or(1.0);
    let repo_qat = is_qat_name(model_id);

    files
        .iter()
        .map(|file| {
            let qat = repo_qat || is_qat_name(&file.filename);
            let (score, fits_in_ram, fits_in_vram, .., gpu_mode) = score_configuration(
                effective_model_size(file.size, meta),
                quant_quality_with_qat(&file.quantization, qat),
                kv_8k,
                ram,
                total_available,
                vram,
                active_ratio,
            );
            RunabilityScore {
                filename: file.filename.clone(),
                score,
                label: score_label(score),
                fits_in_ram,
                fits_in_vram,
                gpu_mode: gpu_mode.to_string(),
            }
        })
        .collect()
}

// Recommendation engine
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelArchInfo {
    pub architecture: Option<String>,
    pub block_count: Option<u64>,
    pub embedding_length: Option<u64>,
    pub head_count: Option<u64>,
    pub head_count_kv: Option<u64>,
    pub context_length: Option<u64>,
    pub feed_forward_length: Option<u64>,
    pub file_type: Option<u32>,
    pub sliding_window: Option<u64>,
    pub kv_lora_rank: Option<u64>,
    pub key_length: Option<u64>,
    pub value_length: Option<u64>,
    pub expert_count: Option<u64>,
    pub expert_used_count: Option<u64>,
    pub expert_shared_count: Option<u64>,
    pub expert_feed_forward_length: Option<u64>,
    pub nextn_predict_layers: Option<u64>,
    pub is_moe: bool,
    pub active_weight_ratio: Option<f64>,
    pub incomplete_parse: bool,
}

impl From<&GgufModelMeta> for ModelArchInfo {
    fn from(m: &GgufModelMeta) -> Self {
        let moe = is_moe(m);
        Self {
            architecture: m.architecture.clone(),
            block_count: m.block_count,
            embedding_length: m.embedding_length,
            head_count: m.head_count,
            head_count_kv: m.head_count_kv,
            context_length: m.context_length,
            feed_forward_length: m.feed_forward_length,
            file_type: m.file_type,
            sliding_window: m.sliding_window,
            kv_lora_rank: m.kv_lora_rank,
            key_length: m.key_length,
            value_length: m.value_length,
            expert_count: m.expert_count,
            expert_used_count: m.expert_used_count,
            expert_shared_count: m.expert_shared_count,
            expert_feed_forward_length: m.expert_feed_forward_length,
            nextn_predict_layers: m.nextn_predict_layers,
            is_moe: moe,
            active_weight_ratio: if moe {
                Some(active_weight_ratio(m))
            } else {
                None
            },
            incomplete_parse: m.architecture.is_none()
                || m.block_count.is_none()
                || m.embedding_length.is_none()
                || m.head_count.is_none()
                || m.context_length.is_none(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationData {
    pub available_ram: u64,
    pub available_vram: u64,
    pub supports_gpu_offload: bool,
    /// Whether RAM and VRAM share the same pool (Apple Silicon, iGPU)
    pub unified_memory: bool,
    /// Effective total memory (max or sum depending on unified)
    pub total_available: u64,
    /// KV cache base cost per token (before bytes_per_value multiplier).
    /// Frontend calculates: kv_bytes = kv_base * bytes_per_value * context_length
    pub kv_base_per_token: Option<f64>,
    /// Effective KV context cap (sliding window or full context)
    pub kv_context_cap: Option<u64>,
    /// Model's training context length from GGUF header
    pub model_max_context: u64,
    pub arch: Option<ModelArchInfo>,
    pub files: Vec<FileRecommendation>,
    pub best: Option<BestRecommendation>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRecommendation {
    pub filename: String,
    pub size: u64,
    pub quantization: String,
    pub quant_quality: u32,
    pub max_context_f16: u64,
    pub max_context_q8_0: u64,
    pub max_context_q4_0: u64,
    /// Max context that keeps model+KV 100% in VRAM (Q8_0 KV). 0 if model doesn't fit.
    pub optimal_gpu_ctx: u64,
    /// Max context that fits in total RAM+VRAM before swapping (Q8_0 KV).
    pub optimal_ram_ctx: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BestRecommendation {
    pub filename: String,
    pub context_length: u64,
    pub kv_type: String,
    pub score: u32,
    pub viable: bool,
}

// Capped at q8_0: q8_0 is the highest-quality KV type we recommend. (Higher
// options like q8_1/f16/f32 remain manually selectable but are never auto-picked;
// q8_1 also has no matching UI dropdown option, which made the select fall back
// to displaying "F32".)
const RECOMMENDATION_KV_TYPES: &[(&str, f64)] = &[
    ("q8_0", 1.0),
    ("q5_1", 0.75),
    ("q5_0", 0.6875),
    ("iq4_nl", 0.5625),
    ("q4_0", 0.5),
];
const MIN_CONTEXT: u64 = 4096;

fn calculate_optimal_context(
    budget_bytes: u64,
    model_weight_bytes: u64,
    bytes_per_token: f64, // kv_base × bytes_per_value (accounts for GQA/MLA + KV quant)
    model_max_ctx: u64,
    active_ratio: f64,
) -> u64 {
    let overhead = compute_overhead(model_weight_bytes, active_ratio);
    let remaining = budget_bytes
        .saturating_sub(model_weight_bytes)
        .saturating_sub(overhead);

    if remaining == 0 || bytes_per_token <= 0.0 {
        return 0;
    }

    let max_possible = (remaining as f64 / bytes_per_token) as u64;
    let mut optimal = max_possible.min(model_max_ctx);

    // Snap to multiples of 1024 for cleaner UI
    if optimal > 1024 {
        optimal = (optimal / 1024) * 1024;
    }

    optimal
}

/// Dynamic safety reserve: 10% of total system memory, clamped to [512MB, 2GB].
fn dynamic_safety_reserve(total_system_memory: u64) -> u64 {
    let reserve = (total_system_memory as f64 * 0.10) as u64;
    reserve.clamp(512_000_000, 2_000_000_000)
}

fn max_context_for(
    model_size: u64,
    kv_base: f64,
    bpv: f64,
    total_available: u64,
    model_max_ctx: u64,
    active_ratio: f64,
) -> u64 {
    let safety = dynamic_safety_reserve(total_available);
    let overhead = compute_overhead(model_size, active_ratio);
    let available = total_available
        .saturating_sub(model_size)
        .saturating_sub(overhead)
        .saturating_sub(safety);
    let per_token = kv_base * bpv;
    if per_token <= 0.0 {
        return model_max_ctx;
    }
    let max = (available as f64 / per_token) as u64;
    max.min(model_max_ctx)
}

fn build_recommendation(
    files: &[RunabilityFileInput],
    model_id: &str,
    meta: Option<&GgufModelMeta>,
    available_ram: u64,
    available_vram: u64,
    supports_gpu_offload: bool,
    unified_override: Option<bool>,
    default_context_cap: u64,
) -> RecommendationData {
    let unified = unified_override.unwrap_or_else(crate::llama_cpp::is_unified_memory);
    let total_available = resolve_total_available(available_ram, available_vram, unified);
    let kv_base = meta.and_then(kv_base_per_token);
    let model_max_ctx = meta.and_then(|m| m.context_length).unwrap_or(8192);
    let active_ratio = meta.map(active_weight_ratio).unwrap_or(1.0);
    // SWA cap: for sliding window architectures, KV cache doesn't grow past window size
    let kv_ctx_cap = meta.and_then(|m| {
        let arch = m.architecture.as_deref().unwrap_or("llama").to_lowercase();
        if arch == "gemma2" || arch == "cohere" {
            m.sliding_window
        } else {
            None
        }
    });

    let repo_qat = is_qat_name(model_id);
    let mut file_recs: Vec<FileRecommendation> = Vec::new();
    let mut best: Option<BestRecommendation> = None;

    for file in files {
        if file.size == 0 {
            continue;
        }

        let load_size = effective_model_size(file.size, meta);
        let qq =
            quant_quality_with_qat(&file.quantization, repo_qat || is_qat_name(&file.filename));
        let (max_f16, max_q8, max_q4) = if let Some(base) = kv_base {
            (
                max_context_for(
                    load_size,
                    base,
                    2.0,
                    total_available,
                    model_max_ctx,
                    active_ratio,
                ),
                max_context_for(
                    load_size,
                    base,
                    1.0,
                    total_available,
                    model_max_ctx,
                    active_ratio,
                ),
                max_context_for(
                    load_size,
                    base,
                    0.5,
                    total_available,
                    model_max_ctx,
                    active_ratio,
                ),
            )
        } else {
            (model_max_ctx, model_max_ctx, model_max_ctx)
        };

        // Optimal context: back-solve for max ctx at Q8_0 KV
        // State A: model fits in VRAM → use VRAM budget
        // State B: model doesn't fit → gpu_ctx = 0
        // State C: total RAM limit → use total_available with safety reserve
        let vram_budget = (available_vram as f64 * 0.90) as u64;
        let safety = dynamic_safety_reserve(total_available);
        let bpv_q8 = 1.0; // Q8_0 KV
        let bytes_per_token_q8 = kv_base.map(|b| b * bpv_q8).unwrap_or(0.0);

        let optimal_gpu_ctx = if load_size <= vram_budget {
            calculate_optimal_context(
                vram_budget,
                load_size,
                bytes_per_token_q8,
                model_max_ctx,
                active_ratio,
            )
        } else {
            0
        };

        let optimal_ram_ctx = calculate_optimal_context(
            total_available.saturating_sub(safety),
            load_size,
            bytes_per_token_q8,
            model_max_ctx,
            active_ratio,
        );

        file_recs.push(FileRecommendation {
            filename: file.filename.clone(),
            size: file.size,
            quantization: file.quantization.clone(),
            quant_quality: qq as u32,
            max_context_f16: max_f16,
            max_context_q8_0: max_q8,
            max_context_q4_0: max_q4,
            optimal_gpu_ctx,
            optimal_ram_ctx,
        });

        if is_draft_file(&file.filename) {
            continue;
        }

        // Try each KV type, find the best scoring configuration for this file
        for &(kv_name, bpv) in RECOMMENDATION_KV_TYPES {
            let max_ctx = if let Some(base) = kv_base {
                max_context_for(
                    load_size,
                    base,
                    bpv,
                    total_available,
                    model_max_ctx,
                    active_ratio,
                )
            } else {
                model_max_ctx
            };

            if max_ctx < MIN_CONTEXT {
                continue;
            }

            let ctx = max_ctx.min(default_context_cap);
            let effective_ctx = kv_ctx_cap.map(|cap| ctx.min(cap)).unwrap_or(ctx);
            let kv_bytes = kv_base
                .map(|b| (b * bpv * effective_ctx as f64) as u64)
                .unwrap_or(0);

            let (score, ..) = score_configuration(
                load_size,
                qq,
                kv_bytes,
                available_ram,
                total_available,
                available_vram,
                active_ratio,
            );

            if best.as_ref().is_none_or(|b| score > b.score) {
                best = Some(BestRecommendation {
                    filename: file.filename.clone(),
                    context_length: ctx,
                    kv_type: kv_name.to_string(),
                    score,
                    viable: score >= 60,
                });
            }
        }
    }

    if let Some(ref mut b) = best {
        b.viable = b.score >= 60;
    }

    // Override: pick the largest quant that fits in VRAM and use its optimal GPU ctx
    if let Some(base) = kv_base {
        let vram_budget = (available_vram as f64 * 0.90) as u64;
        let mut gpu_candidate: Option<(&RunabilityFileInput, &FileRecommendation)> = None;
        for (file, rec) in files.iter().zip(file_recs.iter()) {
            let load_size = effective_model_size(file.size, meta);
            if file.size == 0 || load_size > vram_budget || is_draft_file(&file.filename) {
                continue;
            }
            let qq =
                quant_quality_with_qat(&file.quantization, repo_qat || is_qat_name(&file.filename));
            if gpu_candidate
                .as_ref()
                .is_none_or(|(_, prev_rec)| qq > prev_rec.quant_quality as f64)
            {
                gpu_candidate = Some((file, rec));
            }
        }
        if let Some((file, rec)) = gpu_candidate {
            if rec.optimal_gpu_ctx >= MIN_CONTEXT {
                let ctx = rec.optimal_gpu_ctx;
                let effective_ctx = kv_ctx_cap.map(|cap| ctx.min(cap)).unwrap_or(ctx);
                let kv_bytes = (base * 1.0 * effective_ctx as f64) as u64; // Q8_0
                let (score, ..) = score_configuration(
                    effective_model_size(file.size, meta),
                    quant_quality_with_qat(
                        &file.quantization,
                        repo_qat || is_qat_name(&file.filename),
                    ),
                    kv_bytes,
                    available_ram,
                    total_available,
                    available_vram,
                    active_ratio,
                );
                if score > best.as_ref().map_or(0, |b| b.score) {
                    best = Some(BestRecommendation {
                        filename: file.filename.clone(),
                        context_length: ctx,
                        kv_type: "q8_0".to_string(),
                        score,
                        viable: score >= 60,
                    });
                }
            }
        }
    }

    RecommendationData {
        available_ram,
        available_vram,
        supports_gpu_offload,
        unified_memory: unified,
        total_available,
        kv_base_per_token: kv_base,
        kv_context_cap: kv_ctx_cap,
        model_max_context: model_max_ctx,
        arch: meta.map(ModelArchInfo::from),
        files: file_recs,
        best,
    }
}

fn extract_quantization(filename: &str) -> String {
    let upper = filename.to_uppercase();
    let patterns = [
        "IQ1_S",
        "IQ1_M",
        "IQ2_XXS",
        "IQ2_XS",
        "IQ2_S",
        "IQ2_M",
        "IQ3_XXS",
        "IQ3_XS",
        "IQ3_S",
        "IQ3_M",
        "IQ4_XS",
        "IQ4_NL",
        "Q2_K_S",
        "Q2_K_M",
        "Q2_K_L",
        "Q2_K_XL",
        "Q2_K",
        "Q3_K_S",
        "Q3_K_M",
        "Q3_K_L",
        "Q3_K_XL",
        "Q3_K",
        "Q4_K_S",
        "Q4_K_M",
        "Q4_K_L",
        "Q4_K_XL",
        "Q4_K",
        "Q4_0",
        "Q4_1",
        "Q5_K_S",
        "Q5_K_M",
        "Q5_K_L",
        "Q5_K_XL",
        "Q5_K",
        "Q5_0",
        "Q5_1",
        "Q6_K_S",
        "Q6_K_L",
        "Q6_K_XL",
        "Q6_K",
        "Q8_K_S",
        "Q8_K_L",
        "Q8_K_XL",
        "Q8_K",
        "Q8_0",
        "MXFP4_MOE",
        "F16",
        "F32",
        "BF16",
    ];
    for p in &patterns {
        if upper.contains(p) {
            if upper.contains(&format!("UD-{p}")) {
                return format!("UD-{p}");
            }
            return p.to_string();
        }
    }
    "Unknown".to_string()
}

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("LettuceAI/1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|error| format!("Failed to build HTTP client: {error}"))
}

fn build_hf_client(app: &AppHandle) -> Result<reqwest::Client, String> {
    image_bundle::authenticated_client(app)
}

#[tauri::command]
pub async fn hf_search_models(
    app: AppHandle,
    query: String,
    limit: Option<u32>,
    sort: Option<String>,
    offset: Option<u32>,
    author: Option<String>,
    mode: Option<String>,
    profile_id: Option<String>,
    bundle_role: Option<String>,
    unfiltered: Option<bool>,
) -> Result<Vec<HfSearchResult>, String> {
    let limit = limit.unwrap_or(20).min(100);
    let sort_field = sort.unwrap_or_else(|| "trendingScore".to_string());
    let offset = offset.unwrap_or(0);
    let unfiltered = unfiltered.unwrap_or(false);

    let mode = mode.as_deref().unwrap_or("llm");
    if mode == "imageBundle" {
        if let (Some(profile_id), Some(role)) = (profile_id.as_deref(), bundle_role.as_deref()) {
            if offset > 0 {
                return Ok(Vec::new());
            }
            return image_bundle::bundle_role_search(
                &app,
                profile_id,
                role,
                query.trim(),
                &sort_field,
                author.as_deref(),
            )
            .await;
        }
    }
    if !unfiltered && matches!(mode, "image" | "imageBundle") {
        let mut merged = HashMap::<String, HfSearchResult>::new();
        for task in ["text-to-image", "image-to-image"] {
            let mut url = format!(
                "https://huggingface.co/api/models?pipeline_tag={task}&limit={}&sort={}&direction=-1&offset={}",
                limit, sort_field, offset
            );
            if let Some(author) = author.as_deref().map(str::trim).filter(|author| !author.is_empty()) {
                url.push_str(&format!("&author={}", urlencoding::encode(author)));
            }
            if !query.trim().is_empty() {
                url.push_str(&format!("&search={}", urlencoding::encode(query.trim())));
            }
            for result in fetch_models_list(&app, &url).await? {
                merged.entry(result.model_id.clone()).or_insert(result);
            }
        }
        if !query.trim().is_empty() {
            let mut url = format!(
                "https://huggingface.co/api/models?limit={}&sort={}&direction=-1&offset={}",
                limit, sort_field, offset
            );
            if let Some(author) = author.as_deref().map(str::trim).filter(|author| !author.is_empty()) {
                url.push_str(&format!("&author={}", urlencoding::encode(author)));
            }
            url.push_str(&format!("&search={}", urlencoding::encode(query.trim())));
            for result in fetch_models_list(&app, &url).await? {
                merged.entry(result.model_id.clone()).or_insert(result);
            }
        }
        let mut results = merged.into_values().collect::<Vec<_>>();
        match sort_field.as_str() {
            "likes" => results.sort_by_key(|result| std::cmp::Reverse(result.likes)),
            "lastModified" => results.sort_by(|a, b| b.last_modified.cmp(&a.last_modified)),
            "trendingScore" => results.sort_by(|a, b| b.trending_score.partial_cmp(&a.trending_score).unwrap_or(std::cmp::Ordering::Equal)),
            _ => results.sort_by_key(|result| std::cmp::Reverse(result.downloads)),
        }
        results.truncate(limit as usize);
        return Ok(results);
    }

    let mut url = format!(
        "https://huggingface.co/api/models?{}limit={}&sort={}&offset={}",
        if unfiltered { "" } else { "filter=gguf&" },
        limit,
        sort_field,
        offset
    );
    if let Some(author) = author.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
        url.push_str(&format!("&author={}", urlencoding::encode(author)));
    }
    let trimmed = query.trim();
    if !trimmed.is_empty() {
        url.push_str(&format!("&search={}", urlencoding::encode(trimmed)));
    }

    log_info(&app, "hf_browser", format!("searching: {}", url));
    let mut results = fetch_models_list(&app, &url).await?;
    if !unfiltered && trimmed.contains('/') && !trimmed.contains(char::is_whitespace) {
        let mut generic_url = format!(
            "https://huggingface.co/api/models?limit={}&sort={}&offset={}",
            limit, sort_field, offset
        );
        if let Some(author) = author.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
            generic_url.push_str(&format!("&author={}", urlencoding::encode(author)));
        }
        generic_url.push_str(&format!("&search={}", urlencoding::encode(trimmed)));
        for result in fetch_models_list(&app, &generic_url).await? {
            if !results.iter().any(|existing| existing.model_id == result.model_id) {
                results.push(result);
            }
        }
    }
    log_info(
        &app,
        "hf_browser",
        format!("search returned {} results", results.len()),
    );

    Ok(results)
}

pub(crate) async fn fetch_models_list(app: &AppHandle, url: &str) -> Result<Vec<HfSearchResult>, String> {
    let client = build_hf_client(app)?;
    let response = client.get(url).send().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("HuggingFace API request failed: {}", e),
        )
    })?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(if image_bundle::hf_token(app).is_some() {
            "The saved Hugging Face token is invalid or expired.".to_string()
        } else {
            "This Hugging Face file requires an access token.".to_string()
        });
    }
    if response.status() == reqwest::StatusCode::FORBIDDEN {
        return Err("Accept the gated model license on Hugging Face, then retry.".to_string());
    }
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("HuggingFace API error {}: {}", status, body),
        ));
    }

    let entries: Vec<HfModelEntry> = response.json().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to parse HuggingFace response: {}", e),
        )
    })?;

    let results: Vec<HfSearchResult> = entries
        .into_iter()
        .map(|e| {
            let author = e.author.unwrap_or_else(|| {
                e.model_id
                    .split('/')
                    .next()
                    .unwrap_or("unknown")
                    .to_string()
            });
            HfSearchResult {
                model_id: e.model_id,
                author,
                likes: e.likes,
                downloads: e.downloads,
                tags: e.tags,
                pipeline_tag: e.pipeline_tag,
                last_modified: e.last_modified,
                trending_score: e.trending_score,
            }
        })
        .collect();

    Ok(results)
}

#[tauri::command]
pub async fn hf_get_author_models(
    app: AppHandle,
    author: String,
    search: Option<String>,
    limit: Option<u32>,
    sort: Option<String>,
    offset: Option<u32>,
) -> Result<Vec<HfSearchResult>, String> {
    let author = author.trim();
    if author.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "author is empty",
        ));
    }
    let limit = limit.unwrap_or(50).min(100);
    let sort_field = sort.unwrap_or_else(|| "downloads".to_string());
    let offset = offset.unwrap_or(0);
    let mut url = format!(
        "https://huggingface.co/api/models?author={}&filter=gguf&limit={}&sort={}&direction=-1&offset={}",
        urlencoding::encode(author),
        limit,
        sort_field,
        offset
    );
    if let Some(search) = search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        url.push_str(&format!("&search={}", urlencoding::encode(search)));
    }

    log_info(&app, "hf_browser", format!("author models: {}", url));
    let results = fetch_models_list(&app, &url).await?;
    log_info(
        &app,
        "hf_browser",
        format!("author {} returned {} models", author, results.len()),
    );

    Ok(results)
}

#[tauri::command]
pub async fn hf_get_author_overview(
    app: AppHandle,
    author: String,
) -> Result<HfAuthorOverview, String> {
    let author = author.trim().to_string();
    if author.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "author is empty",
        ));
    }
    log_info(&app, "hf_browser", format!("author overview: {}", author));
    let encoded = urlencoding::encode(&author);
    let client = build_hf_client(&app)?;
    // An author may be a user or an org; try user overview first, then org.
    let urls = [
        format!("https://huggingface.co/api/users/{}/overview", encoded),
        format!(
            "https://huggingface.co/api/organizations/{}/overview",
            encoded
        ),
    ];

    let mut last_err = String::new();
    for url in urls {
        let response = match client.get(&url).send().await {
            Ok(resp) => resp,
            Err(e) => {
                last_err = e.to_string();
                continue;
            }
        };
        if !response.status().is_success() {
            last_err = format!("{} -> {}", url, response.status());
            continue;
        }
        let raw: HfOverviewResponse = match response.json().await {
            Ok(value) => value,
            Err(e) => {
                last_err = e.to_string();
                continue;
            }
        };
        return Ok(HfAuthorOverview {
            name: raw.name.unwrap_or_else(|| author.clone()),
            fullname: raw.fullname,
            avatar_url: raw.avatar_url,
            details: raw.details,
            kind: raw.kind,
            is_pro: raw.is_pro.unwrap_or(false),
            num_models: raw.num_models.unwrap_or(0),
            num_datasets: raw.num_datasets.unwrap_or(0),
            num_spaces: raw.num_spaces.unwrap_or(0),
            num_likes: raw.num_likes.unwrap_or(0),
            num_followers: raw.num_followers.unwrap_or(0),
            num_following: raw.num_following.unwrap_or(0),
            created_at: raw.created_at,
        });
    }

    Err(crate::utils::err_msg(
        module_path!(),
        line!(),
        format!(
            "Failed to fetch author overview for {}: {}",
            author, last_err
        ),
    ))
}

#[tauri::command]
pub async fn hf_get_model_files(
    app: AppHandle,
    model_id: String,
    mode: Option<String>,
) -> Result<HfModelInfo, String> {
    log_info(
        &app,
        "hf_browser",
        format!("fetching model info: {}", model_id),
    );

    let client = build_hf_client(&app)?;

    let detail_url = format!("https://huggingface.co/api/models/{}", model_id);
    let detail_resp = client.get(&detail_url).send().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to fetch model detail: {}", e),
        )
    })?;

    if detail_resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(if image_bundle::hf_token(&app).is_some() {
            "The saved Hugging Face token is invalid or expired.".to_string()
        } else {
            "This Hugging Face model requires an access token.".to_string()
        });
    }
    if detail_resp.status() == reqwest::StatusCode::FORBIDDEN {
        return Err(format!("Accept access to {model_id} on Hugging Face, then retry."));
    }
    if !detail_resp.status().is_success() {
        let status = detail_resp.status();
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Model not found ({}): {}", status, model_id),
        ));
    }

    let detail: HfModelDetail = detail_resp.json().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to parse model detail: {}", e),
        )
    })?;

    let is_image = matches!(mode.as_deref(), Some("image") | Some("imageBundle"));
    let tree_url = format!(
        "https://huggingface.co/api/models/{}/tree/main?recursive={}",
        model_id,
        if is_image { "true" } else { "false" }
    );
    let tree_resp = client.get(&tree_url).send().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to fetch file tree: {}", e),
        )
    })?;

    if tree_resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(if image_bundle::hf_token(&app).is_some() {
            "The saved Hugging Face token is invalid or expired.".to_string()
        } else {
            "This Hugging Face repository requires an access token.".to_string()
        });
    }
    if tree_resp.status() == reqwest::StatusCode::FORBIDDEN {
        return Err(format!("Accept access to {model_id} on Hugging Face, then retry."));
    }
    let tree_entries: Vec<HfTreeEntry> = if tree_resp.status().is_success() {
        tree_resp.json().await.unwrap_or_default()
    } else {
        vec![]
    };

    let size_map: std::collections::HashMap<String, u64> = tree_entries
        .into_iter()
        .filter(|e| e.entry_type == "file")
        .map(|e| {
            let size = e.lfs.as_ref().map(|l| l.size).unwrap_or(e.size);
            (e.path, size)
        })
        .collect();

    let mut files: Vec<HfModelFile> = detail
        .siblings
        .iter()
        .filter(|s| {
            let lower = s.rfilename.to_lowercase();
            if lower.contains("imatrix") {
                return false;
            }
            lower.ends_with(".gguf")
                || (is_image && (lower.ends_with(".safetensors") || lower.ends_with(".sft")))
        })
        .map(|s| {
            let lower = s.rfilename.to_lowercase();
            let size = size_map.get(&s.rfilename).copied().unwrap_or(0);
            let quantization = extract_quantization(&s.rfilename);
            let role = None;
            HfModelFile {
                filename: s.rfilename.clone(),
                size,
                quantization,
                is_mmproj: lower.contains("mmproj"),
                is_mtp: is_mtp_asset(&s.rfilename),
                role,
            }
        })
        .collect();

    files.sort_by_key(|f| f.size);

    let author = detail
        .author
        .unwrap_or_else(|| model_id.split('/').next().unwrap_or("unknown").to_string());

    let architecture = detail.gguf.as_ref().and_then(|g| g.architecture.clone());
    let context_length = detail.gguf.as_ref().and_then(|g| g.context_length);
    let parameter_count = detail.gguf.as_ref().and_then(|g| g.total);

    log_info(
        &app,
        "hf_browser",
        format!(
            "model {} has {} GGUF files, arch={:?}",
            model_id,
            files.len(),
            architecture
        ),
    );

    Ok(HfModelInfo {
        model_id: detail.model_id,
        author,
        likes: detail.likes,
        downloads: detail.downloads,
        tags: detail.tags,
        architecture,
        context_length,
        parameter_count,
        files,
    })
}

#[tauri::command]
pub async fn hf_queue_download(
    app: AppHandle,
    model_id: String,
    filename: String,
    metadata: Option<QueueDownloadMetadata>,
) -> Result<String, String> {
    let queue_id = uuid::Uuid::new_v4().to_string();
    let metadata = metadata.unwrap_or_default();

    {
        let mut state = HF_DOWNLOAD_QUEUE.lock().await;
        state.queue.push(QueuedDownload {
            id: queue_id.clone(),
            model_id: model_id.clone(),
            filename: filename.clone(),
            status: "queued".to_string(),
            phase: None,
            downloaded: 0,
            total: metadata.expected_size.unwrap_or(0),
            speed_bytes_per_sec: 0,
            error: None,
            result_path: None,
            create_model_when_finished: metadata.create_model_when_finished,
            mmproj_file: metadata.mmproj_file,
            mtp_file: metadata.mtp_file,
            mtp_bundled: metadata.mtp_bundled,
            install_id: metadata.install_id,
            display_name: metadata.display_name,
            context_length: metadata.context_length,
            kv_type: metadata.kv_type,
            llama_offload_kqv: metadata.llama_offload_kqv,
            llama_gpu_layers: metadata.llama_gpu_layers,
            llama_model_offload_mode: metadata.llama_model_offload_mode,
            download_role: metadata.download_role,
            queue_kind: metadata.queue_kind,
            asset_root: metadata.asset_root,
            install_kind: metadata.install_kind,
            variant: metadata.variant,
            voice_id: metadata.voice_id,
            download_url: metadata.download_url,
            destination_path: metadata.destination_path,
            expected_size: metadata.expected_size,
            sha256: metadata.sha256,
            runtime_release: metadata.runtime_release,
            runtime_asset: metadata.runtime_asset,
            force_redownload: metadata.force_redownload,
            lora_keywords: metadata.lora_keywords,
            lora_base_model: metadata.lora_base_model,
        });
        emit_queue(&app, &state.queue);
    }

    log_info(
        &app,
        "hf_browser",
        format!(
            "queued download: {}/{} (id={})",
            model_id, filename, queue_id
        ),
    );

    let app_clone = app.clone();
    {
        let mut state = HF_DOWNLOAD_QUEUE.lock().await;
        if !state.processing {
            state.processing = true;
            tokio::spawn(async move {
                process_download_queue(&app_clone).await;
            });
        }
    }

    Ok(queue_id)
}

#[tauri::command]
pub async fn hf_cancel_queue_item(app: AppHandle, queue_id: String) -> Result<(), String> {
    log_info(
        &app,
        "hf_browser",
        format!("cancel requested for: {}", queue_id),
    );
    let mut state = HF_DOWNLOAD_QUEUE.lock().await;

    if let Some(d) = state.queue.iter_mut().find(|d| d.id == queue_id) {
        if d.status == "queued" {
            d.status = "cancelled".to_string();
            emit_queue(&app, &state.queue);
        } else if d.status == "downloading" {
            state.cancel_ids.insert(queue_id);
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn hf_dismiss_queue_item(app: AppHandle, queue_id: String) -> Result<(), String> {
    let mut state = HF_DOWNLOAD_QUEUE.lock().await;
    state.queue.retain(|d| d.id != queue_id);
    emit_queue(&app, &state.queue);
    Ok(())
}

#[tauri::command]
pub async fn hf_get_download_queue() -> Result<Vec<QueuedDownload>, String> {
    let state = HF_DOWNLOAD_QUEUE.lock().await;
    Ok(state.queue.clone())
}

async fn process_download_queue(app: &AppHandle) {
    loop {
        let next_item = {
            let state = HF_DOWNLOAD_QUEUE.lock().await;
            state.queue.iter().find(|d| d.status == "queued").cloned()
        };

        let item = match next_item {
            Some(item) => item,
            None => {
                let mut state = HF_DOWNLOAD_QUEUE.lock().await;
                state.processing = false;
                return;
            }
        };

        {
            let mut state = HF_DOWNLOAD_QUEUE.lock().await;
            if let Some(d) = state.queue.iter_mut().find(|d| d.id == item.id) {
                d.status = "downloading".to_string();
                d.phase = None;
            }
            emit_queue(app, &state.queue);
        }

        let result = match do_queue_download(app, &item).await {
            Ok(path) => match crate::image_generator::sdcpp::handle_download_completed(app, &item, &path).await {
                Ok(()) => {
                    image_bundle::handle_download_completed(app, &item, &path).await;
                    match crate::image_generator::sdcpp::handle_lora_download_completed(app, &item, &path) {
                        Ok(()) => Ok(path),
                        Err(error) => Err(error),
                    }
                }
                Err(error) => Err(error),
            },
            Err(error) => Err(error),
        };

        {
            let mut state = HF_DOWNLOAD_QUEUE.lock().await;
            state.cancel_ids.remove(&item.id);
            if let Some(d) = state.queue.iter_mut().find(|d| d.id == item.id) {
                match result {
                    Ok(path) => {
                        d.status = "complete".to_string();
                        d.phase = None;
                        d.downloaded = d.total;
                        d.speed_bytes_per_sec = 0;
                        d.result_path = Some(path);
                    }
                    Err(ref e) if e.contains("cancelled") => {
                        d.status = "cancelled".to_string();
                        d.phase = None;
                        d.speed_bytes_per_sec = 0;
                    }
                    Err(ref e) => {
                        d.status = "error".to_string();
                        d.phase = None;
                        d.speed_bytes_per_sec = 0;
                        d.error = Some(e.clone());
                    }
                }
            }
            emit_queue(app, &state.queue);
        }
    }
}

async fn do_queue_download(app: &AppHandle, item: &QueuedDownload) -> Result<String, String> {
    let queue_id = item.id.as_str();
    let model_id = item.model_id.as_str();
    let filename = item.filename.as_str();
    let dest_path = if let Some(destination_path) = item.destination_path.as_deref() {
        PathBuf::from(destination_path)
    } else if item.queue_kind.as_deref() == Some("hf_image_manual") {
        let relative = Path::new(filename);
        if relative.is_absolute()
            || !relative
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_)))
        {
            return Err("The selected image component has an unsafe repository path.".to_string());
        }
        crate::utils::lettuce_dir(app)?
            .join("models")
            .join("image")
            .join("huggingface")
            .join("manual")
            .join(model_id.replace('/', "--"))
            .join(relative)
    } else {
        let models_dir = hf_models_dir(app)?;
        let safe_model_name = model_id.replace('/', "--");
        let model_dir = models_dir.join(&safe_model_name);
        if !model_dir.exists() {
            tokio::fs::create_dir_all(&model_dir).await.map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to create model directory: {}", e),
                )
            })?;
        }
        model_dir.join(filename)
    };

    if let Some(parent) = dest_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to create destination directory: {}", e),
            )
        })?;
    }

    if dest_path.exists() && !item.force_redownload {
        let meta = tokio::fs::metadata(&dest_path).await.ok();
        if let Some(m) = meta {
            let min_existing_size = if item.queue_kind.as_deref() == Some("kokoro") {
                1
            } else {
                1_000_000
            };
            let size_matches = item.expected_size.map_or(true, |size| m.len() == size);
            let hash_matches = if size_matches {
                if item.sha256.is_some() {
                    let mut state = HF_DOWNLOAD_QUEUE.lock().await;
                    if let Some(d) = state.queue.iter_mut().find(|d| d.id == queue_id) {
                        d.phase = Some("verifying".to_string());
                        d.total = m.len();
                        d.downloaded = m.len();
                        d.speed_bytes_per_sec = 0;
                    }
                    emit_queue(app, &state.queue);
                }
                verify_sha256(&dest_path, item.sha256.as_deref()).await?
            } else {
                false
            };
            if m.len() >= min_existing_size && size_matches && hash_matches {
                log_info(
                    app,
                    "hf_browser",
                    format!(
                        "File already exists ({} bytes), skipping download: {}",
                        m.len(),
                        dest_path.display()
                    ),
                );
                {
                    let mut state = HF_DOWNLOAD_QUEUE.lock().await;
                    if let Some(d) = state.queue.iter_mut().find(|d| d.id == queue_id) {
                        d.total = m.len();
                        d.downloaded = m.len();
                    }
                }
                return Ok(dest_path.to_string_lossy().to_string());
            }
            log_info(
                app,
                "hf_browser",
                format!(
                    "Existing file failed integrity validation; downloading again: {}",
                    dest_path.display()
                ),
            );
        }
    }

    let download_url = item.download_url.clone().unwrap_or_else(|| {
        format!(
            "https://huggingface.co/{}/resolve/main/{}",
            model_id, filename
        )
    });

    log_info(
        app,
        "hf_browser",
        format!("starting verified download: {}/{}", model_id, filename),
    );

    let client = reqwest::Client::builder()
        .user_agent("LettuceAI/1.0")
        .redirect(reqwest::redirect::Policy::limited(10))
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|error| format!("Failed to build download client: {error}"))?;
    let mut request = client.get(&download_url);
    let is_hugging_face_download = reqwest::Url::parse(&download_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .is_some_and(|host| host == "huggingface.co" || host.ends_with(".huggingface.co"));
    let is_civitai_download = reqwest::Url::parse(&download_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .is_some_and(|host| host == "civitai.com" || host.ends_with(".civitai.com"));
    if is_hugging_face_download {
        if let Some(token) = image_bundle::hf_token(app) {
            request = request.bearer_auth(token);
        }
    }
    if is_civitai_download {
        if let Some(token) = crate::civitai::civitai_token(app) {
            request = request.bearer_auth(token);
        }
    }
    let response = request.send().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to start download: {}", e),
        )
    })?;

    if is_hugging_face_download && response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(if image_bundle::hf_token(app).is_some() {
            "The saved Hugging Face token is invalid or expired.".to_string()
        } else {
            "This Hugging Face file requires an access token.".to_string()
        });
    }
    if is_hugging_face_download && response.status() == reqwest::StatusCode::FORBIDDEN {
        return Err(format!(
            "Accept access to {model_id} on its Hugging Face model card, then retry."
        ));
    }
    if is_civitai_download && response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(if crate::civitai::civitai_token(app).is_some() {
            "The saved CivitAI token is invalid or expired.".to_string()
        } else {
            "This CivitAI file requires an API token. Add one in Runtime Defaults.".to_string()
        });
    }
    if is_civitai_download && response.status() == reqwest::StatusCode::FORBIDDEN {
        return Err(
            "This CivitAI file is restricted or in early access for your account.".to_string(),
        );
    }
    if !response.status().is_success() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Download failed with status: {}", response.status()),
        ));
    }

    let reported_total_size = response.content_length();
    if let (Some(expected), Some(reported)) = (item.expected_size, reported_total_size) {
        if reported != expected {
            return Err(format!(
                "Download size mismatch before transfer: expected {} bytes, server reported {} bytes",
                expected, reported
            ));
        }
    }
    let total_size = reported_total_size.or(item.expected_size).unwrap_or(0);

    {
        let mut state = HF_DOWNLOAD_QUEUE.lock().await;
        if let Some(d) = state.queue.iter_mut().find(|d| d.id == queue_id) {
            d.total = total_size;
            d.downloaded = 0;
            d.speed_bytes_per_sec = 0;
            d.phase = None;
        }
        state.last_speed_sample = std::time::Instant::now();
        state.speed_bytes_window = 0;
        emit_queue(app, &state.queue);
    }

    let temp_path = dest_path.with_extension("tmp");
    let mut file = tokio::fs::File::create(&temp_path).await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to create temp file: {}", e),
        )
    })?;

    let mut stream = response.bytes_stream();
    let mut last_emit = std::time::Instant::now();

    while let Some(chunk_result) =
        match tokio::time::timeout(std::time::Duration::from_secs(120), stream.next()).await {
            Ok(next) => next,
            Err(_) => {
                drop(file);
                let _ = tokio::fs::remove_file(&temp_path).await;
                return Err("The download stalled and timed out.".to_string());
            }
        }
    {
        {
            let state = HF_DOWNLOAD_QUEUE.lock().await;
            if state.cancel_ids.contains(queue_id) {
                drop(file);
                let _ = tokio::fs::remove_file(&temp_path).await;
                return Err("Download cancelled".to_string());
            }
        }

        let chunk = chunk_result.map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Error reading download chunk: {}", e),
            )
        })?;

        file.write_all(&chunk).await.map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Error writing to file: {}", e),
            )
        })?;

        let chunk_len = chunk.len() as u64;

        {
            let mut state = HF_DOWNLOAD_QUEUE.lock().await;
            if let Some(d) = state.queue.iter_mut().find(|d| d.id == queue_id) {
                d.downloaded += chunk_len;
            }
            state.speed_bytes_window += chunk_len;

            let elapsed = state.last_speed_sample.elapsed();
            if elapsed.as_millis() >= 1000 {
                let secs = elapsed.as_secs_f64();
                if secs > 0.0 {
                    let speed = (state.speed_bytes_window as f64 / secs) as u64;
                    if let Some(d) = state.queue.iter_mut().find(|d| d.id == queue_id) {
                        d.speed_bytes_per_sec = speed;
                    }
                }
                state.speed_bytes_window = 0;
                state.last_speed_sample = std::time::Instant::now();
            }

            if last_emit.elapsed().as_millis() > 150 {
                emit_queue(app, &state.queue);
                last_emit = std::time::Instant::now();
            }
        }
    }

    file.flush().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Error flushing file: {}", e),
        )
    })?;
    drop(file);

    let downloaded_size = tokio::fs::metadata(&temp_path)
        .await
        .map_err(|e| format!("Failed to inspect downloaded file: {}", e))?
        .len();
    if let Some(expected) = item.expected_size {
        if downloaded_size != expected {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(format!(
                "Downloaded file has the wrong size: expected {} bytes, received {} bytes",
                expected, downloaded_size
            ));
        }
    }
    if item.sha256.is_some() {
        let mut state = HF_DOWNLOAD_QUEUE.lock().await;
        if let Some(d) = state.queue.iter_mut().find(|d| d.id == queue_id) {
            d.phase = Some("verifying".to_string());
            d.downloaded = downloaded_size;
            if d.total == 0 {
                d.total = downloaded_size;
            }
            d.speed_bytes_per_sec = 0;
        }
        emit_queue(app, &state.queue);
    }
    if !verify_sha256(&temp_path, item.sha256.as_deref()).await? {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err("Downloaded file failed SHA-256 validation".to_string());
    }
    {
        let state = HF_DOWNLOAD_QUEUE.lock().await;
        if state.cancel_ids.contains(queue_id) {
            drop(state);
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err("Download cancelled".to_string());
        }
    }

    tokio::fs::rename(&temp_path, &dest_path)
        .await
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to rename temp file: {}", e),
            )
        })?;

    let final_path = dest_path.to_string_lossy().to_string();

    log_info(
        app,
        "hf_browser",
        format!("download complete: {} ({} bytes)", final_path, total_size),
    );

    Ok(final_path)
}

async fn verify_sha256(path: &Path, expected: Option<&str>) -> Result<bool, String> {
    let Some(expected) = expected else {
        return Ok(true);
    };
    let expected = expected.trim().to_ascii_lowercase();
    if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Invalid expected SHA-256 digest".to_string());
    }

    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut file = std::fs::File::open(path)
            .map_err(|e| format!("Failed to open file for integrity validation: {}", e))?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0_u8; 1024 * 1024];
        loop {
            let read = std::io::Read::read(&mut file, &mut buffer)
                .map_err(|e| format!("Failed to validate downloaded file: {}", e))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        let actual = format!("{:x}", hasher.finalize());
        Ok(actual == expected)
    })
    .await
    .map_err(|e| format!("File integrity validation task failed: {}", e))?
}

#[tauri::command]
pub async fn hf_list_downloaded_models(app: AppHandle) -> Result<Vec<DownloadedGgufModel>, String> {
    let models_dir = hf_models_dir(&app)?;

    let mut results = Vec::new();

    let entries = std::fs::read_dir(&models_dir).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to read models dir: {}", e),
        )
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let dir_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        if let Ok(files) = std::fs::read_dir(&path) {
            for file_entry in files.flatten() {
                let file_path = file_entry.path();
                let fname = file_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                if fname.to_lowercase().ends_with(".gguf") && !fname.ends_with(".tmp") {
                    let is_mmproj = fname.to_lowercase().contains("mmproj");
                    let is_mtp = is_mtp_asset(&fname);
                    let size = file_entry.metadata().map(|m| m.len()).unwrap_or(0);
                    let meta = read_local_gguf_meta(&file_path);
                    results.push(DownloadedGgufModel {
                        model_id: dir_name.replace("--", "/"),
                        filename: fname,
                        path: file_path.to_string_lossy().to_string(),
                        size,
                        quantization: extract_quantization(&file_path.to_string_lossy()),
                        is_mmproj,
                        is_mtp,
                        architecture: meta.as_ref().and_then(|value| value.architecture.clone()),
                        context_length: meta.as_ref().and_then(|value| value.context_length),
                        image_role: None,
                    });
                }
            }
        }
    }

    Ok(results)
}

fn frontend_image_role(role: &str) -> &'static str {
    match role {
        "text_encoder" => "llm",
        "vision_encoder" => "llmVision",
        "vae" => "vae",
        _ => "diffusionModel",
    }
}

fn infer_image_role(filename: &str) -> &'static str {
    if filename.contains("vae") || filename.starts_with("ae.") {
        "vae"
    } else if filename.contains("mmproj") || filename.contains("vision") {
        "llmVision"
    } else if ["qwen3", "qwen2.5", "qwen2_5", "t5", "umt5", "clip"]
        .iter()
        .any(|marker| filename.contains(marker))
    {
        "llm"
    } else {
        "diffusionModel"
    }
}

#[tauri::command]
pub async fn hf_list_downloaded_image_models(
    app: AppHandle,
) -> Result<Vec<DownloadedGgufModel>, String> {
    let image_root = crate::utils::lettuce_dir(&app)?.join("models").join("image");
    let mut results = Vec::new();
    if !image_root.exists() {
        return Ok(results);
    }
    let mut role_by_path: HashMap<String, String> = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(image_root.join("huggingface").join("bundles")) {
        for entry in entries.flatten() {
            let Ok(bytes) = std::fs::read(entry.path()) else { continue };
            let Ok(manifest) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                continue;
            };
            for asset in manifest
                .get("assets")
                .and_then(|assets| assets.as_array())
                .into_iter()
                .flatten()
            {
                if let (Some(local_path), Some(role)) = (
                    asset.get("localPath").and_then(|value| value.as_str()),
                    asset.get("role").and_then(|value| value.as_str()),
                ) {
                    role_by_path
                        .insert(local_path.to_string(), frontend_image_role(role).to_string());
                }
            }
        }
    }
    let mut stack = vec![image_root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let fname = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let lower = fname.to_lowercase();
            if !(lower.ends_with(".gguf")
                || lower.ends_with(".safetensors")
                || lower.ends_with(".sft"))
            {
                continue;
            }
            let segments = path
                .strip_prefix(&image_root)
                .unwrap_or(&path)
                .components()
                .filter_map(|component| match component {
                    std::path::Component::Normal(part) => {
                        Some(part.to_string_lossy().to_string())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let model_id = if segments.first().map(String::as_str) == Some("huggingface") {
                if segments.get(1).map(String::as_str) == Some("manual") {
                    segments
                        .get(2)
                        .map(|part| part.replace("--", "/"))
                        .unwrap_or_else(|| "huggingface".to_string())
                } else if segments.len() >= 3 {
                    format!("{}/{}", segments[1], segments[2])
                } else {
                    "huggingface".to_string()
                }
            } else {
                segments.first().cloned().unwrap_or_default()
            };
            let meta = if lower.ends_with(".gguf") {
                read_local_gguf_meta(&path)
            } else {
                None
            };
            let path_string = path.to_string_lossy().to_string();
            let image_role = role_by_path
                .get(&path_string)
                .cloned()
                .or_else(|| {
                    if segments.first().map(String::as_str) == Some("components") {
                        segments
                            .get(1)
                            .and_then(|sha| {
                                crate::image_generator::sdcpp::catalog_component_role(sha)
                            })
                            .map(|role| frontend_image_role(role).to_string())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| infer_image_role(&lower).to_string());
            results.push(DownloadedGgufModel {
                model_id,
                filename: fname,
                size: entry.metadata().map(|metadata| metadata.len()).unwrap_or(0),
                quantization: extract_quantization(&path_string),
                is_mmproj: lower.contains("mmproj"),
                is_mtp: false,
                architecture: meta.as_ref().and_then(|value| value.architecture.clone()),
                context_length: None,
                image_role: Some(image_role),
                path: path_string,
            });
        }
    }
    results.sort_by(|left, right| {
        left.model_id
            .cmp(&right.model_id)
            .then(left.filename.cmp(&right.filename))
    });
    Ok(results)
}

#[tauri::command]
pub async fn hf_delete_downloaded_model(app: AppHandle, file_path: String) -> Result<(), String> {
    let path = PathBuf::from(&file_path);

    if !path.exists() {
        return Ok(());
    }

    let models_dir = hf_models_dir(&app)?;
    let image_root = crate::utils::lettuce_dir(&app)?.join("models").join("image");
    if !path.starts_with(&models_dir) && !path.starts_with(&image_root) {
        return Err("Cannot delete files outside the models directory".to_string());
    }

    tokio::fs::remove_file(&path).await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to delete model file: {}", e),
        )
    })?;

    if let Some(parent) = path.parent() {
        if parent != models_dir && parent != image_root {
            let _ = tokio::fs::remove_dir(parent).await;
        }
    }

    log_info(
        &app,
        "hf_browser",
        format!("deleted model file: {}", file_path),
    );

    Ok(())
}

#[tauri::command]
pub async fn hf_get_gguf_models_dir(app: AppHandle) -> Result<String, String> {
    let dir = hf_models_dir(&app)?;
    Ok(dir.to_string_lossy().to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmModelsDirInfo {
    path: String,
    default_path: String,
    is_custom: bool,
    model_count: u32,
}

#[tauri::command]
pub async fn hf_get_llm_models_dir(app: AppHandle) -> Result<LlmModelsDirInfo, String> {
    let default_path = default_hf_models_dir(&app)?;
    let is_custom = custom_llm_models_dir(&app).is_some();
    let path = hf_models_dir(&app)?;
    let model_count = crate::infra::model_storage::count_models_in_dir(&path);
    Ok(LlmModelsDirInfo {
        path: path.to_string_lossy().to_string(),
        default_path: default_path.to_string_lossy().to_string(),
        is_custom,
        model_count,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetLlmModelsDirResult {
    path: String,
    moved_entries: u32,
    rewired_models: u32,
}

#[tauri::command]
pub async fn hf_set_llm_models_dir(
    app: AppHandle,
    new_dir: String,
    move_existing: bool,
) -> Result<SetLlmModelsDirResult, String> {
    let new_path = PathBuf::from(new_dir.trim());
    if new_path.as_os_str().is_empty() {
        return Err("New models folder path is empty".to_string());
    }

    let old_path = hf_models_dir(&app)?;

    std::fs::create_dir_all(&new_path).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to create models folder: {}", e),
        )
    })?;

    let same = crate::infra::model_storage::paths_equal(&old_path, &new_path);

    let (moved_entries, rewired_models) = if move_existing && !same {
        crate::infra::model_storage::migrate_models_dir(&old_path, &new_path, |old, new| {
            rewire_llama_model_paths(&app, old, new)
        })?
    } else {
        (0, 0)
    };

    if crate::infra::model_storage::paths_equal(&new_path, &default_hf_models_dir(&app)?) {
        persist_custom_llm_models_dir(&app, None)?;
    } else {
        persist_custom_llm_models_dir(&app, Some(new_path.to_string_lossy().as_ref()))?;
    }

    Ok(SetLlmModelsDirResult {
        path: new_path.to_string_lossy().to_string(),
        moved_entries,
        rewired_models,
    })
}

fn rewire_llama_model_paths(
    app: &AppHandle,
    old_prefix: &str,
    new_prefix: &str,
) -> Result<u32, String> {
    let mut conn = crate::storage_manager::db::open_db(app)?;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let rows: Vec<(String, String, Option<String>)> = {
        let mut stmt = tx
            .prepare(
                "SELECT id, name, advanced_model_settings FROM models WHERE provider_id = 'llamacpp'",
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let collected = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        collected
    };

    let mut count = 0u32;
    for (id, name, advanced) in rows {
        let new_name =
            crate::infra::model_storage::rewrite_path_prefix(&name, old_prefix, new_prefix);
        let mut changed = new_name != name;
        let new_advanced = match advanced.as_deref() {
            Some(text) => match serde_json::from_str::<serde_json::Value>(text) {
                Ok(mut value) => {
                    if let Some(map) = value.as_object_mut() {
                        for key in ["llamaMmprojPath", "llamaMtpModelPath"] {
                            if let Some(serde_json::Value::String(current)) = map.get(key) {
                                let rewritten = crate::infra::model_storage::rewrite_path_prefix(
                                    current, old_prefix, new_prefix,
                                );
                                if rewritten != *current {
                                    map.insert(
                                        key.to_string(),
                                        serde_json::Value::String(rewritten),
                                    );
                                    changed = true;
                                }
                            }
                        }
                    }
                    serde_json::to_string(&value).ok()
                }
                Err(_) => advanced.clone(),
            },
            None => None,
        };
        if changed {
            tx.execute(
                "UPDATE models SET name = ?1, advanced_model_settings = ?2 WHERE id = ?3",
                rusqlite::params![new_name, new_advanced, id],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            count += 1;
        }
    }

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(count)
}

fn persist_custom_llm_models_dir(app: &AppHandle, dir: Option<&str>) -> Result<(), String> {
    crate::infra::model_storage::persist_custom_dir(app, "customLlmModelsDir", dir)
}

#[tauri::command]
pub async fn hf_move_model_to_gguf_dir(
    app: AppHandle,
    source_path: String,
    model_name: Option<String>,
) -> Result<String, String> {
    let src = PathBuf::from(&source_path);

    if !src.exists() {
        return Err(format!("Source file does not exist: {}", source_path));
    }
    if !src.is_file() {
        return Err(format!("Source path is not a file: {}", source_path));
    }

    let models_dir = hf_models_dir(&app)?;

    if src.starts_with(&models_dir) {
        return Ok(source_path);
    }

    let filename = src
        .file_name()
        .ok_or_else(|| "Cannot determine filename from source path".to_string())?
        .to_string_lossy()
        .to_string();

    let folder_name = model_name
        .filter(|n| !n.trim().is_empty())
        .map(|n| n.replace('/', "--"))
        .unwrap_or_else(|| {
            src.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });

    let dest_dir = models_dir.join(&folder_name);
    if !dest_dir.exists() {
        tokio::fs::create_dir_all(&dest_dir).await.map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to create destination directory: {}", e),
            )
        })?;
    }

    let dest_path = dest_dir.join(&filename);

    if dest_path.exists() {
        log_info(
            &app,
            "hf_browser",
            format!(
                "Model already exists at destination: {}",
                dest_path.display()
            ),
        );
        let _ = tokio::fs::remove_file(&src).await;
        return Ok(dest_path.to_string_lossy().to_string());
    }

    if tokio::fs::rename(&src, &dest_path).await.is_ok() {
        log_info(
            &app,
            "hf_browser",
            format!(
                "Moved model (rename): {} -> {}",
                source_path,
                dest_path.display()
            ),
        );
        return Ok(dest_path.to_string_lossy().to_string());
    }

    log_info(
        &app,
        "hf_browser",
        format!(
            "Rename failed, falling back to copy: {} -> {}",
            source_path,
            dest_path.display()
        ),
    );

    tokio::fs::copy(&src, &dest_path).await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to copy model file: {}", e),
        )
    })?;

    tokio::fs::remove_file(&src).await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("File copied but failed to remove original: {}", e),
        )
    })?;

    log_info(
        &app,
        "hf_browser",
        format!(
            "Moved model (copy+delete): {} -> {}",
            source_path,
            dest_path.display()
        ),
    );

    Ok(dest_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn hf_fetch_readme(app: AppHandle, model_id: String) -> Result<String, String> {
    log_info(
        &app,
        "hf_browser",
        format!("fetching README for: {}", model_id),
    );

    let client = build_hf_client(&app)?;

    let url = format!("https://huggingface.co/{}/raw/main/README.md", model_id);

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch README: {}", e))?;

    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(if image_bundle::hf_token(&app).is_some() {
            "The saved Hugging Face token is invalid or expired.".to_string()
        } else {
            "This Hugging Face model requires an access token.".to_string()
        });
    }
    if resp.status() == reqwest::StatusCode::FORBIDDEN {
        return Err(format!("Accept access to {model_id} on Hugging Face, then retry."));
    }
    if !resp.status().is_success() {
        return Err(format!(
            "README not found (HTTP {})",
            resp.status().as_u16()
        ));
    }

    let raw = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read README body: {}", e))?;

    let content = if let Some(stripped) = raw.strip_prefix("---") {
        if let Some(end) = stripped.find("---") {
            let after = &stripped[end + 3..];
            after
                .trim_start_matches('\n')
                .trim_start_matches('\r')
                .to_string()
        } else {
            raw
        }
    } else {
        raw
    };

    Ok(content)
}

async fn fetch_gguf_meta(
    app: &AppHandle,
    model_id: &str,
    files: &[RunabilityFileInput],
) -> Option<GgufModelMeta> {
    let representative = files.iter().filter(|f| f.size > 0).min_by_key(|f| f.size)?;

    let url = format!(
        "https://huggingface.co/{}/resolve/main/{}",
        model_id, representative.filename
    );

    let client = build_hf_client(app).ok()?;

    // First attempt: 512KB
    let meta = fetch_gguf_range(&client, &url, 524_287).await;

    // If parse was incomplete, retry with 5MB
    let essentials_missing = |m: &GgufModelMeta| {
        m.architecture.is_none()
            || m.block_count.is_none()
            || m.embedding_length.is_none()
            || m.head_count.is_none()
            || m.context_length.is_none()
    };

    if let Some(ref m) = meta {
        if essentials_missing(m) {
            log_info(
                app,
                "hf_browser",
                format!(
                    "GGUF essentials missing after 512KB ({}/{} KVs parsed), retrying with 10MB",
                    m.parsed_kv_count, m.metadata_kv_count
                ),
            );
            let retry = fetch_gguf_range(&client, &url, 10_485_759).await;
            if let Some(ref r) = retry {
                log_gguf_meta(app, r);
                return retry;
            }
        }
    }

    if let Some(ref m) = meta {
        log_gguf_meta(app, m);
    }

    meta
}

async fn fetch_gguf_range(
    client: &reqwest::Client,
    url: &str,
    end_byte: u64,
) -> Option<GgufModelMeta> {
    let resp = client
        .get(url)
        .header("Range", format!("bytes=0-{}", end_byte))
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() && resp.status().as_u16() != 206 {
        return None;
    }

    let bytes = resp.bytes().await.ok()?;
    parse_gguf_meta(&bytes)
}

fn log_gguf_meta(app: &AppHandle, m: &GgufModelMeta) {
    log_info(
        app,
        "hf_browser",
        format!(
            "GGUF meta: arch={:?} blocks={:?} embd={:?} heads={:?} heads_kv={:?} ctx={:?} swa={:?} mla_rank={:?} parsed={}/{}",
            m.architecture, m.block_count, m.embedding_length,
            m.head_count, m.head_count_kv, m.context_length,
            m.sliding_window, m.kv_lora_rank,
            m.parsed_kv_count, m.metadata_kv_count,
        ),
    );
}

#[tauri::command]
pub async fn hf_compute_runability(
    app: AppHandle,
    model_id: String,
    files: Vec<RunabilityFileInput>,
    sprout_url: Option<String>,
    sprout_api_key: Option<String>,
) -> Result<Vec<RunabilityScore>, String> {
    if files.is_empty() {
        return Ok(vec![]);
    }

    log_info(
        &app,
        "hf_browser",
        format!(
            "computing runability for {} ({} files)",
            model_id,
            files.len()
        ),
    );

    let meta = fetch_gguf_meta(&app, &model_id, &files).await;

    let (available_ram, available_vram, supports_gpu_offload, unified_override) =
        resolve_runnability_hardware(sprout_url.as_deref(), sprout_api_key.as_deref()).await?;

    log_info(
        &app,
        "hf_browser",
        format!(
            "system: RAM={:?} VRAM={:?} supports_gpu_offload={} sprout={}",
            available_ram,
            available_vram,
            supports_gpu_offload,
            sprout_url.is_some()
        ),
    );

    let defaults = llama_runtime_defaults(&app);
    Ok(compute_scores(
        &files,
        &model_id,
        meta.as_ref(),
        available_ram,
        available_vram,
        unified_override,
        &defaults,
    ))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalRunabilityResult {
    pub score: u32,
    pub label: String,
    pub fits_in_ram: bool,
    pub fits_in_vram: bool,
    pub memory_score: u32,
    pub gpu_score: u32,
    pub kv_score: u32,
    pub quant_score: u32,
    pub gpu_mode: String,
    pub available_ram: u64,
    pub available_vram: u64,
    pub model_size: u64,
    pub quantization: String,
}

#[tauri::command]
pub async fn hf_compute_local_runability(
    app: AppHandle,
    file_path: String,
    llama_mmproj_path: Option<String>,
    llama_mtp_enabled: Option<bool>,
    llama_mtp_placement: Option<String>,
    llama_mtp_model_path: Option<String>,
) -> Result<LocalRunabilityResult, String> {
    let path = PathBuf::from(&file_path);
    if !path.exists() {
        return Err("File does not exist".to_string());
    }

    let file_size = std::fs::metadata(&path)
        .map(|m| m.len())
        .map_err(|e| format!("Failed to read file metadata: {e}"))?;

    let quantization = extract_quantization(&file_path);
    let meta = read_local_gguf_meta(&path);

    let available_ram = crate::llama_cpp::available_memory_bytes().unwrap_or(0);
    let supports_gpu_offload = crate::llama_cpp::supports_gpu_offload();
    let available_vram = if supports_gpu_offload {
        crate::llama_cpp::available_vram_bytes().unwrap_or(0)
    } else {
        0
    };
    let sidecar_reserve_bytes = if supports_gpu_offload {
        let mmproj_reserve = llama_mmproj_path
            .as_deref()
            .filter(|path| !path.trim().is_empty())
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|meta| meta.len())
            .unwrap_or(0);
        let mtp_reserve = if llama_mtp_enabled == Some(true)
            && llama_mtp_placement.as_deref() != Some("cpu")
        {
            llama_mtp_model_path
                .as_deref()
                .filter(|path| !path.trim().is_empty())
                .and_then(|path| std::fs::metadata(path).ok())
                .map(|meta| meta.len())
                .unwrap_or(0)
        } else {
            0
        };
        mmproj_reserve.saturating_add(mtp_reserve)
    } else {
        0
    };
    let unified = crate::llama_cpp::is_unified_memory();
    let total_available = resolve_total_available(available_ram, available_vram, unified);

    let defaults = llama_runtime_defaults(&app);
    let effective_ctx = meta
        .as_ref()
        .map(|m| effective_kv_context(m, defaults.context_length))
        .unwrap_or(defaults.context_length);
    let kv_8k: u64 = meta
        .as_ref()
        .and_then(kv_base_per_token)
        .map(|base| (base * defaults.kv_bytes_per_value * effective_ctx as f64) as u64)
        .unwrap_or(0);

    let quant_quality = quant_quality_with_qat(&quantization, is_qat_name(&file_path));

    log_info(
        &app,
        "hf_browser",
        format!(
            "local runability: path={} size={} quant={} RAM={} VRAM={} sidecar_reserve={} supports_gpu_offload={}",
            file_path,
            file_size,
            quantization,
            available_ram,
            available_vram,
            sidecar_reserve_bytes,
            supports_gpu_offload
        ),
    );

    let active_ratio = meta.as_ref().map(active_weight_ratio).unwrap_or(1.0);
    let estimated_resident_size =
        effective_model_size(file_size, meta.as_ref()).saturating_add(sidecar_reserve_bytes);
    let (score, fits_in_ram, fits_in_vram, memory_score, gpu_score, kv_score, gpu_mode) =
        score_configuration(
            estimated_resident_size,
            quant_quality,
            kv_8k,
            available_ram,
            total_available,
            available_vram,
            active_ratio,
        );

    Ok(LocalRunabilityResult {
        score,
        label: score_label(score),
        fits_in_ram,
        fits_in_vram,
        memory_score,
        gpu_score,
        kv_score,
        quant_score: (quant_quality.round() as u32).min(100),
        gpu_mode: gpu_mode.to_string(),
        available_ram,
        available_vram,
        model_size: file_size,
        quantization,
    })
}

#[tauri::command]
pub async fn hf_get_recommendation_data(
    app: AppHandle,
    model_id: String,
    files: Vec<RunabilityFileInput>,
    sprout_url: Option<String>,
    sprout_api_key: Option<String>,
) -> Result<RecommendationData, String> {
    if files.is_empty() {
        return Ok(RecommendationData {
            available_ram: 0,
            available_vram: 0,
            supports_gpu_offload: false,
            unified_memory: false,
            total_available: 0,
            kv_base_per_token: None,
            kv_context_cap: None,
            arch: None,
            model_max_context: 8192,
            files: vec![],
            best: None,
        });
    }

    log_info(
        &app,
        "hf_browser",
        format!(
            "computing recommendations for {} ({} files)",
            model_id,
            files.len()
        ),
    );

    let meta = fetch_gguf_meta(&app, &model_id, &files).await;
    let (available_ram_opt, available_vram_opt, supports_gpu_offload, unified_override) =
        resolve_runnability_hardware(sprout_url.as_deref(), sprout_api_key.as_deref()).await?;
    let available_ram = available_ram_opt.unwrap_or(0);
    let available_vram = available_vram_opt.unwrap_or(0);

    let defaults = llama_runtime_defaults(&app);
    Ok(build_recommendation(
        &files,
        &model_id,
        meta.as_ref(),
        available_ram,
        available_vram,
        supports_gpu_offload,
        unified_override,
        defaults.context_length,
    ))
}
