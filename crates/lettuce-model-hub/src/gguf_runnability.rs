//! GGUF header metadata and how well each file of a model runs on a machine:
//! a runnability score per file and the recommended file, context length and
//! KV cache type.

/// The model shape read from a GGUF header.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GgufModelMeta {
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
    pub metadata_kv_count: u64,
    pub parsed_kv_count: u64,
}

impl GgufModelMeta {
    /// Whether the fields every estimate needs were read.
    #[must_use]
    pub const fn has_essentials(&self) -> bool {
        self.architecture.is_some()
            && self.block_count.is_some()
            && self.embedding_length.is_some()
            && self.head_count.is_some()
            && self.context_length.is_some()
    }
}

/// How many leading bytes of a GGUF file are read first, and how many when
/// those did not hold the essentials.
pub const GGUF_HEADER_PROBE_BYTES: u64 = 524_288;
pub const GGUF_HEADER_RETRY_BYTES: u64 = 10_485_760;

pub(crate) struct GgufReader<'a> {
    data: &'a [u8],
    pub(crate) pos: usize,
    pub(crate) needed: usize,
}

impl<'a> GgufReader<'a> {
    pub(crate) const fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            needed: 0,
        }
    }

    pub(crate) const fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    pub(crate) fn read_bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.pos.checked_add(n)? > self.data.len() {
            self.needed = self.pos.checked_add(n)?;
            return None;
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Some(slice)
    }

    fn read_array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.read_bytes(N)?.try_into().ok()
    }

    pub(crate) fn read_u8(&mut self) -> Option<u8> {
        self.read_bytes(1).map(|bytes| bytes[0])
    }

    pub(crate) fn read_u32(&mut self) -> Option<u32> {
        self.read_array().map(u32::from_le_bytes)
    }

    pub(crate) fn read_u64(&mut self) -> Option<u64> {
        self.read_array().map(u64::from_le_bytes)
    }

    pub(crate) fn read_string(&mut self) -> Option<String> {
        let len = usize::try_from(self.read_u64()?).ok()?;
        if len > self.remaining() {
            self.needed = self.pos.checked_add(len)?;
            return None;
        }
        String::from_utf8(self.read_bytes(len)?.to_vec()).ok()
    }

    pub(crate) fn skip_value(&mut self, value_type: u32) -> Option<()> {
        match value_type {
            0 | 1 | 7 => {
                self.read_bytes(1)?;
            }
            2 | 3 => {
                self.read_bytes(2)?;
            }
            4..=6 => {
                self.read_bytes(4)?;
            }
            8 => {
                self.read_string()?;
            }
            9 => {
                let array_type = self.read_u32()?;
                let len = self.read_u64()?;
                for _ in 0..len {
                    self.skip_value(array_type)?;
                }
            }
            10..=12 => {
                self.read_bytes(8)?;
            }
            _ => return None,
        }
        Some(())
    }

    #[expect(
        clippy::cast_sign_loss,
        clippy::cast_possible_wrap,
        reason = "signed GGUF integers are reinterpreted as the header parser always did"
    )]
    pub(crate) fn read_value_as_u64(&mut self, value_type: u32) -> Option<u64> {
        match value_type {
            0 => self.read_u8().map(u64::from),
            1 => self.read_u8().map(|value| value as i8 as u64),
            2 => self
                .read_array()
                .map(|bytes| u64::from(u16::from_le_bytes(bytes))),
            3 => self
                .read_array()
                .map(|bytes| i16::from_le_bytes(bytes) as u64),
            4 => self.read_u32().map(u64::from),
            5 => self
                .read_array()
                .map(|bytes| i32::from_le_bytes(bytes) as u64),
            10 => self.read_u64(),
            11 => self
                .read_array()
                .map(|bytes| i64::from_le_bytes(bytes) as u64),
            9 => {
                let array_type = self.read_u32()?;
                let len = self.read_u64()?;
                let mut first = None;
                for index in 0..len {
                    if index == 0 {
                        first = self.read_value_as_u64(array_type);
                        if first.is_none() {
                            self.skip_value(array_type)?;
                        }
                    } else {
                        self.skip_value(array_type)?;
                    }
                }
                first
            }
            _ => None,
        }
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "the file type is a u32 field read through the integer coercion"
    )]
    fn read_value_as_u32(&mut self, value_type: u32) -> Option<u32> {
        self.read_value_as_u64(value_type).map(|value| value as u32)
    }
}

