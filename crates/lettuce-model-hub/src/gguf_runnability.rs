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

struct GgufReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> GgufReader<'a> {
    const fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    const fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn read_bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.pos.checked_add(n)? > self.data.len() {
            return None;
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Some(slice)
    }

    fn read_array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.read_bytes(N)?.try_into().ok()
    }

    fn read_u8(&mut self) -> Option<u8> {
        self.read_bytes(1).map(|bytes| bytes[0])
    }

    fn read_u32(&mut self) -> Option<u32> {
        self.read_array().map(u32::from_le_bytes)
    }

    fn read_u64(&mut self) -> Option<u64> {
        self.read_array().map(u64::from_le_bytes)
    }

    fn read_string(&mut self) -> Option<String> {
        let len = usize::try_from(self.read_u64()?).ok()?;
        if len > self.remaining() {
            return None;
        }
        String::from_utf8(self.read_bytes(len)?.to_vec()).ok()
    }

    fn skip_value(&mut self, value_type: u32) -> Option<()> {
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
    fn read_value_as_u64(&mut self, value_type: u32) -> Option<u64> {
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

fn quant_quality_score(quant: &str) -> f64 {
    match quant.to_uppercase().as_str() {
        "F32" | "BF16" | "F16" => 100.0,
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

fn sliding_window_cap(meta: &GgufModelMeta) -> Option<u64> {
    let arch = architecture(meta);
    if arch == "gemma2" || arch == "cohere" {
        meta.sliding_window
    } else {
        None
    }
}

fn effective_kv_context(meta: &GgufModelMeta, requested: u64) -> u64 {
    sliding_window_cap(meta).map_or(requested, |window| requested.min(window))
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

/// Where the model and its KV cache would live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuMode {
    Full,
    NearFull,
    KvSpill,
    KvHeavySpill,
    RamModelVramCtx,
    RamModelRamCtx,
    MostLayers,
    HalfLayers,
    FewLayers,
    Cpu,
}

impl GpuMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::NearFull => "nearFull",
            Self::KvSpill => "kvSpill",
            Self::KvHeavySpill => "kvHeavySpill",
            Self::RamModelVramCtx => "ramModelVramCtx",
            Self::RamModelRamCtx => "ramModelRamCtx",
            Self::MostLayers => "mostLayers",
            Self::HalfLayers => "halfLayers",
            Self::FewLayers => "fewLayers",
            Self::Cpu => "cpu",
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

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "byte estimates truncate toward zero as the frozen formulas do"
)]
fn effective_model_size(file_size: u64, meta: Option<&GgufModelMeta>) -> u64 {
    let Some(meta) = meta else {
        return file_size;
    };
    let nextn = meta.nextn_predict_layers.unwrap_or(0);
    let Some(blocks) = meta.block_count.filter(|&blocks| blocks > 0) else {
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
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(|part| part == "qat")
}

fn quant_quality_with_qat(quant: &str, qat: bool) -> f64 {
    let base = quant_quality_score(quant);
    if qat { base.max(90.0) } else { base }
}

fn is_draft_file(name: &str) -> bool {
    name.to_lowercase().contains("draft") || crate::is_mtp_asset(name)
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "byte estimates truncate toward zero as the frozen formulas do"
)]
fn compute_overhead(model_size: u64, active_ratio: f64) -> u64 {
    let active_size = (model_size as f64 * active_ratio.clamp(0.05, 1.0)) as u64;
    let five_pct = (active_size as f64 * 0.05) as u64;
    five_pct.max(200_000_000)
}

/// One scored configuration and its parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigurationScore {
    pub score: u32,
    pub fits_in_ram: bool,
    pub fits_in_vram: bool,
    pub memory_score: u32,
    pub gpu_score: u32,
    pub kv_score: u32,
    pub gpu_mode: GpuMode,
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the frozen runnability formula"
)]
fn score_configuration(
    model_size: u64,
    quant_quality: f64,
    kv_cache_bytes: u64,
    available_ram: u64,
    total_available: u64,
    available_vram: u64,
    active_ratio: f64,
) -> ConfigurationScore {
    let overhead = compute_overhead(model_size, active_ratio);
    let total_needed = model_size
        .saturating_add(kv_cache_bytes)
        .saturating_add(overhead);
    let fits_in_ram = total_available > 0 && total_needed <= total_available;
    let ram_budget = (available_ram as f64 * 0.90) as u64;
    let vram_budget = (available_vram as f64 * 0.90) as u64;
    let model_fits_ram = available_ram > 0 && model_size.saturating_add(overhead) <= ram_budget;
    let kv_fits_vram = available_vram > 0 && kv_cache_bytes.saturating_add(overhead) <= vram_budget;
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
    let (gpu_score, fits_in_vram, gpu_mode) = if available_vram > 0 {
        if total_needed <= vram_budget {
            (100.0, true, GpuMode::Full)
        } else if model_size == 0 {
            (10.0, false, GpuMode::Cpu)
        } else if model_size <= vram_budget {
            let remaining = vram_budget.saturating_sub(model_size);
            let spill = kv_cache_bytes.saturating_add(overhead);
            let fit_ratio = if spill > 0 {
                (remaining as f64 / spill as f64).min(1.0)
            } else {
                1.0
            };
            let mode = if fit_ratio >= 0.8 {
                GpuMode::NearFull
            } else if fit_ratio >= 0.4 {
                GpuMode::KvSpill
            } else {
                GpuMode::KvHeavySpill
            };
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
                GpuMode::RamModelVramCtx,
            )
        } else if model_fits_ram {
            let ram_fit_ratio = if model_size.saturating_add(overhead) > 0 {
                (ram_budget as f64 / model_size.saturating_add(overhead) as f64).min(1.0)
            } else {
                1.0
            };
            (62.0 + ram_fit_ratio * 18.0, false, GpuMode::RamModelRamCtx)
        } else {
            let offload_ratio = (vram_budget as f64 / model_size as f64).min(1.0);
            let mode = if offload_ratio >= 0.75 {
                GpuMode::MostLayers
            } else if offload_ratio >= 0.5 {
                GpuMode::HalfLayers
            } else if offload_ratio >= 0.2 {
                GpuMode::FewLayers
            } else {
                GpuMode::Cpu
            };
            (10.0 + offload_ratio * 60.0, false, mode)
        }
    } else {
        (0.0, false, GpuMode::Cpu)
    };
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
    ConfigurationScore {
        score: (capped.round() as u32).min(100),
        fits_in_ram,
        fits_in_vram,
        memory_score: (memory_score.round() as u32).min(100),
        gpu_score: (gpu_score.round() as u32).min(100),
        kv_score: (kv_score.round() as u32).min(100),
        gpu_mode,
    }
}

