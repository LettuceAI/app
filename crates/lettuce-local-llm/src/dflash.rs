//! DFlash speculative decoding as llama.cpp's `common/speculative.cpp` runs
//! it: a block-diffusion drafter that reads the target's inputs at a few of
//! its layers. Covers DFlash, DFlash2 (selector lattice) and DSpark (DFlash
//! with a Markov head and an optional confidence head) drafters: detecting
//! one, finding one beside the model, the drafter context (a default
//! context bound to the target), feature injection into the drafter's
//! cache, and the draft/verify rounds with their adaptive draft length.

use std::collections::VecDeque;
use std::path::Path;

use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::gguf::GgufContext;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::{LlamaModel, RopeType};
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token::LlamaToken;

use crate::mtp::greedy_token_with_prob;
pub use crate::request::{DFLASH_DRAFT_DEFAULT, DFLASH_DRAFT_MAX, DFLASH_P_MIN_DEFAULT};

const DFLASH_BLOCK_SIZE_KEY: &str = "dflash.block_size";
const DFLASH_ARCHITECTURE_KEY: &str = "general.architecture";
const DFLASH_ARCHITECTURE: &str = "dflash";
const MROPE_POSITION_ROWS: usize = 4;
const DFLASH_CAUSAL_KEY: &str = "dflash.attention.causal";
const DFLASH_SAMPLE_FROM_ANCHOR_KEY: &str = "dflash.sample_from_anchor";
const DFLASH_HAS_CONFIDENCE_HEAD_KEY: &str = "dflash.has_confidence_head";
const DSPARK_MARKOV_TENSOR: &str = "markov_w1.weight";
const DFLASH_BLOCK_SIZE_FALLBACK: usize = 16;
const DFLASH_ADAPT_WINDOW_ROUNDS: u32 = 8;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct DflashError(pub String);

fn fail<E: std::fmt::Display>(context: &str) -> impl FnOnce(E) -> DflashError + '_ {
    move |error| DflashError(format!("{context}: {error}"))
}

/// The drafter family, which decides the noise block layout, the draft
/// length limit and how drafts are read and truncated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrafterKind {
    Dflash,
    Dflash2,
    Dspark,
}

impl DrafterKind {
    /// DSpark when the GGUF carries the Markov head, else DFlash2 when the
    /// drafter has a selector lattice, else DFlash.
    #[must_use]
    pub fn detect(dspark: bool, selector_top_k: usize) -> Self {
        if dspark {
            Self::Dspark
        } else if selector_top_k > 0 {
            Self::Dflash2
        } else {
            Self::Dflash
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Dflash => "DFlash",
            Self::Dflash2 => "DFlash2",
            Self::Dspark => "DSpark",
        }
    }
}

pub struct DflashRuntime<'m> {
    pub draft: LlamaContext<'m>,
    anchor: Option<LlamaToken>,
    pub kind: DrafterKind,
    pub draft_n: usize,
    pub draft_n_max: usize,
    pub block_size: usize,
    pub p_min: f32,
    pub causal_attn: bool,
    pub sample_from_anchor: bool,
    pub has_confidence_head: bool,
    pub selector_top_k: usize,
    pub adaptation_count: u32,
    adaptive_rounds: u32,
    adaptive_drafted: u64,
    adaptive_matched: u64,
    pub n_embd_tgt: usize,
    pub n_embd_enc: usize,
    pub target_layers: Vec<u32>,
    pub mask_token: LlamaToken,
    mrope: bool,
    features: Vec<f32>,
    pub pending: VecDeque<LlamaToken>,
    pub rounds: u64,
    pub drafted: u64,
    pub accepted: u64,
}

impl std::fmt::Debug for DflashRuntime<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DflashRuntime")
            .field("anchor", &self.anchor)
            .field("kind", &self.kind)
            .field("draft_n", &self.draft_n)
            .field("draft_n_max", &self.draft_n_max)
            .field("block_size", &self.block_size)
            .field("p_min", &self.p_min)
            .field("causal_attn", &self.causal_attn)
            .field("sample_from_anchor", &self.sample_from_anchor)
            .field("has_confidence_head", &self.has_confidence_head)
            .field("selector_top_k", &self.selector_top_k)
            .field("target_layers", &self.target_layers)
            .field("rounds", &self.rounds)
            .field("drafted", &self.drafted)
            .field("accepted", &self.accepted)
            .finish_non_exhaustive()
    }
}

/// Whether the GGUF at `model_path` is a DFlash drafter: its architecture
/// is `dflash`, as llama.cpp identifies one, or it carries
/// `dflash.block_size`.
#[must_use]
pub fn model_is_dflash(model_path: &str) -> bool {
    let Some(gguf) = GgufContext::from_file(Path::new(model_path)) else {
        return false;
    };
    let architecture = gguf.find_key(DFLASH_ARCHITECTURE_KEY);
    (architecture >= 0 && gguf.val_str(architecture) == Some(DFLASH_ARCHITECTURE))
        || gguf.find_key(DFLASH_BLOCK_SIZE_KEY) >= 0
}

/// Whether a drafter stem (lowercase) names the model stem: the drafter
/// contains the whole model stem, or the model stem contains the part of the
/// drafter name before the `dflash` marker (after it when nothing precedes
/// it), so a drafter's own quantization suffix does not matter. A stem that
/// is only the marker names no model.
fn shares_stem(name: &str, model_stem: &str) -> bool {
    let separators = |c: char| matches!(c, '-' | '_' | '.' | ' ');
    let (before, after) = name.split_once("dflash").unwrap_or((name, ""));
    let before = before.trim_matches(separators);
    let base = if before.is_empty() {
        after.trim_matches(separators)
    } else {
        before
    };
    !base.is_empty()
        && !model_stem.is_empty()
        && (name.contains(model_stem) || model_stem.contains(base))
}

/// The candidates that share the model's stem, ordered by lowercase stem.
fn rank_candidates(model_stem: &str, candidates: Vec<String>) -> Vec<String> {
    let mut named: Vec<(String, String)> = candidates
        .into_iter()
        .filter_map(|candidate| {
            let name = Path::new(&candidate)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_lowercase)?;
            shares_stem(&name, model_stem).then_some((name, candidate))
        })
        .collect();
    named.sort();
    named.into_iter().map(|(_, candidate)| candidate).collect()
}