/// The model shape from the leading bytes of a GGUF file; keys past the end
/// of `data` are left unset. `None` when `data` is not a GGUF v2/v3 header.
#[must_use]
pub fn parse_gguf_meta(data: &[u8]) -> Option<GgufModelMeta> {
    let mut reader = GgufReader::new(data);
    if reader.read_bytes(4)? != b"GGUF" {
        return None;
    }
    let version = reader.read_u32()?;
    if !(2..=3).contains(&version) {
        return None;
    }
    reader.read_u64()?;
    let metadata_kv_count = reader.read_u64()?;
    let mut meta = GgufModelMeta {
        metadata_kv_count,
        ..GgufModelMeta::default()
    };
    let start = reader.pos;
    for _ in 0..metadata_kv_count {
        if reader.remaining() < 8 {
            break;
        }
        let Some(key) = reader.read_string() else {
            break;
        };
        let Some(value_type) = reader.read_u32() else {
            break;
        };
        if key == "general.architecture" && value_type == 8 {
            meta.architecture = reader.read_string();
            break;
        } else if reader.skip_value(value_type).is_none() {
            break;
        }
    }
    let arch = meta
        .architecture
        .clone()
        .unwrap_or_else(|| "llama".to_owned());
    let key = |suffix: &str| format!("{arch}.{suffix}");
    let block_count = key("block_count");
    let embedding_length = key("embedding_length");
    let head_count = key("attention.head_count");
    let head_count_kv = key("attention.head_count_kv");
    let context_length = key("context_length");
    let feed_forward_length = key("feed_forward_length");
    let sliding_window = key("attention.sliding_window");
    let kv_lora_rank = key("attention.kv_lora_rank");
    let key_length = key("attention.key_length");
    let value_length = key("attention.value_length");
    let expert_count = key("expert_count");
    let expert_used_count = key("expert_used_count");
    let expert_shared_count = key("expert_shared_count");
    let expert_feed_forward_length = key("expert_feed_forward_length");
    let nextn_predict_layers = key("nextn_predict_layers");
    reader.pos = start;
    let mut parsed = 0;
    for _ in 0..metadata_kv_count {
        if reader.remaining() < 8 {
            break;
        }
        let Some(key) = reader.read_string() else {
            break;
        };
        let Some(value_type) = reader.read_u32() else {
            break;
        };
        let before = reader.pos;
        if key == "general.file_type" {
            meta.file_type = reader.read_value_as_u32(value_type);
        } else {
            let slot = if key == block_count {
                Some(&mut meta.block_count)
            } else if key == embedding_length {
                Some(&mut meta.embedding_length)
            } else if key == head_count {
                Some(&mut meta.head_count)
            } else if key == head_count_kv {
                Some(&mut meta.head_count_kv)
            } else if key == context_length {
                Some(&mut meta.context_length)
            } else if key == feed_forward_length {
                Some(&mut meta.feed_forward_length)
            } else if key == sliding_window {
                Some(&mut meta.sliding_window)
            } else if key == kv_lora_rank {
                Some(&mut meta.kv_lora_rank)
            } else if key == key_length {
                Some(&mut meta.key_length)
            } else if key == value_length {
                Some(&mut meta.value_length)
            } else if key == expert_count {
                Some(&mut meta.expert_count)
            } else if key == expert_used_count {
                Some(&mut meta.expert_used_count)
            } else if key == expert_shared_count {
                Some(&mut meta.expert_shared_count)
            } else if key == expert_feed_forward_length {
                Some(&mut meta.expert_feed_forward_length)
            } else if key == nextn_predict_layers {
                Some(&mut meta.nextn_predict_layers)
            } else {
                None
            };
            if let Some(slot) = slot {
                *slot = reader.read_value_as_u64(value_type);
            }
        }
        if reader.pos == before && reader.skip_value(value_type).is_none() {
            break;
        }
        parsed += 1;
    }
    meta.parsed_kv_count = parsed;
    Some(meta)
}