/// Memory the machine can give a model: the larger pool when RAM and VRAM are
/// unified, else both together.
#[must_use]
pub const fn resolve_total_available(
    available_ram: u64,
    available_vram: u64,
    unified: bool,
) -> u64 {
    if unified && available_vram > 0 {
        if available_ram > available_vram {
            available_ram
        } else {
            available_vram
        }
    } else {
        available_ram.saturating_add(available_vram)
    }
}

/// What the machine offers; absent RAM or VRAM counts as none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunnabilityHardware {
    pub available_ram: Option<u64>,
    pub available_vram: Option<u64>,
    pub supports_gpu_offload: bool,
    pub unified_memory: bool,
}

/// The app's llama.cpp defaults the estimates assume.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunnabilityDefaults {
    pub context_length: u64,
    pub kv_bytes_per_value: f64,
}

impl RunnabilityDefaults {
    /// A stored default context below 512 counts as unset (8192); the KV type
    /// `auto` counts as f16.
    #[must_use]
    pub fn new(context_length: Option<u64>, kv_type: Option<&str>) -> Self {
        Self {
            context_length: context_length.filter(|value| *value >= 512).unwrap_or(8192),
            kv_bytes_per_value: kv_type.map_or(2.0, kv_bytes_per_value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnabilityFile {
    pub filename: String,
    pub size: u64,
    pub quantization: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnabilityScore {
    pub filename: String,
    pub score: u32,
    pub label: RunnabilityLabel,
    pub fits_in_ram: bool,
    pub fits_in_vram: bool,
    pub gpu_mode: GpuMode,
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "byte estimates truncate toward zero as the frozen formulas do"
)]
fn default_kv_bytes(meta: Option<&GgufModelMeta>, defaults: RunnabilityDefaults) -> u64 {
    let context = meta.map_or(defaults.context_length, |meta| {
        effective_kv_context(meta, defaults.context_length)
    });
    meta.and_then(kv_base_per_token).map_or(0, |base| {
        (base * defaults.kv_bytes_per_value * context as f64) as u64
    })
}

/// A score per file of `model_id` at the app's default context and KV type.
#[must_use]
pub fn runnability_scores(
    files: &[RunnabilityFile],
    model_id: &str,
    meta: Option<&GgufModelMeta>,
    hardware: RunnabilityHardware,
    defaults: RunnabilityDefaults,
) -> Vec<RunnabilityScore> {
    let ram = hardware.available_ram.unwrap_or(0);
    let vram = hardware.available_vram.unwrap_or(0);
    let total_available = resolve_total_available(ram, vram, hardware.unified_memory);
    let kv_bytes = default_kv_bytes(meta, defaults);
    let active_ratio = meta.map_or(1.0, active_weight_ratio);
    let repo_qat = is_qat_name(model_id);
    files
        .iter()
        .map(|file| {
            let qat = repo_qat || is_qat_name(&file.filename);
            let scored = score_configuration(
                effective_model_size(file.size, meta),
                quant_quality_with_qat(&file.quantization, qat),
                kv_bytes,
                ram,
                total_available,
                vram,
                active_ratio,
            );
            RunnabilityScore {
                filename: file.filename.clone(),
                score: scored.score,
                label: RunnabilityLabel::from_score(scored.score),
                fits_in_ram: scored.fits_in_ram,
                fits_in_vram: scored.fits_in_vram,
                gpu_mode: scored.gpu_mode,
            }
        })
        .collect()
}

/// A downloaded model file's score with its parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalRunnability {
    pub configuration: ConfigurationScore,
    pub label: RunnabilityLabel,
    pub quant_score: u32,
    pub available_ram: u64,
    pub available_vram: u64,
    pub model_size: u64,
    pub quantization: String,
}

/// The score of a model file on disk; `sidecar_bytes` are the projector and
/// MTP draft files loaded next to it on the GPU.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the quant quality is a small positive score"
)]
pub fn local_runnability(
    file_path: &str,
    file_size: u64,
    meta: Option<&GgufModelMeta>,
    hardware: RunnabilityHardware,
    sidecar_bytes: u64,
    defaults: RunnabilityDefaults,
) -> LocalRunnability {
    let available_ram = hardware.available_ram.unwrap_or(0);
    let available_vram = if hardware.supports_gpu_offload {
        hardware.available_vram.unwrap_or(0)
    } else {
        0
    };
    let sidecar_bytes = if hardware.supports_gpu_offload {
        sidecar_bytes
    } else {
        0
    };
    let total_available =
        resolve_total_available(available_ram, available_vram, hardware.unified_memory);
    let quantization = crate::extract_quantization(file_path);
    let quant_quality = quant_quality_with_qat(&quantization, is_qat_name(file_path));
    let configuration = score_configuration(
        effective_model_size(file_size, meta).saturating_add(sidecar_bytes),
        quant_quality,
        default_kv_bytes(meta, defaults),
        available_ram,
        total_available,
        available_vram,
        meta.map_or(1.0, active_weight_ratio),
    );
    LocalRunnability {
        label: RunnabilityLabel::from_score(configuration.score),
        configuration,
        quant_score: (quant_quality.round() as u32).min(100),
        available_ram,
        available_vram,
        model_size: file_size,
        quantization,
    }
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

const RECOMMENDATION_KV_TYPES: &[(&str, f64)] = &[
    ("q8_0", 1.0),
    ("q5_1", 0.75),
    ("q5_0", 0.6875),
    ("iq4_nl", 0.5625),
    ("q4_0", 0.5),
];
const MIN_CONTEXT: u64 = 4096;

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "context lengths truncate toward zero as the frozen formulas do"
)]
fn calculate_optimal_context(
    budget_bytes: u64,
    model_weight_bytes: u64,
    bytes_per_token: f64,
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
    if optimal > 1024 {
        optimal = (optimal / 1024) * 1024;
    }
    optimal
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "byte estimates truncate toward zero as the frozen formulas do"
)]
fn dynamic_safety_reserve(total_system_memory: u64) -> u64 {
    let reserve = (total_system_memory as f64 * 0.10) as u64;
    reserve.clamp(512_000_000, 2_000_000_000)
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "context lengths truncate toward zero as the frozen formulas do"
)]
fn max_context_for(
    model_size: u64,
    kv_base: f64,
    bytes_per_value: f64,
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
    let per_token = kv_base * bytes_per_value;
    if per_token <= 0.0 {
        return model_max_ctx;
    }
    let max = (available as f64 / per_token) as u64;
    max.min(model_max_ctx)
}

