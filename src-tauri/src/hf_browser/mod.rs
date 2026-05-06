use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex as TokioMutex;

use crate::utils::log_info;

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfModelFile {
    pub filename: String,
    pub size: u64,
    pub quantization: String,
    pub is_mmproj: bool,
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
    pub architecture: Option<String>,
    pub context_length: Option<u64>,
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
    pub downloaded: u64,
    pub total: u64,
    pub speed_bytes_per_sec: u64,
    pub error: Option<String>,
    pub result_path: Option<String>,
    pub create_model_when_finished: bool,
    pub mmproj_file: MmprojFileLink,
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
    pub force_redownload: bool,
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
    pub force_redownload: bool,
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
pub async fn hf_get_avatars(authors: Vec<String>) -> Result<HashMap<String, String>, String> {
    let client = build_client()?;
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

fn hf_models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let lettuce_dir = crate::utils::lettuce_dir(app)?;
    let dir = lettuce_dir.join("models").join("gguf");
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

    /// Read a GGUF value as u64 (coercing integer types) Returns None for non-integer types
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
            _ => None,
        }
    }

    /// Read a GGUF value as u32 (coercing integer types)
    fn read_value_as_u32(&mut self, value_type: u32) -> Option<u32> {
        self.read_value_as_u64(value_type).map(|v| v as u32)
    }
}