/// A file's metadata read from its first bytes, and again from more bytes
/// when the header parsed but lacked the essentials; `None` when that second
/// read fails.
#[must_use]
pub fn gguf_meta_with_retry(
    probe: &[u8],
    retry: impl FnOnce() -> Option<Vec<u8>>,
) -> Option<GgufModelMeta> {
    let first = parse_gguf_meta(probe);
    if first.as_ref().is_none_or(GgufModelMeta::has_essentials) {
        return first;
    }
    let data = retry()?;
    parse_gguf_meta(&data).or(first)
}

pub fn quant_quality_score(quant: &str) -> f64 {
    match quant.to_uppercase().as_str() {
        "F32" | "BF16" | "F16" | "UD-BF16" => 100.0,
        "UD-Q8_K_XL" => 96.0,
        "Q8_K" | "Q8_K_S" | "Q8_K_L" | "Q8_K_XL" => 95.0,
        "UD-Q6_K_XL" => 92.0,
        "Q8_0" | "Q6_K" | "Q6_K_S" | "Q6_K_L" | "Q6_K_XL" => 90.0,
        "UD-Q5_K_XL" => 88.0,
        "Q5_K_M" | "Q5_K_L" | "Q5_K_XL" | "Q5_K" => 85.0,
        "UD-Q4_K_XL" => 82.0,
        "Q5_K_S" | "UD-IQ4_XS" | "UD-IQ4_NL" => 80.0,
        "Q4_K_M" | "Q4_K_L" | "Q4_K_XL" | "Q4_K" => 75.0,
        "IQ4_XS" | "IQ4_NL" => 72.0,
        "Q5_0" | "Q5_1" | "Q4_K_S" | "UD-Q3_K_XL" | "MXFP4_MOE" => 70.0,
        "Q4_0" | "Q4_1" | "Q3_K_M" | "Q3_K_L" | "Q3_K_XL" | "Q3_K" | "UD-IQ3_XXS" => 60.0,
        "UD-Q2_K_XL" => 55.0,
        "IQ3_M" | "IQ3_S" => 52.0,
        "Q3_K_S" | "UD-IQ2_M" => 50.0,
        "IQ3_XS" | "IQ3_XXS" => 45.0,
        "UD-IQ2_XXS" => 40.0,
        "Q2_K" | "Q2_K_S" | "Q2_K_M" | "Q2_K_L" | "Q2_K_XL" => 35.0,
        "UD-IQ1_M" => 30.0,
        "IQ2_M" | "IQ2_S" | "IQ2_XS" | "IQ2_XXS" => 25.0,
        "UD-IQ1_S" => 22.0,
        "IQ1_M" | "IQ1_S" => 15.0,
        _ => 50.0,
    }
}

fn architecture(meta: &GgufModelMeta) -> String {
    meta.architecture
        .as_deref()
        .unwrap_or("llama")
        .to_lowercase()
}