/// A DFlash drafter beside the model: a `.gguf` whose stem contains
/// `dflash`, which is a DFlash GGUF and whose stem shares the model's.
#[must_use]
pub fn discover_external_dflash(model_path: &str) -> Option<String> {
    let path = Path::new(model_path);
    let model_stem = path.file_stem()?.to_str()?.to_lowercase();
    let dir = path.parent()?;
    let entries = std::fs::read_dir(dir).ok()?;

    let candidates: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let candidate = entry.path();
            if candidate.extension().and_then(|ext| ext.to_str())? != "gguf" {
                return None;
            }
            let candidate_path = candidate.to_str()?.to_string();
            if candidate_path == model_path {
                return None;
            }
            let name = candidate.file_stem()?.to_str()?.to_lowercase();
            if !name.contains("dflash") || !model_is_dflash(&candidate_path) {
                return None;
            }
            Some(candidate_path)
        })
        .collect();

    rank_candidates(&model_stem, candidates).into_iter().next()
}

/// Whether the GGUF carries a DSpark Markov head (`markov_w1.weight`), the
/// tensor upstream uses to tell DSpark drafters from DFlash ones.
#[must_use]
pub fn model_is_dspark(model_path: &str) -> bool {
    let Some(gguf) = GgufContext::from_file(Path::new(model_path)) else {
        return false;
    };
    (0..gguf.n_tensors()).any(|index| gguf.tensor_name(index) == Some(DSPARK_MARKOV_TENSOR))
}

/// The drafter for a request: the configured path when it is a DFlash
/// GGUF (DSpark drafters included), else one discovered beside the model.
#[must_use]
pub fn resolve_drafter(configured: Option<&str>, model_path: &str) -> Option<String> {
    configured
        .filter(|path| model_is_dflash(path))
        .map(ToOwned::to_owned)
        .or_else(|| discover_external_dflash(model_path))
}

fn block_size_for(model: &LlamaModel) -> usize {
    parse_block_size(model.meta_val_str(DFLASH_BLOCK_SIZE_KEY).ok().as_deref())
}

fn parse_block_size(value: Option<&str>) -> usize {
    value
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|size| *size > 1)
        .unwrap_or(DFLASH_BLOCK_SIZE_FALLBACK)
}

/// A boolean GGUF flag as upstream reads it: set only by the text `true`.
fn metadata_flag(value: Option<&str>, default: bool) -> bool {
    value.map_or(default, |value| value == "true")
}

/// llama.cpp builds the drafter's injection input as `target_layers x` the
/// drafter's own `n_embd`, while the features come in as `target_layers x`
/// the target's; the widths match only when the two agree.
fn injection_width_matches(target_n_embd: usize, draft_n_embd: usize) -> bool {
    target_n_embd > 0 && target_n_embd == draft_n_embd
}

/// The mask token is embedded with the target's token embeddings, so it must
/// be a token of both vocabularies.
fn mask_token_fits(mask_token: i32, draft_n_vocab: i32, target_n_vocab: i32) -> bool {
    mask_token >= 0 && mask_token < draft_n_vocab.min(target_n_vocab)
}

/// The positions of an injection batch for an M-RoPE drafter, as
/// llama.cpp's speculative runner writes them: rows 1 and 2 repeat the
/// position, row 3 is zero.
fn mrope_extra_rows(positions: &[i32]) -> [Vec<i32>; 3] {
    [
        positions.to_vec(),
        positions.to_vec(),
        vec![0; positions.len()],
    ]
}

/// Whether a DSpark drafter can truncate at `p_min`: it needs its
/// confidence head whenever `p_min` is above zero. A drafter whose metadata
/// omits `dflash.has_confidence_head` counts as having one.
fn confidence_available(kind: DrafterKind, p_min: f32, has_confidence_head: bool) -> bool {
    kind != DrafterKind::Dspark || p_min <= 0.0 || has_confidence_head
}

/// Whether the noise block starts with a prediction slot rather than the
/// bonus anchor: DSpark drafters sampling from the anchor.
fn anchor_is_prediction(kind: DrafterKind, sample_from_anchor: bool) -> bool {
    kind == DrafterKind::Dspark && sample_from_anchor
}

/// The most drafts one block yields: the whole trained block when the
/// anchor slot is itself a prediction, one less otherwise.
fn max_draft_len(kind: DrafterKind, sample_from_anchor: bool, block_size: usize) -> usize {
    if anchor_is_prediction(kind, sample_from_anchor) {
        block_size
    } else {
        block_size.saturating_sub(1)
    }
}

/// The draft length a run starts with: at least one, at most the drafter's
/// `max_draft`, and small enough that the target's verification batch (the
/// anchor plus the drafts) fits its `n_batch`. `None` when the target batch
/// cannot hold even one draft.
fn clamp_draft_n(draft_n: usize, max_draft: usize, target_batch: usize) -> Option<usize> {
    let limit = target_batch.checked_sub(1).filter(|limit| *limit > 0)?;
    Some(draft_n.max(1).min(max_draft.max(1)).min(limit))
}

/// The noise block for `steps` drafts: its length (`[anchor, mask...]`)
/// and the first slot read as a draft. The block is `steps` long with slot
/// 0 read when the anchor slot is a prediction, else `steps + 1` long read
/// from slot 1; a DFlash2 selector always reads from slot 1.
fn block_layout(
    kind: DrafterKind,
    sample_from_anchor: bool,
    selector: bool,
    steps: usize,
) -> (usize, usize) {
    if anchor_is_prediction(kind, sample_from_anchor) {
        (steps, usize::from(selector))
    } else {
        (steps + 1, 1)
    }
}

/// Reads drafts from the block slots `first..len`. DSpark stops at the
/// first slot whose confidence falls below `p_min` (read only when `p_min`
/// is above zero) and takes the greedy token; the others stop at the first
/// greedy token whose top-10 probability falls below `p_min`.
fn read_drafts<E>(
    dspark: bool,
    first: usize,
    len: usize,
    p_min: f32,
    mut confidence: impl FnMut(i32) -> Result<f32, E>,
    mut greedy: impl FnMut(i32) -> Option<(LlamaToken, f32)>,
) -> Result<Vec<LlamaToken>, E> {
    let mut drafted = Vec::with_capacity(len.saturating_sub(first));
    for slot in first..len {
        let slot = slot as i32;
        if dspark && p_min > 0.0 && confidence(slot)? < p_min {
            break;
        }
        let Some((token, prob)) = greedy(slot) else {
            break;
        };
        if !dspark && prob < p_min {
            break;
        }
        drafted.push(token);
    }
    Ok(drafted)
}