/// Parse GGUF metadata from a byte buffer (typically the first 512KB–5MB of a GGUF file)
fn parse_gguf_meta(data: &[u8]) -> Option<GgufModelMeta> {
    let mut reader = GgufReader::new(data);

    // Magic: "GGUF"
    let magic = reader.read_bytes(4)?;
    if magic != b"GGUF" {
        return None;
    }

    // Version
    let version = reader.read_u32()?;
    if !(2..=3).contains(&version) {
        return None;
    }

    let _tensor_count = reader.read_u64()?;
    let metadata_kv_count = reader.read_u64()?;

    let mut meta = GgufModelMeta {
        metadata_kv_count,
        ..Default::default()
    };

    // First pass: find architecture name
    let start_pos = reader.pos;
    for _ in 0..metadata_kv_count {
        if reader.remaining() < 8 {
            break;
        }
        let key = match reader.read_string() {
            Some(k) => k,
            None => break,
        };
        let value_type = match reader.read_u32() {
            Some(t) => t,
            None => break,
        };

        if key == "general.architecture" && value_type == 8 {
            meta.architecture = reader.read_string();
            break;
        } else {
            if reader.skip_value(value_type).is_none() {
                break;
            }
        }
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

    reader.pos = start_pos;
    let mut parsed: u64 = 0;
    for _ in 0..metadata_kv_count {
        if reader.remaining() < 8 {
            break;
        }
        let key = match reader.read_string() {
            Some(k) => k,
            None => break,
        };
        let value_type = match reader.read_u32() {
            Some(t) => t,
            None => break,
        };

        let ok = if key == "general.architecture" {
            reader.skip_value(value_type).is_some()
        } else if key == "general.file_type" {
            meta.file_type = reader.read_value_as_u32(value_type);
            meta.file_type.is_some()
        } else if key == key_block_count {
            meta.block_count = reader.read_value_as_u64(value_type);
            meta.block_count.is_some()
        } else if key == key_embedding_length {
            meta.embedding_length = reader.read_value_as_u64(value_type);
            meta.embedding_length.is_some()
        } else if key == key_head_count {
            meta.head_count = reader.read_value_as_u64(value_type);
            meta.head_count.is_some()
        } else if key == key_head_count_kv {
            meta.head_count_kv = reader.read_value_as_u64(value_type);
            meta.head_count_kv.is_some()
        } else if key == key_context_length {
            meta.context_length = reader.read_value_as_u64(value_type);
            meta.context_length.is_some()
        } else if key == key_feed_forward {
            meta.feed_forward_length = reader.read_value_as_u64(value_type);
            meta.feed_forward_length.is_some()
        } else if key == key_sliding_window {
            meta.sliding_window = reader.read_value_as_u64(value_type);
            meta.sliding_window.is_some()
        } else if key == key_kv_lora_rank {
            meta.kv_lora_rank = reader.read_value_as_u64(value_type);
            meta.kv_lora_rank.is_some()
        } else if key == key_key_length {
            meta.key_length = reader.read_value_as_u64(value_type);
            meta.key_length.is_some()
        } else if key == key_value_length {
            meta.value_length = reader.read_value_as_u64(value_type);
            meta.value_length.is_some()
        } else {
            reader.skip_value(value_type).is_some()
        };

        if ok {
            parsed += 1;
        } else {
            break;
        }
    }
    meta.parsed_kv_count = parsed;

    Some(meta)
}

fn read_local_gguf_meta(path: &Path) -> Option<GgufModelMeta> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut primary = vec![0u8; 524_288];
    let primary_read = file.read(&mut primary).ok()?;
    primary.truncate(primary_read);

    let primary_meta = parse_gguf_meta(&primary);
    if primary_meta
        .as_ref()
        .is_some_and(|meta| meta.parsed_kv_count >= meta.metadata_kv_count)
    {
        return primary_meta;
    }

    let mut file = std::fs::File::open(path).ok()?;
    let mut fallback = vec![0u8; 5_242_880];
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
        "Q8_K" | "Q8_K_S" | "Q8_K_L" | "Q8_K_XL" => 95.0,
        "Q8_0" => 90.0, // Legacy 8-bit
        "Q6_K" | "Q6_K_S" | "Q6_K_L" | "Q6_K_XL" => 90.0,
        "Q5_K_M" | "Q5_K_L" | "Q5_K_XL" | "Q5_K" => 85.0,
        "Q5_K_S" => 80.0,
        "Q5_0" | "Q5_1" => 70.0, // Legacy 5-bit (−10)
        "Q4_K_M" | "Q4_K_L" | "Q4_K_XL" | "Q4_K" => 75.0,
        "Q4_K_S" => 70.0,
        "IQ4_XS" | "IQ4_NL" => 72.0, // I-quant 4-bit (↑ from 65)
        "Q4_0" | "Q4_1" => 60.0,     // Legacy 4-bit (−10)
        "Q3_K_M" | "Q3_K_L" | "Q3_K_XL" | "Q3_K" => 60.0,
        "Q3_K_S" => 50.0,
        "IQ3_M" | "IQ3_S" => 52.0, // I-quant 3-bit (↑ from 45)
        "IQ3_XS" | "IQ3_XXS" => 45.0,
        "Q2_K" | "Q2_K_S" | "Q2_K_M" | "Q2_K_L" | "Q2_K_XL" => 35.0,
        "IQ2_M" | "IQ2_S" | "IQ2_XS" | "IQ2_XXS" => 25.0,
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
    let blocks = meta.block_count? as f64;
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
        "f16" => 2.0,
        "q8_0" => 1.0,
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

/// Compute buffer overhead: scratch/work RAM for matrix multiplications.
/// Safe heuristic: max(model_size × 5%, 200MB).
fn compute_overhead(model_size: u64) -> u64 {
    let five_pct = (model_size as f64 * 0.05) as u64;
    five_pct.max(200_000_000) // 200MB minimum
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
) -> (u32, bool, bool, u32, u32, u32, &'static str) {
    let overhead = compute_overhead(model_size);
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
        if ratio < 1.2 {
            20.0
        } else if ratio < 1.5 {
            50.0
        } else if ratio < 2.0 {
            70.0
        } else if ratio < 3.0 {
            85.0
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

fn compute_scores(
    files: &[RunabilityFileInput],
    meta: Option<&GgufModelMeta>,
    available_ram: Option<u64>,
    available_vram: Option<u64>,
) -> Vec<RunabilityScore> {
    let ram = available_ram.unwrap_or(0);
    let vram = available_vram.unwrap_or(0);
    let unified = crate::llama_cpp::is_unified_memory();
    let total_available = resolve_total_available(ram, vram, unified);

    // KV cache at 8192 context, F16 KV
    let effective_ctx = meta.map(|m| effective_kv_context(m, 8192)).unwrap_or(8192);
    let kv_8k: u64 = meta
        .and_then(kv_base_per_token)
        .map(|base| (base * 2.0 * effective_ctx as f64) as u64) // F16 = 2.0 bytes
        .unwrap_or(0);

    files
        .iter()
        .map(|file| {
            let (score, fits_in_ram, fits_in_vram, .., gpu_mode) = score_configuration(
                file.size,
                quant_quality_score(&file.quantization),
                kv_8k,
                ram,
                total_available,
                vram,
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
    pub incomplete_parse: bool,
}

impl From<&GgufModelMeta> for ModelArchInfo {
    fn from(m: &GgufModelMeta) -> Self {
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
            incomplete_parse: m.parsed_kv_count < m.metadata_kv_count,
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

const KV_TYPES: &[(&str, f64)] = &[("f16", 2.0), ("q8_0", 1.0), ("q4_0", 0.5)];
const MIN_CONTEXT: u64 = 4096;

fn calculate_optimal_context(
    budget_bytes: u64,
    model_weight_bytes: u64,
    bytes_per_token: f64, // kv_base × bytes_per_value (accounts for GQA/MLA + KV quant)
    model_max_ctx: u64,
) -> u64 {
    let overhead = compute_overhead(model_weight_bytes);
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
) -> u64 {
    let safety = dynamic_safety_reserve(total_available);
    let overhead = compute_overhead(model_size);
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
    meta: Option<&GgufModelMeta>,
    available_ram: u64,
    available_vram: u64,
    supports_gpu_offload: bool,
) -> RecommendationData {
    let unified = crate::llama_cpp::is_unified_memory();
    let total_available = resolve_total_available(available_ram, available_vram, unified);
    let kv_base = meta.and_then(kv_base_per_token);
    let model_max_ctx = meta.and_then(|m| m.context_length).unwrap_or(8192);
    // SWA cap: for sliding window architectures, KV cache doesn't grow past window size
    let kv_ctx_cap = meta.and_then(|m| {
        let arch = m.architecture.as_deref().unwrap_or("llama").to_lowercase();
        if arch == "gemma2" || arch == "cohere" {
            m.sliding_window
        } else {
            None
        }
    });

    let mut file_recs: Vec<FileRecommendation> = Vec::new();
    let mut best: Option<BestRecommendation> = None;

    for file in files {
        if file.size == 0 {
            continue;
        }

        let qq = quant_quality_score(&file.quantization);
        let (max_f16, max_q8, max_q4) = if let Some(base) = kv_base {
            (
                max_context_for(file.size, base, 2.0, total_available, model_max_ctx),
                max_context_for(file.size, base, 1.0, total_available, model_max_ctx),
                max_context_for(file.size, base, 0.5, total_available, model_max_ctx),
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

        let optimal_gpu_ctx = if file.size <= vram_budget {
            calculate_optimal_context(vram_budget, file.size, bytes_per_token_q8, model_max_ctx)
        } else {
            0 // Model doesn't fit in VRAM
        };

        let optimal_ram_ctx = calculate_optimal_context(
            total_available.saturating_sub(safety),
            file.size,
            bytes_per_token_q8,
            model_max_ctx,
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

        // Try each KV type, find the best scoring configuration for this file
        for &(kv_name, bpv) in KV_TYPES {
            let max_ctx = if let Some(base) = kv_base {
                max_context_for(file.size, base, bpv, total_available, model_max_ctx)
            } else {
                model_max_ctx
            };

            if max_ctx < MIN_CONTEXT {
                continue;
            }

            let ctx = max_ctx.min(8192);
            let effective_ctx = kv_ctx_cap.map(|cap| ctx.min(cap)).unwrap_or(ctx);
            let kv_bytes = kv_base
                .map(|b| (b * bpv * effective_ctx as f64) as u64)
                .unwrap_or(0);

            let (score, ..) = score_configuration(
                file.size,
                qq,
                kv_bytes,
                available_ram,
                total_available,
                available_vram,
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
            if file.size == 0 || file.size > vram_budget {
                continue;
            }
            let qq = quant_quality_score(&file.quantization);
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
                    file.size,
                    quant_quality_score(&file.quantization),
                    kv_bytes,
                    available_ram,
                    total_available,
                    available_vram,
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
        .map_err(|e| format!("Failed to build HTTP client: {}", e))
}

#[tauri::command]
pub async fn hf_search_models(
    app: AppHandle,
    query: String,
    limit: Option<u32>,
    sort: Option<String>,
    offset: Option<u32>,
) -> Result<Vec<HfSearchResult>, String> {
    let limit = limit.unwrap_or(20).min(100);
    let sort_field = sort.unwrap_or_else(|| "trendingScore".to_string());
    let offset = offset.unwrap_or(0);

    let mut url = format!(
        "https://huggingface.co/api/models?filter=gguf&limit={}&sort={}&offset={}",
        limit, sort_field, offset
    );
    let trimmed = query.trim();
    if !trimmed.is_empty() {
        url.push_str(&format!("&search={}", urlencoding::encode(trimmed)));
    }

    log_info(&app, "hf_browser", format!("searching: {}", url));

    let client = build_client()?;
    let response = client.get(&url).send().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("HuggingFace API request failed: {}", e),
        )
    })?;

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

    log_info(
        &app,
        "hf_browser",
        format!("search returned {} results", results.len()),
    );

    Ok(results)
}

#[tauri::command]
pub async fn hf_get_model_files(app: AppHandle, model_id: String) -> Result<HfModelInfo, String> {
    log_info(
        &app,
        "hf_browser",
        format!("fetching model info: {}", model_id),
    );

    let client = build_client()?;

    let detail_url = format!("https://huggingface.co/api/models/{}", model_id);
    let detail_resp = client.get(&detail_url).send().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to fetch model detail: {}", e),
        )
    })?;

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

    let tree_url = format!(
        "https://huggingface.co/api/models/{}/tree/main?recursive=false",
        model_id
    );
    let tree_resp = client.get(&tree_url).send().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to fetch file tree: {}", e),
        )
    })?;

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
            lower.ends_with(".gguf") && !lower.contains("imatrix")
        })
        .map(|s| {
            let lower = s.rfilename.to_lowercase();
            let size = size_map.get(&s.rfilename).copied().unwrap_or(0);
            let quantization = extract_quantization(&s.rfilename);
            HfModelFile {
                filename: s.rfilename.clone(),
                size,
                quantization,
                is_mmproj: lower.contains("mmproj"),
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
            downloaded: 0,
            total: 0,
            speed_bytes_per_sec: 0,
            error: None,
            result_path: None,
            create_model_when_finished: metadata.create_model_when_finished,
            mmproj_file: metadata.mmproj_file,
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
            force_redownload: metadata.force_redownload,
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
            }
            emit_queue(app, &state.queue);
        }

        let result = do_queue_download(app, &item).await;

        {
            let mut state = HF_DOWNLOAD_QUEUE.lock().await;
            state.cancel_ids.remove(&item.id);
            if let Some(d) = state.queue.iter_mut().find(|d| d.id == item.id) {
                match result {
                    Ok(path) => {
                        d.status = "complete".to_string();
                        d.downloaded = d.total;
                        d.result_path = Some(path);
                    }
                    Err(ref e) if e.contains("cancelled") => {
                        d.status = "cancelled".to_string();
                    }
                    Err(ref e) => {
                        d.status = "error".to_string();
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
            if m.len() >= min_existing_size {
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
        format!(
            "starting download: {} → {}",
            download_url,
            dest_path.display()
        ),
    );

    let client = reqwest::Client::builder()
        .user_agent("LettuceAI/1.0")
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| format!("Failed to build client: {}", e))?;

    let response = client.get(&download_url).send().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to start download: {}", e),
        )
    })?;

    if !response.status().is_success() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Download failed with status: {}", response.status()),
        ));
    }

    let total_size = response.content_length().unwrap_or(0);

    {
        let mut state = HF_DOWNLOAD_QUEUE.lock().await;
        if let Some(d) = state.queue.iter_mut().find(|d| d.id == queue_id) {
            d.total = total_size;
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

    while let Some(chunk_result) = stream.next().await {
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
                    let size = file_entry.metadata().map(|m| m.len()).unwrap_or(0);
                    let meta = read_local_gguf_meta(&file_path);
                    results.push(DownloadedGgufModel {
                        model_id: dir_name.replace("--", "/"),
                        filename: fname,
                        path: file_path.to_string_lossy().to_string(),
                        size,
                        quantization: extract_quantization(&file_path.to_string_lossy()),
                        is_mmproj,
                        architecture: meta.as_ref().and_then(|value| value.architecture.clone()),
                        context_length: meta.as_ref().and_then(|value| value.context_length),
                    });
                }
            }
        }
    }

    Ok(results)
}