/// KV cache bytes per token before the KV type's bytes per value: blocks
/// (without MTP layers) × KV width × 2, or the compressed latent rank for
/// DeepSeek multi-head latent attention.
#[must_use]
pub fn kv_base_per_token(meta: &GgufModelMeta) -> Option<f64> {
    let nextn = meta.nextn_predict_layers.unwrap_or(0);
    let blocks = meta.block_count?.saturating_sub(nextn).max(1) as f64;
    let embd = meta.embedding_length? as f64;
    let heads = meta.head_count.filter(|&heads| heads > 0)? as f64;
    let heads_kv = meta.head_count_kv.unwrap_or(meta.head_count?) as f64;
    let arch = architecture(meta);
    if arch.starts_with("deepseek")
        && let Some(lora_rank) = meta.kv_lora_rank
    {
        return Some(blocks * lora_rank as f64 * 2.0);
    }
    Some(blocks * (embd * heads_kv / heads) * 2.0)
}

/// Bytes per KV value for a llama.cpp KV cache type name; `auto` and unknown
/// names count as f16.
#[must_use]
pub fn kv_bytes_per_value(kv_type: &str) -> f64 {
    match kv_type {
        "f32" => 4.0,
        "q8_1" => 1.0625,
        "q8_0" => 1.0,
        "q5_1" => 0.75,
        "q5_0" => 0.6875,
        "iq4_nl" => 0.5625,
        "q4_0" => 0.5,
        _ => 2.0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnabilityLabel {
    Excellent,
    Good,
    Marginal,
    Poor,
    Unrunnable,
}

impl RunnabilityLabel {
    #[must_use]
    pub const fn from_score(score: u32) -> Self {
        match score {
            80..=100 => Self::Excellent,
            60..=79 => Self::Good,
            40..=59 => Self::Marginal,
            20..=39 => Self::Poor,
            _ => Self::Unrunnable,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Excellent => "excellent",
            Self::Good => "good",
            Self::Marginal => "marginal",
            Self::Poor => "poor",
            Self::Unrunnable => "unrunnable",
        }
    }
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

/// The header fields shown next to a recommendation.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelArchInfo {
    pub meta: GgufModelMeta,
    pub is_moe: bool,
    pub active_weight_ratio: Option<f64>,
    pub incomplete_parse: bool,
}

impl From<&GgufModelMeta> for ModelArchInfo {
    fn from(meta: &GgufModelMeta) -> Self {
        let moe = is_moe(meta);
        Self {
            meta: meta.clone(),
            is_moe: moe,
            active_weight_ratio: moe.then(|| active_weight_ratio(meta)),
            incomplete_parse: !meta.has_essentials(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileRecommendation {
    pub filename: String,
    pub size: u64,
    pub quantization: String,
    pub quant_quality: u32,
    pub max_context_f16: u64,
    pub max_context_q8_0: u64,
    pub max_context_q4_0: u64,
    /// Longest Q8_0 context that keeps the model and its KV cache in VRAM;
    /// 0 when the model does not fit.
    pub optimal_gpu_ctx: u64,
    /// Longest Q8_0 context that fits in all memory before swapping.
    pub optimal_ram_ctx: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BestRecommendation {
    pub filename: String,
    pub context_length: u64,
    pub kv_type: String,
    pub score: u32,
    pub viable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecommendationData {
    pub available_ram: u64,
    pub available_vram: u64,
    pub supports_gpu_offload: bool,
    pub unified_memory: bool,
    pub total_available: u64,
    /// KV bytes per token before the KV type; bytes = base × bytes per value
    /// × context.
    pub kv_base_per_token: Option<f64>,
    pub kv_context_cap: Option<u64>,
    pub model_max_context: u64,
    pub arch: Option<ModelArchInfo>,
    pub files: Vec<FileRecommendation>,
    pub best: Option<BestRecommendation>,
}

impl RecommendationData {
    /// The recommendation for a model with no files.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            available_ram: 0,
            available_vram: 0,
            supports_gpu_offload: false,
            unified_memory: false,
            total_available: 0,
            kv_base_per_token: None,
            kv_context_cap: None,
            model_max_context: 8192,
            arch: None,
            files: Vec::new(),
            best: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gguf(entries: &[(&str, u32, Vec<u8>)]) -> Vec<u8> {
        let mut out = b"GGUF".to_vec();
        out.extend(3_u32.to_le_bytes());
        out.extend(0_u64.to_le_bytes());
        out.extend((entries.len() as u64).to_le_bytes());
        for (key, kind, value) in entries {
            out.extend((key.len() as u64).to_le_bytes());
            out.extend(key.as_bytes());
            out.extend(kind.to_le_bytes());
            out.extend(value);
        }
        out
    }

    fn text(value: &str) -> Vec<u8> {
        let mut out = (value.len() as u64).to_le_bytes().to_vec();
        out.extend(value.as_bytes());
        out
    }

    fn int(value: u32) -> Vec<u8> {
        value.to_le_bytes().to_vec()
    }

    fn ints(values: &[u32]) -> Vec<u8> {
        let mut out = 4_u32.to_le_bytes().to_vec();
        out.extend((values.len() as u64).to_le_bytes());
        for value in values {
            out.extend(value.to_le_bytes());
        }
        out
    }

    fn headers() -> Vec<Vec<u8>> {
        vec![
            gguf(&[
                ("general.name", 8, text("Llama")),
                ("general.file_type", 4, int(15)),
                ("general.architecture", 8, text("llama")),
                ("llama.block_count", 4, int(32)),
                ("llama.embedding_length", 4, int(4096)),
                ("llama.attention.head_count", 4, int(32)),
                (
                    "llama.attention.head_count_kv",
                    10,
                    8_u64.to_le_bytes().to_vec(),
                ),
                ("llama.context_length", 4, int(131_072)),
                ("tokenizer.ggml.scores", 9, ints(&[1, 2, 3])),
                ("llama.feed_forward_length", 5, int(14_336)),
            ]),
            gguf(&[
                ("general.architecture", 8, text("deepseek2")),
                ("deepseek2.block_count", 4, int(62)),
                ("deepseek2.embedding_length", 4, int(7168)),
                ("deepseek2.attention.head_count", 4, int(128)),
                ("deepseek2.attention.head_count_kv", 9, ints(&[128, 1])),
                ("deepseek2.context_length", 4, int(163_840)),
                ("deepseek2.attention.kv_lora_rank", 4, int(512)),
                ("deepseek2.expert_count", 4, int(256)),
                ("deepseek2.expert_used_count", 4, int(8)),
                ("deepseek2.expert_shared_count", 4, int(1)),
                ("deepseek2.nextn_predict_layers", 4, int(1)),
            ]),
            gguf(&[
                ("general.architecture", 8, text("gemma2")),
                ("gemma2.block_count", 4, int(42)),
                ("gemma2.embedding_length", 4, int(3584)),
                ("gemma2.attention.head_count", 4, int(16)),
                ("gemma2.attention.head_count_kv", 4, int(8)),
                ("gemma2.context_length", 4, int(8192)),
                ("gemma2.attention.sliding_window", 4, int(4096)),
            ]),
            gguf(&[
                ("general.alignment", 0, vec![32]),
                ("general.flag", 7, vec![1]),
                ("general.i8", 1, vec![0xff]),
                ("general.u16", 2, vec![1, 0]),
                ("general.i16", 3, vec![0xfe, 0xff]),
                ("general.f32", 6, vec![0, 0, 128, 63]),
                ("general.f64", 12, vec![0; 8]),
                ("general.i64", 11, (-5_i64).to_le_bytes().to_vec()),
                ("general.tags", 9, {
                    let mut out = 8_u32.to_le_bytes().to_vec();
                    out.extend(2_u64.to_le_bytes());
                    out.extend(text("a"));
                    out.extend(text("bc"));
                    out
                }),
                ("general.architecture", 8, text("cohere")),
                ("cohere.block_count", 2, vec![40, 0]),
                (
                    "cohere.embedding_length",
                    11,
                    8192_i64.to_le_bytes().to_vec(),
                ),
                ("cohere.attention.head_count", 1, vec![64]),
                (
                    "cohere.attention.head_count_kv",
                    5,
                    (-8_i32).to_le_bytes().to_vec(),
                ),
                ("cohere.context_length", 3, 16384_i16.to_le_bytes().to_vec()),
                ("cohere.attention.sliding_window", 0, vec![255]),
                ("general.file_type", 11, 7_i64.to_le_bytes().to_vec()),
            ]),
            gguf(&[
                ("general.architecture", 8, text("cohere")),
                ("cohere.block_count", 4, int(40)),
                ("cohere.embedding_length", 4, int(8192)),
                ("cohere.attention.head_count", 4, int(64)),
                ("cohere.attention.head_count_kv", 4, int(8)),
                ("cohere.context_length", 4, int(131_072)),
                ("cohere.attention.sliding_window", 4, int(4096)),
            ]),
            gguf(&[
                ("general.architecture", 7, vec![1]),
                ("llama.block_count", 4, int(16)),
            ]),
            gguf(&[
                ("general.architecture", 8, text("qwen3moe")),
                ("qwen3moe.block_count", 4, int(48)),
            ])[..60]
                .to_vec(),
        ]
    }

    #[test]
    fn reads_every_value_width_and_degrades_on_odd_headers() {
        let parsed = headers()
            .iter()
            .map(|header| {
                format!(
                    "{:?}",
                    parse_gguf_meta(header).map(|m| {
                        (
                            (
                                m.architecture.clone(),
                                m.block_count,
                                m.embedding_length,
                                m.head_count,
                                m.head_count_kv,
                                m.context_length,
                                m.feed_forward_length,
                                m.file_type,
                            ),
                            (
                                m.sliding_window,
                                m.kv_lora_rank,
                                m.expert_count,
                                m.expert_used_count,
                                m.expert_shared_count,
                                m.nextn_predict_layers,
                                m.metadata_kv_count,
                                m.parsed_kv_count,
                            ),
                        )
                    })
                )
            })
            .collect::<Vec<_>>();
        let expected = [
            r#"Some(((Some("llama"), Some(32), Some(4096), Some(32), Some(8), Some(131072), Some(14336), Some(15)), (None, None, None, None, None, None, 10, 10)))"#,
            r#"Some(((Some("deepseek2"), Some(62), Some(7168), Some(128), Some(128), Some(163840), None, None), (None, Some(512), Some(256), Some(8), Some(1), Some(1), 11, 11)))"#,
            r#"Some(((Some("gemma2"), Some(42), Some(3584), Some(16), Some(8), Some(8192), None, None), (Some(4096), None, None, None, None, None, 7, 7)))"#,
            r#"Some(((Some("cohere"), Some(40), Some(8192), Some(64), Some(18446744073709551608), Some(16384), None, Some(7)), (Some(255), None, None, None, None, None, 17, 17)))"#,
            r#"Some(((Some("cohere"), Some(40), Some(8192), Some(64), Some(8), Some(131072), None, None), (Some(4096), None, None, None, None, None, 7, 7)))"#,
            r#"Some(((None, Some(16), None, None, None, None, None, None), (None, None, None, None, None, None, 2, 2)))"#,
            r#"Some(((None, None, None, None, None, None, None, None), (None, None, None, None, None, None, 2, 0)))"#,
        ];
        assert_eq!(parsed, expected);
    }

    #[test]
    fn a_header_is_read_again_from_more_bytes_only_when_it_lacked_essentials() {
        let full = headers().remove(0);
        let mut retried = false;
        let meta = gguf_meta_with_retry(&full[..80], || {
            retried = true;
            Some(full.clone())
        })
        .expect("meta");
        assert!(retried);
        assert!(meta.has_essentials());
        assert_eq!(
            gguf_meta_with_retry(b"nope", || panic!("not a GGUF header")),
            None
        );
    }
}