/// Walks the verification rows: row `i` is sampled once, drafts are
/// accepted while each row's sample equals the draft, and an accepted
/// end-of-generation draft ends the walk. Returns how many drafts matched
/// and the token sampled after them.
fn verify_drafts(
    drafted: &[LlamaToken],
    mut sample_row: impl FnMut(i32) -> LlamaToken,
    is_eog: impl Fn(LlamaToken) -> bool,
) -> (usize, LlamaToken) {
    let mut matched = 0usize;
    let mut extra = sample_row(0);
    while matched < drafted.len() && extra == drafted[matched] {
        matched += 1;
        extra = sample_row(matched as i32);
        if is_eog(drafted[matched - 1]) {
            break;
        }
    }
    (matched, extra)
}

impl<'m> DflashRuntime<'m> {
    /// A drafter context bound to `target_ctx`; `dspark` says the GGUF
    /// carries a DSpark Markov head. Fails, so the run goes on without
    /// DFlash, for a drafter whose target layers the target lacks, whose
    /// feature width or vocabulary does not match the target, without a mask
    /// token inside its vocabulary, whose DFlash2 selector does not fit its
    /// output rows, for a DSpark drafter asked to truncate at `p_min` without
    /// a confidence head, or when the target batch cannot verify a draft.
    #[expect(
        clippy::too_many_arguments,
        reason = "the target, drafter and request inputs"
    )]
    pub fn new(
        target_model: &LlamaModel,
        draft_model: &'m LlamaModel,
        target_ctx: &LlamaContext<'_>,
        backend: &LlamaBackend,
        draft_params: LlamaContextParams,
        dspark: bool,
        draft_n: usize,
        p_min: f32,
    ) -> Result<Self, DflashError> {
        let target_layers: Vec<u32> = draft_model
            .target_layer_ids()
            .iter()
            .map(|layer| u32::try_from(*layer))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| DflashError("DFlash target layer id is negative".to_string()))?;
        if target_layers.is_empty() {
            return Err(DflashError(
                "DFlash draft model exposes no target_layer_ids".to_string(),
            ));
        }

        let n_embd_tgt = usize::try_from(target_model.n_embd())
            .map_err(|_| DflashError("target n_embd does not fit into usize".to_string()))?;

        let n_layers_target = target_model.n_layer() as usize;
        if let Some(layer) = target_layers
            .iter()
            .find(|layer| **layer as usize >= n_layers_target)
        {
            return Err(DflashError(format!(
                "DFlash draft model requests target layer {layer}, but the target model has {n_layers_target} layers"
            )));
        }

        let selector_top_k = usize::try_from(draft_model.dflash_selector_top_k()).unwrap_or(0);
        let lattice_width = usize::try_from(draft_model.n_embd_out()).unwrap_or(0);
        if selector_top_k > 0 && selector_top_k * (selector_top_k + 1) > lattice_width {
            return Err(DflashError(format!(
                "DFlash2 selector top-k {selector_top_k} does not fit the drafter's {lattice_width}-wide output rows"
            )));
        }

        let draft_n_embd = usize::try_from(draft_model.n_embd()).unwrap_or(0);
        if !injection_width_matches(n_embd_tgt, draft_n_embd) {
            return Err(DflashError(format!(
                "DFlash draft model reads {} features per target layer, but the target model has {n_embd_tgt}",
                draft_n_embd
            )));
        }
        if !target_model.is_speculation_compatible(draft_model) {
            return Err(DflashError(
                "DFlash draft model's vocabulary does not match the target model's".to_string(),
            ));
        }

        let mask_token = draft_model.token_mask();
        if !mask_token_fits(mask_token.0, draft_model.n_vocab(), target_model.n_vocab()) {
            return Err(DflashError(format!(
                "DFlash draft model has no usable mask token ({})",
                mask_token.0
            )));
        }

        let kind = DrafterKind::detect(dspark, selector_top_k);
        let p_min = p_min.clamp(0.0, 1.0);
        let has_confidence_head = metadata_flag(
            draft_model
                .meta_val_str(DFLASH_HAS_CONFIDENCE_HEAD_KEY)
                .ok()
                .as_deref(),
            true,
        );
        if !confidence_available(kind, p_min, has_confidence_head) {
            return Err(DflashError(
                "DSpark draft model has no confidence head; a minimum draft probability of 0 is required"
                    .to_string(),
            ));
        }

        let block_size = block_size_for(draft_model);
        let causal_attn = metadata_flag(
            draft_model.meta_val_str(DFLASH_CAUSAL_KEY).ok().as_deref(),
            false,
        );
        let sample_from_anchor = metadata_flag(
            draft_model
                .meta_val_str(DFLASH_SAMPLE_FROM_ANCHOR_KEY)
                .ok()
                .as_deref(),
            true,
        );
        let target_batch = target_ctx.n_batch() as usize;
        let max_draft = max_draft_len(kind, sample_from_anchor, block_size);
        let draft_n = clamp_draft_n(draft_n, max_draft, target_batch).ok_or_else(|| {
            DflashError(format!(
                "target batch of {target_batch} cannot verify a DFlash draft"
            ))
        })?;

        let block = (draft_n + 1) as u32;
        let params = draft_params
            .with_ctx_other(target_ctx)
            .with_n_ubatch(target_ctx.n_ubatch().max(block))
            .with_n_outputs_max(block);
        let draft = draft_model
            .new_context(backend, params)
            .map_err(fail("failed to create DFlash draft context"))?;
        tracing::debug!(
            kind = kind.label(),
            block_size,
            mask_token = mask_token.0,
            n_extract = target_layers.len(),
            sample_from_anchor,
            has_confidence_head,
            causal_attn,
            selector_top_k,
            "DFlash drafter metadata"
        );

        Ok(Self {
            draft,
            anchor: None,
            kind,
            draft_n,
            draft_n_max: draft_n,
            block_size,
            p_min,
            causal_attn,
            sample_from_anchor,
            has_confidence_head,
            selector_top_k,
            adaptation_count: 0,
            adaptive_rounds: 0,
            adaptive_drafted: 0,
            adaptive_matched: 0,
            n_embd_tgt,
            n_embd_enc: target_layers.len() * n_embd_tgt,
            target_layers,
            mask_token,
            mrope: draft_model.rope_type() == Some(RopeType::MRope),
            features: Vec::new(),
            pending: VecDeque::new(),
            rounds: 0,
            drafted: 0,
            accepted: 0,
        })
    }

    /// Makes the target keep its inputs at the drafter's layers, and the
    /// drafter attend causally only when its metadata asks for it. Its nextn
    /// embeddings cover the rows that request outputs, or every row for a
    /// DFlash2 drafter, whose selector lattice is read from them.
    pub fn enable_feature_extraction(
        &mut self,
        target: &mut LlamaContext<'_>,
    ) -> Result<(), DflashError> {
        for layer in &self.target_layers {
            target.set_embeddings_layer_inp(*layer, true);
        }
        let masked = self.selector_top_k == 0;
        self.draft.set_embeddings_nextn(true, masked).map_err(fail(
            "failed to enable nextn embeddings on DFlash draft context",
        ))?;
        self.draft.set_causal_attn(self.causal_attn);
        Ok(())
    }

    /// Applies the request's minimum draft probability to a runtime kept
    /// from an earlier request (the hot context key does not carry it).
    /// Fails for a DSpark drafter without a confidence head asked to
    /// truncate at a `p_min` above zero.
    pub fn refresh_settings(&mut self, p_min: f32) -> Result<(), DflashError> {
        let p_min = p_min.clamp(0.0, 1.0);
        if !confidence_available(self.kind, p_min, self.has_confidence_head) {
            return Err(DflashError(
                "DSpark draft model has no confidence head; a minimum draft probability of 0 is required"
                    .to_string(),
            ));
        }
        self.p_min = p_min;
        Ok(())
    }

    fn reset_rounds(&mut self) {
        self.anchor = None;
        self.pending.clear();
        self.rounds = 0;
        self.drafted = 0;
        self.accepted = 0;
        self.adaptation_count = 0;
        self.adaptive_rounds = 0;
        self.adaptive_drafted = 0;
        self.adaptive_matched = 0;
    }

    pub fn reset_for_prompt_reuse(&mut self, draft_clear_from: u32) -> Result<(), DflashError> {
        let cleared = self
            .draft
            .clear_kv_cache_seq(Some(0), Some(draft_clear_from), None)
            .map_err(fail("failed to rewind DFlash prompt cache"))?;
        if !cleared {
            return Err(DflashError(format!(
                "DFlash prompt cache rewind failed at position {draft_clear_from}"
            )));
        }
        self.reset_rounds();
        Ok(())
    }

    pub fn truncate_for_prompt_cache(
        &mut self,
        target: &mut LlamaContext<'_>,
        token_count: u32,
    ) -> Result<(), DflashError> {
        let target_trimmed = target
            .clear_kv_cache_seq(Some(0), Some(token_count), None)
            .map_err(fail("failed to trim target prompt cache"))?;
        if !target_trimmed {
            return Err(DflashError(format!(
                "target prompt cache trim failed at position {token_count}"
            )));
        }
        let draft_trimmed = self
            .draft
            .clear_kv_cache_seq(Some(0), Some(token_count), None)
            .map_err(fail("failed to trim DFlash prompt cache"))?;
        if !draft_trimmed {
            return Err(DflashError(format!(
                "DFlash prompt cache trim failed at position {token_count}"
            )));
        }
        self.anchor = None;
        self.pending.clear();
        Ok(())
    }

    fn record_adaptive_round(&mut self, drafted: usize, matched: usize) {
        self.adaptive_rounds += 1;
        self.adaptive_drafted += drafted as u64;
        self.adaptive_matched += matched as u64;
        if self.adaptive_rounds < DFLASH_ADAPT_WINDOW_ROUNDS {
            return;
        }
        let next = adjusted_draft_length(
            self.draft_n,
            self.draft_n_max,
            self.adaptive_drafted,
            self.adaptive_matched,
        );
        if next != self.draft_n {
            self.draft_n = next;
            self.adaptation_count += 1;
        }
        self.adaptive_rounds = 0;
        self.adaptive_drafted = 0;
        self.adaptive_matched = 0;
    }

    /// Writes the target's layer inputs of the last decoded batch (rows
    /// `0..n_rows`, at positions from `start_pos`) into the drafter's cache:
    /// one embedding decode per drafter micro-batch, which projects and
    /// stores them.
    pub fn inject(
        &mut self,
        target: &LlamaContext<'_>,
        n_rows: usize,
        start_pos: i32,
    ) -> Result<(), DflashError> {
        let chunk = (self.draft.n_ubatch() as usize).max(1);
        let mut offset = 0usize;
        while offset < n_rows {
            let n_chunk = chunk.min(n_rows - offset);
            self.gather_features(target, offset, n_chunk)?;
            let chunk_pos = i32::try_from(offset)
                .ok()
                .and_then(|offset| start_pos.checked_add(offset))
                .ok_or_else(|| {
                    DflashError("DFlash injection position overflowed i32".to_string())
                })?;
            let position_rows = if self.mrope { MROPE_POSITION_ROWS } else { 1 };
            let mut batch = LlamaBatch::new_embeddings_only_with_position_rows(
                n_chunk,
                self.n_embd_enc,
                1,
                position_rows,
            );
            for row in 0..n_chunk {
                let start = row * self.n_embd_enc;
                batch
                    .add_embedding(
                        &self.features[start..start + self.n_embd_enc],
                        chunk_pos + row as i32,
                        &[0],
                        false,
                    )
                    .map_err(fail("failed to build DFlash injection batch"))?;
            }
            if self.mrope {
                let positions: Vec<i32> = (0..n_chunk).map(|row| chunk_pos + row as i32).collect();
                for (index, row) in mrope_extra_rows(&positions).iter().enumerate() {
                    batch
                        .set_position_row(index + 1, row)
                        .map_err(fail("failed to build DFlash injection positions"))?;
                }
            }
            self.draft
                .decode(&mut batch)
                .map_err(fail("DFlash cache injection failed"))?;
            offset += n_chunk;
        }
        Ok(())
    }

    fn gather_features(
        &mut self,
        target: &LlamaContext<'_>,
        row_offset: usize,
        n_rows: usize,
    ) -> Result<(), DflashError> {
        self.features.clear();
        self.features.resize(n_rows * self.n_embd_enc, 0.0);

        for (k, layer) in self.target_layers.iter().enumerate() {
            let rows = target
                .embeddings_layer_inp(*layer, row_offset + n_rows)
                .map_err(fail(&format!(
                    "failed to read target layer {layer} embeddings"
                )))?;
            for row in 0..n_rows {
                let src_start = (row_offset + row) * self.n_embd_tgt;
                let src = rows
                    .get(src_start..src_start + self.n_embd_tgt)
                    .ok_or_else(|| {
                        DflashError(format!("target layer {layer} embeddings are truncated"))
                    })?;
                let dst_start = row * self.n_embd_enc + k * self.n_embd_tgt;
                self.features[dst_start..dst_start + self.n_embd_tgt].copy_from_slice(src);
            }
        }
        Ok(())
    }

    /// One round at `pos`, the next free position. The first round after a
    /// prompt samples the anchor from the prompt's logits. Later rounds draft
    /// a block after the anchor (still outside the target's cache at
    /// `pos - 1`), verify anchor and drafts in one target batch, write that
    /// batch's features into the drafter, and keep the accepted prefix in
    /// both caches. Returns the tokens to emit; the last becomes the next
    /// anchor.
    pub fn round(
        &mut self,
        target: &mut LlamaContext<'_>,
        sampler: &mut LlamaSampler,
        model: &LlamaModel,
        pos: i32,
        max_pos: i32,
    ) -> Result<Vec<LlamaToken>, DflashError> {
        self.rounds += 1;

        let Some(anchor) = self.anchor else {
            let first = sampler.sample(target, -1);
            self.anchor = Some(first);
            self.accepted += 1;
            return Ok(vec![first]);
        };
        let anchor_pos = pos - 1;

        let steps = self.draft_n.min((max_pos - pos).max(0) as usize);
        let drafted = if steps == 0 {
            Vec::new()
        } else {
            self.draft_block(anchor, anchor_pos, steps)?
        };
        self.drafted += drafted.len() as u64;

        let mut batch = LlamaBatch::new(drafted.len() + 1, 1);
        batch
            .add(anchor, anchor_pos, &[0], true)
            .map_err(fail("failed to build DFlash verification batch"))?;
        for (i, token) in drafted.iter().enumerate() {
            batch
                .add(*token, pos + i as i32, &[0], true)
                .map_err(fail("failed to build DFlash verification batch"))?;
        }
        target
            .decode(&mut batch)
            .map_err(fail("DFlash verification decode failed"))?;
        self.inject(target, drafted.len() + 1, anchor_pos)?;

        let (matched, extra) = verify_drafts(
            &drafted,
            |row| sampler.sample(target, row),
            |token| model.is_eog_token(token),
        );

        let keep_until = u32::try_from(pos + matched as i32).map_err(|_| {
            DflashError("DFlash rollback position does not fit into u32".to_string())
        })?;
        let target_rolled_back = target
            .clear_kv_cache_seq(Some(0), Some(keep_until), None)
            .map_err(fail("failed to roll back target KV cache"))?;
        if !target_rolled_back {
            return Err(DflashError(format!(
                "target KV rollback failed at position {keep_until}"
            )));
        }
        let draft_rolled_back = self
            .draft
            .clear_kv_cache_seq(Some(0), Some(keep_until), None)
            .map_err(fail("failed to roll back DFlash draft KV cache"))?;
        if !draft_rolled_back {
            return Err(DflashError(format!(
                "DFlash draft KV rollback failed at position {keep_until}"
            )));
        }

        let mut accepted: Vec<LlamaToken> = Vec::with_capacity(matched + 1);
        accepted.extend_from_slice(&drafted[..matched]);
        accepted.push(extra);
        self.anchor = Some(extra);
        self.accepted += accepted.len() as u64;
        self.record_adaptive_round(drafted.len(), matched);

        Ok(accepted)
    }

    /// Decodes the noise block `[anchor, mask...]` from `anchor_pos` in the
    /// drafter (see [`block_layout`]), reads its draft slots (greedy top-10
    /// tokens, the DFlash2 selector walk, or DSpark's confidence-truncated
    /// greedy tokens) until one falls below `p_min`, and drops the block from
    /// the drafter's cache again.
    fn draft_block(
        &mut self,
        anchor: LlamaToken,
        anchor_pos: i32,
        steps: usize,
    ) -> Result<Vec<LlamaToken>, DflashError> {
        let selector = self.selector_top_k > 0;
        let (n_block, first) = block_layout(self.kind, self.sample_from_anchor, selector, steps);
        let mut batch = LlamaBatch::new(n_block, 1);
        for i in 0..n_block {
            let token = if i == 0 { anchor } else { self.mask_token };
            batch
                .add(token, anchor_pos + i as i32, &[0], !selector)
                .map_err(fail("failed to build DFlash noise batch"))?;
        }
        self.draft
            .decode(&mut batch)
            .map_err(fail("DFlash draft decode failed"))?;

        let drafted = if selector {
            let rows = (first..n_block)
                .map(|i| self.draft.embeddings_nextn_ith(i as i32))
                .collect::<Result<Vec<_>, _>>()
                .map_err(fail("failed to read the DFlash2 selector lattice"))?;
            walk_selector_lattice(&rows, self.selector_top_k, self.p_min)
        } else {
            let draft = &self.draft;
            read_drafts(
                self.kind == DrafterKind::Dspark,
                first,
                n_block,
                self.p_min,
                |slot| {
                    draft
                        .embeddings_nextn_ith(slot)
                        .map_err(fail("failed to read the DSpark confidence"))?
                        .first()
                        .copied()
                        .ok_or_else(|| DflashError("DSpark confidence row is empty".to_string()))
                },
                |slot| greedy_token_with_prob(draft.get_logits_ith(slot)),
            )?
        };

        let rollback = u32::try_from(anchor_pos).map_err(|_| {
            DflashError("DFlash draft rollback position does not fit into u32".to_string())
        })?;
        let rolled_back = self
            .draft
            .clear_kv_cache_seq(Some(0), Some(rollback), None)
            .map_err(fail("failed to roll back DFlash draft KV cache"))?;
        if !rolled_back {
            return Err(DflashError(format!(
                "DFlash draft KV rollback failed at position {rollback}"
            )));
        }

        Ok(drafted)
    }
}