/// Per-file context limits and the best file, context length and KV type for
/// this machine; `default_context_cap` is the app's default context length.
#[must_use]
#[expect(
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the frozen recommendation formula, kept in one piece"
)]
pub fn build_recommendation(
    files: &[RunnabilityFile],
    model_id: &str,
    meta: Option<&GgufModelMeta>,
    hardware: RunnabilityHardware,
    default_context_cap: u64,
) -> RecommendationData {
    let available_ram = hardware.available_ram.unwrap_or(0);
    let available_vram = hardware.available_vram.unwrap_or(0);
    let unified = hardware.unified_memory;
    let total_available = resolve_total_available(available_ram, available_vram, unified);
    let kv_base = meta.and_then(kv_base_per_token);
    let model_max_ctx = meta.and_then(|meta| meta.context_length).unwrap_or(8192);
    let active_ratio = meta.map_or(1.0, active_weight_ratio);
    let kv_ctx_cap = meta.and_then(sliding_window_cap);
    let repo_qat = is_qat_name(model_id);
    let mut file_recs: Vec<FileRecommendation> = Vec::new();
    let mut best: Option<BestRecommendation> = None;
    let vram_budget = (available_vram as f64 * 0.90) as u64;
    for file in files {
        if file.size == 0 {
            continue;
        }
        let load_size = effective_model_size(file.size, meta);
        let quality =
            quant_quality_with_qat(&file.quantization, repo_qat || is_qat_name(&file.filename));
        let max_for = |bytes_per_value: f64| {
            kv_base.map_or(model_max_ctx, |base| {
                max_context_for(
                    load_size,
                    base,
                    bytes_per_value,
                    total_available,
                    model_max_ctx,
                    active_ratio,
                )
            })
        };
        let safety = dynamic_safety_reserve(total_available);
        let bytes_per_token_q8 = kv_base.unwrap_or(0.0);
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
            quant_quality: quality as u32,
            max_context_f16: max_for(2.0),
            max_context_q8_0: max_for(1.0),
            max_context_q4_0: max_for(0.5),
            optimal_gpu_ctx,
            optimal_ram_ctx,
        });
        if is_draft_file(&file.filename) {
            continue;
        }
        for &(kv_name, bytes_per_value) in RECOMMENDATION_KV_TYPES {
            let max_ctx = max_for(bytes_per_value);
            if max_ctx < MIN_CONTEXT {
                continue;
            }
            let ctx = max_ctx.min(default_context_cap);
            let effective_ctx = kv_ctx_cap.map_or(ctx, |cap| ctx.min(cap));
            let kv_bytes = kv_base.map_or(0, |base| {
                (base * bytes_per_value * effective_ctx as f64) as u64
            });
            let scored = score_configuration(
                load_size,
                quality,
                kv_bytes,
                available_ram,
                total_available,
                available_vram,
                active_ratio,
            );
            if best.as_ref().is_none_or(|best| scored.score > best.score) {
                best = Some(BestRecommendation {
                    filename: file.filename.clone(),
                    context_length: ctx,
                    kv_type: kv_name.to_owned(),
                    score: scored.score,
                    viable: scored.score >= 60,
                });
            }
        }
    }
    if let Some(base) = kv_base {
        let mut gpu_candidate: Option<(&RunnabilityFile, &FileRecommendation)> = None;
        for (file, rec) in files
            .iter()
            .filter(|file| file.size != 0)
            .zip(file_recs.iter())
        {
            let load_size = effective_model_size(file.size, meta);
            if load_size > vram_budget || is_draft_file(&file.filename) {
                continue;
            }
            let quality =
                quant_quality_with_qat(&file.quantization, repo_qat || is_qat_name(&file.filename));
            if gpu_candidate
                .as_ref()
                .is_none_or(|(_, previous)| quality > f64::from(previous.quant_quality))
            {
                gpu_candidate = Some((file, rec));
            }
        }
        if let Some((file, rec)) = gpu_candidate
            && rec.optimal_gpu_ctx >= MIN_CONTEXT
        {
            let ctx = rec.optimal_gpu_ctx;
            let effective_ctx = kv_ctx_cap.map_or(ctx, |cap| ctx.min(cap));
            let kv_bytes = (base * effective_ctx as f64) as u64;
            let scored = score_configuration(
                effective_model_size(file.size, meta),
                quant_quality_with_qat(&file.quantization, repo_qat || is_qat_name(&file.filename)),
                kv_bytes,
                available_ram,
                total_available,
                available_vram,
                active_ratio,
            );
            if scored.score > best.as_ref().map_or(0, |best| best.score) {
                best = Some(BestRecommendation {
                    filename: file.filename.clone(),
                    context_length: ctx,
                    kv_type: "q8_0".to_owned(),
                    score: scored.score,
                    viable: scored.score >= 60,
                });
            }
        }
    }
    RecommendationData {
        available_ram,
        available_vram,
        supports_gpu_offload: hardware.supports_gpu_offload,
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

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::*;

    const GB: u64 = 1_000_000_000;

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

    fn files() -> Vec<RunnabilityFile> {
        [
            ("m-Q2_K.gguf", 3 * GB, "Q2_K"),
            ("m-UD-Q4_K_XL.gguf", 5 * GB, "UD-Q4_K_XL"),
            ("m-qat-Q4_0.gguf", 4 * GB + 300_000_000, "Q4_0"),
            ("m-Q8_0.gguf", 8 * GB + 500_000_000, "Q8_0"),
            ("m-BF16.gguf", 16 * GB, "BF16"),
            ("draft-Q4_K_M.gguf", 500_000_000, "Q4_K_M"),
            ("m-IQ2_XXS.gguf", 90 * GB, "IQ2_XXS"),
            ("mtp-m-Q8_0.gguf", 400_000_000, "Q8_0"),
            ("m-UD-IQ1_S.gguf", 2 * GB, "UD-IQ1_S"),
            ("m-MXFP4_MOE.gguf", 11 * GB, "MXFP4_MOE"),
            ("m-TQ1_0.gguf", GB, "TQ1_0"),
            ("m-F32.gguf", 0, "F32"),
        ]
        .into_iter()
        .map(|(filename, size, quantization)| RunnabilityFile {
            filename: filename.to_owned(),
            size,
            quantization: quantization.to_owned(),
        })
        .collect()
    }

    fn some<T: std::fmt::Debug>(value: Option<T>) -> String {
        format!("{value:?}")
    }

    #[test]
    fn estimates_match_the_frozen_formulas_on_every_fixture() {
        let mut out = String::new();
        let metas = headers()
            .iter()
            .map(|header| parse_gguf_meta(header))
            .collect::<Vec<_>>();
        for meta in &metas {
            let line = meta.as_ref().map(|m| {
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
            });
            writeln!(out, "META {}", some(line)).expect("write");
        }
        for (context, kv) in [
            (None, None),
            (Some(256_u32), Some("q4_0")),
            (Some(4096), Some("auto")),
            (Some(65536), Some("iq4_nl")),
        ] {
            let defaults = RunnabilityDefaults::new(context.map(u64::from), kv);
            writeln!(
                out,
                "DEFAULTS {context:?} {kv:?} {} {}",
                defaults.context_length, defaults.kv_bytes_per_value
            )
            .expect("write");
        }
        let files = files();
        let mut all = metas.iter().map(Option::as_ref).collect::<Vec<_>>();
        all.push(None);
        let machines = [
            (32 * GB, 8 * GB, false),
            (16 * GB, 0, false),
            (64 * GB, 48 * GB, true),
            (6 * GB, 24 * GB, false),
            (2 * GB, 0, false),
            (0, 0, false),
            (0, 8 * GB, true),
            (16 * GB, 0, true),
        ];
        for (mi, meta) in all.iter().enumerate() {
            for (hi, (ram, vram, unified)) in machines.into_iter().enumerate() {
                let hardware = RunnabilityHardware {
                    available_ram: Some(ram),
                    available_vram: Some(vram),
                    supports_gpu_offload: true,
                    unified_memory: unified,
                };
                for (ctx, kv) in [(8192_u64, 2.0_f64), (32768, 1.0)] {
                    let defaults = RunnabilityDefaults {
                        context_length: ctx,
                        kv_bytes_per_value: kv,
                    };
                    for s in runnability_scores(&files, "org/model", *meta, hardware, defaults) {
                        writeln!(
                            out,
                            "SCORE {mi} {hi} {ctx} {} {} {} {} {} {}",
                            s.filename,
                            s.score,
                            s.label.as_str(),
                            s.fits_in_ram,
                            s.fits_in_vram,
                            s.gpu_mode.as_str()
                        )
                        .expect("write");
                    }
                    let r = build_recommendation(&files, "org/model-QAT", *meta, hardware, ctx);
                    writeln!(
                        out,
                        "REC {mi} {hi} {ctx} {} {} {} {} {} {}",
                        r.unified_memory,
                        r.total_available,
                        some(r.kv_base_per_token),
                        some(r.kv_context_cap),
                        r.model_max_context,
                        some(r.arch.as_ref().map(|a| (
                            a.is_moe,
                            a.active_weight_ratio,
                            a.incomplete_parse
                        )))
                    )
                    .expect("write");
                    for f in &r.files {
                        writeln!(
                            out,
                            "FILE {mi} {hi} {ctx} {} {} {} {} {} {} {} {} {}",
                            f.filename,
                            f.size,
                            f.quantization,
                            f.quant_quality,
                            f.max_context_f16,
                            f.max_context_q8_0,
                            f.max_context_q4_0,
                            f.optimal_gpu_ctx,
                            f.optimal_ram_ctx
                        )
                        .expect("write");
                    }
                    writeln!(
                        out,
                        "BEST {mi} {hi} {ctx} {}",
                        some(r.best.as_ref().map(|b| (
                            b.filename.clone(),
                            b.context_length,
                            b.kv_type.clone(),
                            b.score,
                            b.viable
                        )))
                    )
                    .expect("write");
                    for (sidecar, supports) in
                        [(0_u64, true), (900_000_000, true), (900_000_000, false)]
                    {
                        let local = local_runnability(
                            &files[1].filename,
                            files[1].size,
                            *meta,
                            RunnabilityHardware {
                                supports_gpu_offload: supports,
                                ..hardware
                            },
                            sidecar,
                            defaults,
                        );
                        let t = local.configuration;
                        writeln!(
                            out,
                            "LOCAL {mi} {hi} {ctx} {sidecar} {supports} {} {} {} {} {} {} {} {} {}",
                            t.score,
                            local.label.as_str(),
                            t.fits_in_ram,
                            t.fits_in_vram,
                            t.memory_score,
                            t.gpu_score,
                            t.kv_score,
                            local.quant_score,
                            t.gpu_mode.as_str()
                        )
                        .expect("write");
                    }
                    for f in &files {
                        let t = score_configuration(
                            f.size + 700_000_000,
                            quant_quality_with_qat(&f.quantization, false),
                            900_000_000,
                            ram,
                            resolve_total_available(ram, vram, unified),
                            vram,
                            meta.map_or(1.0, active_weight_ratio),
                        );
                        writeln!(
                            out,
                            "CONF {mi} {hi} {ctx} {} {} {} {} {} {} {} {}",
                            f.filename,
                            t.score,
                            t.fits_in_ram,
                            t.fits_in_vram,
                            t.memory_score,
                            t.gpu_score,
                            t.kv_score,
                            t.gpu_mode.as_str()
                        )
                        .expect("write");
                    }
                }
            }
        }
        let expected = include_str!("../tests/fixtures/legacy_runnability.txt");
        for (index, (actual, expected)) in out.lines().zip(expected.lines()).enumerate() {
            assert_eq!(actual, expected, "line {}", index + 1);
        }
        assert_eq!(out.lines().count(), expected.lines().count());
    }

    #[test]
    fn a_file_with_no_size_does_not_shift_the_gpu_candidate_onto_another_file() {
        let meta = headers()
            .first()
            .and_then(|header| parse_gguf_meta(header))
            .expect("meta");
        let zero = RunnabilityFile {
            filename: "m-F16.gguf".to_owned(),
            size: 0,
            quantization: "F16".to_owned(),
        };
        let sized = files();
        let mut with_zero = vec![zero];
        with_zero.extend(sized.iter().cloned());
        for vram in 1..=48 {
            for cap in [8192, 131_072] {
                let hardware = RunnabilityHardware {
                    available_ram: Some(64 * GB),
                    available_vram: Some(vram * GB),
                    supports_gpu_offload: true,
                    unified_memory: false,
                };
                let expected = build_recommendation(&sized, "org/m", Some(&meta), hardware, cap);
                let actual = build_recommendation(&with_zero, "org/m", Some(&meta), hardware, cap);
                assert_eq!(actual.best, expected.best, "{vram} GB, cap {cap}");
            }
        }
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
        assert_eq!(
            RunnabilityDefaults::new(Some(256), Some("auto")),
            RunnabilityDefaults {
                context_length: 8192,
                kv_bytes_per_value: 2.0
            }
        );
    }
}