#[tauri::command]
pub async fn hf_delete_downloaded_model(app: AppHandle, file_path: String) -> Result<(), String> {
    let path = PathBuf::from(&file_path);

    if !path.exists() {
        return Ok(());
    }

    let models_dir = hf_models_dir(&app)?;
    if !path.starts_with(&models_dir) {
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
        if parent != models_dir {
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

    let client = build_client()?;

    let url = format!("https://huggingface.co/{}/raw/main/README.md", model_id);

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch README: {}", e))?;

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

    let client = reqwest::Client::builder()
        .user_agent("LettuceAI/1.0")
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .ok()?;

    // First attempt: 512KB
    let meta = fetch_gguf_range(&client, &url, 524_287).await;

    // If parse was incomplete, retry with 5MB
    if let Some(ref m) = meta {
        if m.parsed_kv_count < m.metadata_kv_count && m.block_count.is_none() {
            log_info(
                app,
                "hf_browser",
                format!(
                    "GGUF header truncated ({}/{} KVs parsed), retrying with 5MB",
                    m.parsed_kv_count, m.metadata_kv_count
                ),
            );
            let retry = fetch_gguf_range(&client, &url, 5_242_879).await;
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

    let available_ram = crate::llama_cpp::available_memory_bytes();
    let supports_gpu_offload = crate::llama_cpp::supports_gpu_offload();
    let available_vram = if supports_gpu_offload {
        crate::llama_cpp::available_vram_bytes()
    } else {
        Some(0)
    };

    log_info(
        &app,
        "hf_browser",
        format!(
            "system: RAM={:?} VRAM={:?} supports_gpu_offload={}",
            available_ram, available_vram, supports_gpu_offload
        ),
    );

    Ok(compute_scores(
        &files,
        meta.as_ref(),
        available_ram,
        available_vram,
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
    let unified = crate::llama_cpp::is_unified_memory();
    let total_available = resolve_total_available(available_ram, available_vram, unified);

    // KV cache at 8192 context, F16 KV
    let effective_ctx = meta
        .as_ref()
        .map(|m| effective_kv_context(m, 8192))
        .unwrap_or(8192);
    let kv_8k: u64 = meta
        .as_ref()
        .and_then(kv_base_per_token)
        .map(|base| (base * 2.0 * effective_ctx as f64) as u64)
        .unwrap_or(0);

    let quant_quality = quant_quality_score(&quantization);

    log_info(
        &app,
        "hf_browser",
        format!(
            "local runability: path={} size={} quant={} RAM={} VRAM={} supports_gpu_offload={}",
            file_path, file_size, quantization, available_ram, available_vram, supports_gpu_offload
        ),
    );

    let (score, fits_in_ram, fits_in_vram, memory_score, gpu_score, kv_score, gpu_mode) =
        score_configuration(
            file_size,
            quant_quality,
            kv_8k,
            available_ram,
            total_available,
            available_vram,
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
    let available_ram = crate::llama_cpp::available_memory_bytes().unwrap_or(0);
    let supports_gpu_offload = crate::llama_cpp::supports_gpu_offload();
    let available_vram = if supports_gpu_offload {
        crate::llama_cpp::available_vram_bytes().unwrap_or(0)
    } else {
        0
    };

    Ok(build_recommendation(
        &files,
        meta.as_ref(),
        available_ram,
        available_vram,
        supports_gpu_offload,
    ))
}