/// DFlash2's walk over its selector lattice, one row per mask position:
/// `top_k` candidate ids, then a `top_k` by `top_k` score table indexed by
/// the previous position's chosen slot (the first row's predecessor is the
/// anchor, slot 0). Each step takes the best-scoring slot and, with `p_min`
/// above zero, stops when that slot's softmax probability falls below it.
fn walk_selector_lattice(rows: &[&[f32]], top_k: usize, p_min: f32) -> Vec<LlamaToken> {
    let mut drafted = Vec::with_capacity(rows.len());
    let mut predecessor = 0usize;
    for row in rows {
        let start = top_k + predecessor * top_k;
        let Some(scores) = row.get(start..start + top_k) else {
            break;
        };
        let Some(best) = scores
            .iter()
            .enumerate()
            .fold(None::<(usize, f32)>, |best, (slot, &score)| match best {
                Some((_, top)) if score <= top => best,
                _ => Some((slot, score)),
            })
            .map(|(slot, _)| slot)
        else {
            break;
        };
        predecessor = best;
        if p_min > 0.0 {
            let sum: f32 = scores
                .iter()
                .map(|&score| (score - scores[best]).exp())
                .sum();
            if 1.0 / sum < p_min {
                break;
            }
        }
        drafted.push(LlamaToken::new(row[best] as i32));
    }
    drafted
}

fn adjusted_draft_length(current: usize, maximum: usize, drafted: u64, matched: u64) -> usize {
    if drafted == 0 || matched.saturating_mul(2) < drafted {
        return (current / 2).max(1);
    }
    if matched.saturating_mul(5) >= drafted.saturating_mul(4) {
        return current.saturating_add(1).min(maximum);
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adaptive_draft_length_halves_low_acceptance() {
        assert_eq!(adjusted_draft_length(8, 8, 16, 7), 4);
        assert_eq!(adjusted_draft_length(1, 8, 16, 0), 1);
        assert_eq!(adjusted_draft_length(6, 8, 0, 0), 3);
    }

    #[test]
    fn adaptive_draft_length_grows_high_acceptance_to_configured_limit() {
        assert_eq!(adjusted_draft_length(3, 6, 10, 8), 4);
        assert_eq!(adjusted_draft_length(6, 6, 10, 10), 6);
    }

    #[test]
    fn adaptive_draft_length_holds_middle_acceptance() {
        assert_eq!(adjusted_draft_length(4, 8, 10, 6), 4);
        assert_eq!(adjusted_draft_length(4, 8, 10, 5), 4);
    }

    #[test]
    fn the_draft_length_fits_inside_the_block_and_the_target_batch() {
        let dflash = |block| max_draft_len(DrafterKind::Dflash, true, block);
        assert_eq!(clamp_draft_n(4, dflash(16), 512), Some(4));
        assert_eq!(clamp_draft_n(15, dflash(8), 512), Some(7));
        assert_eq!(clamp_draft_n(0, dflash(16), 512), Some(1));
        assert_eq!(clamp_draft_n(5, dflash(1), 512), Some(1));
        assert_eq!(clamp_draft_n(15, dflash(16), 8), Some(7));
        assert_eq!(clamp_draft_n(15, dflash(16), 2), Some(1));
        assert_eq!(clamp_draft_n(4, dflash(16), 1), None);
        assert_eq!(clamp_draft_n(4, dflash(16), 0), None);
        assert_eq!(parse_block_size(Some(" 8 ")), 8);
        assert_eq!(parse_block_size(Some("1")), DFLASH_BLOCK_SIZE_FALLBACK);
        assert_eq!(parse_block_size(Some("x")), DFLASH_BLOCK_SIZE_FALLBACK);
        assert_eq!(parse_block_size(None), DFLASH_BLOCK_SIZE_FALLBACK);
    }

    #[test]
    fn dspark_sampling_from_the_anchor_drafts_the_whole_block() {
        assert_eq!(max_draft_len(DrafterKind::Dspark, true, 7), 7);
        assert_eq!(max_draft_len(DrafterKind::Dspark, false, 7), 6);
        assert_eq!(max_draft_len(DrafterKind::Dflash, true, 7), 6);
        assert_eq!(max_draft_len(DrafterKind::Dflash2, true, 16), 15);
        let dspark = max_draft_len(DrafterKind::Dspark, true, 7);
        assert_eq!(clamp_draft_n(15, dspark, 512), Some(7));
        assert_eq!(clamp_draft_n(15, dspark, 6), Some(5));
    }

    #[test]
    fn drafter_kinds_follow_the_markov_head_then_the_selector() {
        assert_eq!(DrafterKind::detect(true, 0), DrafterKind::Dspark);
        assert_eq!(DrafterKind::detect(true, 4), DrafterKind::Dspark);
        assert_eq!(DrafterKind::detect(false, 4), DrafterKind::Dflash2);
        assert_eq!(DrafterKind::detect(false, 0), DrafterKind::Dflash);
    }

    #[test]
    fn dspark_truncation_needs_a_confidence_head_only_above_zero() {
        assert!(confidence_available(DrafterKind::Dspark, 0.55, true));
        assert!(!confidence_available(DrafterKind::Dspark, 0.55, false));
        assert!(confidence_available(DrafterKind::Dspark, 0.0, false));
        assert!(confidence_available(DrafterKind::Dflash, 0.55, false));
        assert!(confidence_available(DrafterKind::Dflash2, 0.55, false));
        assert!(metadata_flag(None, true));
        assert!(!metadata_flag(Some("false"), true));
    }

    #[test]
    fn noise_blocks_start_reading_where_the_drafter_predicts() {
        assert_eq!(block_layout(DrafterKind::Dspark, true, false, 7), (7, 0));
        assert_eq!(block_layout(DrafterKind::Dspark, false, false, 6), (7, 1));
        assert_eq!(block_layout(DrafterKind::Dflash, true, false, 6), (7, 1));
        assert_eq!(block_layout(DrafterKind::Dflash, false, false, 6), (7, 1));
        assert_eq!(block_layout(DrafterKind::Dflash2, true, true, 6), (7, 1));
        assert_eq!(block_layout(DrafterKind::Dspark, true, true, 7), (7, 1));
        for steps in 1..=7 {
            let (len, first) = block_layout(DrafterKind::Dspark, true, false, steps);
            assert!(len <= 7);
            assert_eq!(len - first, steps);
            let (len, first) = block_layout(DrafterKind::Dflash, true, false, steps.min(6));
            assert!(len <= 7);
            assert_eq!(len - first, steps.min(6));
        }
    }

    fn read(
        dspark: bool,
        first: usize,
        len: usize,
        p_min: f32,
        confidences: &[f32],
        probs: &[f32],
    ) -> (Vec<i32>, Vec<i32>) {
        let mut confidence_rows = Vec::new();
        let drafted = read_drafts::<()>(
            dspark,
            first,
            len,
            p_min,
            |slot| {
                confidence_rows.push(slot);
                Ok(confidences[slot as usize])
            },
            |slot| Some((LlamaToken::new(100 + slot), probs[slot as usize])),
        )
        .expect("drafts");
        (
            drafted.into_iter().map(|token| token.0).collect(),
            confidence_rows,
        )
    }

    #[test]
    fn dspark_stops_at_the_first_unconfident_slot() {
        let confidences = [0.9, 0.8, 0.4, 0.95];
        let probs = [0.1, 0.1, 0.1, 0.1];
        assert_eq!(
            read(true, 0, 4, 0.5, &confidences, &probs),
            (vec![100, 101], vec![0, 1, 2])
        );
        assert_eq!(
            read(true, 1, 4, 0.5, &confidences, &probs),
            (vec![101], vec![1, 2])
        );
        assert_eq!(
            read(true, 0, 4, 0.0, &confidences, &probs),
            (vec![100, 101, 102, 103], vec![])
        );
    }

    #[test]
    fn dflash_stops_at_the_first_improbable_token() {
        let confidences = [0.0; 4];
        let probs = [0.9, 0.9, 0.3, 0.9];
        assert_eq!(
            read(false, 1, 4, 0.5, &confidences, &probs),
            (vec![101], vec![])
        );
        assert_eq!(
            read(false, 1, 4, 0.0, &confidences, &probs),
            (vec![101, 102, 103], vec![])
        );
    }

    #[test]
    fn a_failed_confidence_read_fails_the_draft() {
        let result = read_drafts(
            true,
            0,
            3,
            0.5,
            |_| Err("no row"),
            |slot| Some((LlamaToken::new(slot), 1.0)),
        );
        assert_eq!(result, Err("no row"));
    }

    fn verify(drafted: &[i32], samples: &[i32], eog: i32) -> (usize, LlamaToken, Vec<i32>) {
        let drafted: Vec<LlamaToken> = drafted.iter().copied().map(LlamaToken::new).collect();
        let mut rows = Vec::new();
        let (matched, extra) = verify_drafts(
            &drafted,
            |row| {
                rows.push(row);
                LlamaToken::new(samples[row as usize])
            },
            |token| token == LlamaToken::new(eog),
        );
        (matched, extra, rows)
    }

    #[test]
    fn verification_samples_each_row_once() {
        assert_eq!(
            verify(&[1, 2, 3], &[9, 2, 3, 4], -1),
            (0, LlamaToken::new(9), vec![0])
        );
        assert_eq!(
            verify(&[1, 2, 3], &[1, 7, 3, 4], -1),
            (1, LlamaToken::new(7), vec![0, 1])
        );
        assert_eq!(
            verify(&[1, 2, 3], &[1, 2, 3, 4], -1),
            (3, LlamaToken::new(4), vec![0, 1, 2, 3])
        );
        assert_eq!(
            verify(&[1, 2, 3], &[1, 2, 3, 4], 2),
            (2, LlamaToken::new(3), vec![0, 1, 2])
        );
    }

    #[test]
    fn the_dflash2_selector_walks_its_lattice_from_the_chosen_slot() {
        let top_k = 2;
        let first = [11.0, 12.0, 0.1, 3.0, 0.2, 0.1, 0.0, 0.0];
        let second = [21.0, 22.0, 5.0, 0.0, 0.0, 4.0, 0.0, 0.0];
        let third = [31.0, 32.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0];
        let rows: Vec<&[f32]> = vec![&first, &second, &third];
        assert_eq!(
            walk_selector_lattice(&rows, top_k, 0.0),
            vec![
                LlamaToken::new(12),
                LlamaToken::new(22),
                LlamaToken::new(31)
            ]
        );
        assert_eq!(
            walk_selector_lattice(&rows, top_k, 0.9),
            vec![LlamaToken::new(12), LlamaToken::new(22)]
        );
        assert_eq!(
            walk_selector_lattice(&rows, top_k, 0.99),
            Vec::<LlamaToken>::new()
        );
        let short: Vec<&[f32]> = vec![&first[..3]];
        assert!(walk_selector_lattice(&short, top_k, 0.0).is_empty());
    }

    #[test]
    fn metadata_flags_are_set_only_by_true() {
        assert!(!metadata_flag(None, false));
        assert!(metadata_flag(None, true));
        assert!(metadata_flag(Some("true"), false));
        assert!(!metadata_flag(Some("false"), true));
        assert!(!metadata_flag(Some("1"), true));
    }

    #[test]
    fn only_drafters_sharing_the_model_stem_are_ranked_by_name() {
        let ranked = rank_candidates(
            "qwen3-8b-q4_k_m",
            vec![
                "/m/aaa-dflash.gguf".into(),
                "/m/Qwen3-8B-DFlash.gguf".into(),
                "/m/qwen3-8b-q4_k_m-dflash.gguf".into(),
                "/m/dflash.gguf".into(),
            ],
        );
        assert_eq!(
            ranked,
            vec![
                "/m/Qwen3-8B-DFlash.gguf".to_string(),
                "/m/qwen3-8b-q4_k_m-dflash.gguf".to_string(),
            ]
        );
        assert!(shares_stem("model-dflash", "model"));
        assert!(shares_stem("model_dflash", "model-q4"));
        assert!(!shares_stem("other-dflash", "model"));
        assert!(!shares_stem("dflash", "model"));
        assert!(!shares_stem("-dflash-", "model"));
        assert!(shares_stem("qwen3.5-9b-dflash-q8_0", "qwen3.5-9b-q4_k_m"));
        assert!(shares_stem("dflash-qwen3.5-9b", "qwen3.5-9b-q4_k_m"));
        assert!(!shares_stem("qwen3-4b-dflash-q8_0", "qwen3.5-9b-q4_k_m"));
    }

    #[test]
    fn drafter_checks_reject_mismatched_widths_and_mask_tokens() {
        assert!(injection_width_matches(5120, 5120));
        assert!(!injection_width_matches(5120, 4096));
        assert!(!injection_width_matches(0, 0));
        assert!(mask_token_fits(248_319, 248_320, 248_320));
        assert!(!mask_token_fits(248_320, 248_320, 248_320));
        assert!(!mask_token_fits(-1, 248_320, 248_320));
        assert!(!mask_token_fits(248_300, 248_320, 248_256));
    }

    #[test]
    fn mrope_injection_rows_repeat_the_position_and_zero_the_last() {
        assert_eq!(
            mrope_extra_rows(&[7, 8]),
            [vec![7, 8], vec![7, 8], vec![0, 0]]
        );
    }

    fn write_gguf_with_tensors(path: &Path, keys: &[(&str, u32)], tensors: &[&str]) {
        let mut out = b"GGUF".to_vec();
        out.extend(3_u32.to_le_bytes());
        out.extend((tensors.len() as u64).to_le_bytes());
        out.extend((keys.len() as u64).to_le_bytes());
        for (key, value) in keys {
            out.extend((key.len() as u64).to_le_bytes());
            out.extend(key.as_bytes());
            out.extend(4_u32.to_le_bytes());
            out.extend(value.to_le_bytes());
        }
        for (index, name) in tensors.iter().enumerate() {
            out.extend((name.len() as u64).to_le_bytes());
            out.extend(name.as_bytes());
            out.extend(1_u32.to_le_bytes());
            out.extend(8_u64.to_le_bytes());
            out.extend(0_u32.to_le_bytes());
            out.extend((index as u64 * 32).to_le_bytes());
        }
        while out.len() % 32 != 0 {
            out.push(0);
        }
        out.extend(vec![0_u8; tensors.len() * 32]);
        std::fs::write(path, out).expect("gguf");
    }

    fn write_gguf(path: &Path, keys: &[(&str, u32)]) {
        write_gguf_with_tensors(path, keys, &[]);
    }

    #[test]
    fn drafters_are_known_by_architecture_and_need_a_stem_match() {
        let dir = std::env::temp_dir().join(format!("lettuce-dflash-arch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let model = dir.join("mira-8b.gguf");
        write_gguf(&model, &[("llama.block_count", 32)]);
        let mut out = b"GGUF".to_vec();
        out.extend(3_u32.to_le_bytes());
        out.extend(0_u64.to_le_bytes());
        out.extend(1_u64.to_le_bytes());
        let key = "general.architecture";
        out.extend((key.len() as u64).to_le_bytes());
        out.extend(key.as_bytes());
        out.extend(8_u32.to_le_bytes());
        out.extend(6_u64.to_le_bytes());
        out.extend(b"dflash");
        let by_arch = dir.join("mira-8b-dflash.gguf");
        std::fs::write(&by_arch, out).expect("gguf");
        write_gguf(&dir.join("dflash.gguf"), &[("dflash.block_size", 16)]);
        write_gguf(&dir.join("other-dflash.gguf"), &[("dflash.block_size", 16)]);
        let by_arch = by_arch.to_str().expect("path").to_string();
        assert!(model_is_dflash(&by_arch));
        let model_path = model.to_str().expect("path");
        assert_eq!(discover_external_dflash(model_path), Some(by_arch.clone()));
        std::fs::remove_file(&by_arch).expect("remove");
        assert_eq!(discover_external_dflash(model_path), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn drafters_are_found_by_their_gguf_key_and_name() {
        let dir = std::env::temp_dir().join(format!("lettuce-dflash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let model = dir.join("Qwen3-8B-Q4.gguf");
        write_gguf(&model, &[("qwen3.block_count", 36)]);
        write_gguf(&dir.join("aaa-dflash.gguf"), &[("dflash.block_size", 16)]);
        write_gguf(
            &dir.join("qwen3-8b-dflash.gguf"),
            &[("dflash.block_size", 16)],
        );
        write_gguf(&dir.join("qwen3-8b-q4-dflash-fake.gguf"), &[("x.y", 1)]);
        write_gguf(
            &dir.join("qwen3-drafter.gguf"),
            &[("dflash.block_size", 16)],
        );
        let model_path = model.to_str().expect("path");
        assert!(model_is_dflash(
            dir.join("qwen3-8b-dflash.gguf").to_str().expect("path")
        ));
        assert!(!model_is_dflash(model_path));
        assert!(!model_is_dflash(
            dir.join("missing.gguf").to_str().expect("path")
        ));
        let found = discover_external_dflash(model_path).expect("drafter");
        assert!(found.ends_with("qwen3-8b-dflash.gguf"), "{found}");
        let explicit = dir.join("qwen3-drafter.gguf");
        let explicit = explicit.to_str().expect("path");
        assert_eq!(
            resolve_drafter(Some(explicit), model_path).as_deref(),
            Some(explicit)
        );
        assert_eq!(
            resolve_drafter(Some(model_path), model_path),
            Some(found.clone())
        );
        let dspark = dir.join("qwen3-dspark.gguf");
        write_gguf_with_tensors(
            &dspark,
            &[("dflash.block_size", 16)],
            &["markov_w1.weight", "output.weight"],
        );
        let dspark = dspark.to_str().expect("path");
        assert!(model_is_dflash(dspark));
        assert!(model_is_dspark(dspark));
        assert!(!model_is_dspark(&found));
        assert_eq!(
            resolve_drafter(Some(dspark), model_path).as_deref(),
            Some(dspark)
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
