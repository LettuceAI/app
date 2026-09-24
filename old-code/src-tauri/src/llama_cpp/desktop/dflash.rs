use super::*;
use llama_cpp_2::context::params::LlamaContextType;
use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::gguf::GgufContext;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::token::LlamaToken;
use std::collections::VecDeque;

pub(super) const DFLASH_DRAFT_DEFAULT: u32 = 4;
pub(super) const DFLASH_DRAFT_MAX: u32 = 15;
pub(super) const DFLASH_P_MIN_DEFAULT: f32 = 0.55;
const DFLASH_BLOCK_SIZE_KEY: &str = "dflash.block_size";
const DFLASH_BLOCK_SIZE_FALLBACK: usize = 16;
const DFLASH_ADAPT_WINDOW_ROUNDS: u32 = 8;

pub(super) struct DflashRuntime<'m> {
    pub(super) draft: LlamaContext<'m>,
    pub(super) primed: bool,
    pub(super) draft_n: usize,
    pub(super) draft_n_max: usize,
    pub(super) block_size: usize,
    pub(super) p_min: f32,
    pub(super) adaptation_count: u32,
    adaptive_rounds: u32,
    adaptive_drafted: u64,
    adaptive_matched: u64,
    pub(super) max_batch: usize,
    pub(super) n_embd_tgt: usize,
    pub(super) n_embd_dec: usize,
    pub(super) target_layers: Vec<u32>,
    pub(super) mask_token: LlamaToken,
    features: Vec<f32>,
    pub(super) pending: VecDeque<LlamaToken>,
    pub(super) rounds: u64,
    pub(super) drafted: u64,
    pub(super) accepted: u64,
}

pub(crate) fn model_is_dflash(model_path: &str) -> bool {
    let Some(gguf) = GgufContext::from_file(Path::new(model_path)) else {
        return false;
    };
    gguf.find_key(DFLASH_BLOCK_SIZE_KEY) >= 0
}

pub(super) fn discover_external_dflash(model_path: &str) -> Option<String> {
    let path = Path::new(model_path);
    let model_stem = path.file_stem()?.to_str()?.to_lowercase();
    let dir = path.parent()?;
    let entries = std::fs::read_dir(dir).ok()?;

    let mut candidates: Vec<String> = entries
        .filter_map(|entry| entry.ok())
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
            if !name.contains("dflash") {
                return None;
            }
            if !model_is_dflash(&candidate_path) {
                return None;
            }
            Some(candidate_path)
        })
        .collect();

    candidates.sort_by_key(|candidate| {
        let name = Path::new(candidate)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(str::to_lowercase)
            .unwrap_or_default();
        let shares_stem = name.contains(&model_stem)
            || model_stem.contains(&name.replace("-dflash", "").replace("dflash", ""));
        (!shares_stem, name)
    });

    candidates.into_iter().next()
}

pub(super) fn block_size_for(model: &LlamaModel) -> usize {
    model
        .meta_val_str(DFLASH_BLOCK_SIZE_KEY)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|size| *size > 1)
        .unwrap_or(DFLASH_BLOCK_SIZE_FALLBACK)
}

pub(super) fn create_runtime<'m>(
    target_model: &LlamaModel,
    draft_model: &'m LlamaModel,
    target_ctx: &LlamaContext<'_>,
    backend: &LlamaBackend,
    draft_params: LlamaContextParams,
    draft_n: usize,
    p_min: f32,
) -> Result<DflashRuntime<'m>, String> {
    let target_layers: Vec<u32> = draft_model
        .target_layer_ids()
        .iter()
        .map(|layer| u32::try_from(*layer))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "DFlash target layer id is negative".to_string())?;
    if target_layers.is_empty() {
        return Err("DFlash draft model exposes no target_layer_ids".to_string());
    }

    let n_embd_tgt = usize::try_from(target_model.n_embd())
        .map_err(|_| "target n_embd does not fit into usize".to_string())?;
    let n_embd_dec = usize::try_from(draft_model.n_embd())
        .map_err(|_| "DFlash draft n_embd does not fit into usize".to_string())?;

    let n_layers_target = target_model.n_layer() as usize;
    if let Some(layer) = target_layers
        .iter()
        .find(|layer| **layer as usize >= n_layers_target)
    {
        return Err(format!(
            "DFlash draft model requests target layer {layer}, but the target model has {n_layers_target} layers"
        ));
    }

    let block_size = block_size_for(draft_model);
    let draft_n = draft_n.max(1).min(block_size.saturating_sub(1).max(1));

    let max_batch = (draft_n + 1).max(2) as u32;
    let params = draft_params
        .with_ctx_type(LlamaContextType::Mtp)
        .with_ctx_other(target_ctx)
        .with_n_batch(max_batch)
        .with_n_ubatch(max_batch)
        .with_n_outputs_max(max_batch);
    let draft = draft_model
        .new_context(backend, params)
        .map_err(|e| format!("failed to create DFlash draft context: {e}"))?;

    Ok(DflashRuntime {
        draft,
        primed: false,
        draft_n,
        draft_n_max: draft_n,
        block_size,
        p_min: p_min.clamp(0.0, 1.0),
        adaptation_count: 0,
        adaptive_rounds: 0,
        adaptive_drafted: 0,
        adaptive_matched: 0,
        max_batch: max_batch as usize,
        n_embd_tgt,
        n_embd_dec,
        target_layers,
        mask_token: draft_model.token_mask(),
        features: Vec::new(),
        pending: VecDeque::new(),
        rounds: 0,
        drafted: 0,
        accepted: 0,
    })
}

pub(super) fn enable_feature_extraction(
    target: &mut LlamaContext<'_>,
    rt: &mut DflashRuntime<'_>,
) -> Result<(), String> {
    for layer in &rt.target_layers {
        target.set_embeddings_layer_inp(*layer, true);
    }
    rt.draft
        .set_embeddings_nextn(true, true)
        .map_err(|e| format!("failed to enable nextn embeddings on DFlash draft context: {e}"))?;
    rt.draft.set_causal_attn(false);
    Ok(())
}

pub(super) fn reset_for_prompt_reuse(
    rt: &mut DflashRuntime<'_>,
    draft_clear_from: u32,
) -> Result<(), String> {
    let cleared = rt
        .draft
        .clear_kv_cache_seq(Some(0), Some(draft_clear_from), None)
        .map_err(|e| format!("failed to rewind DFlash prompt cache: {e}"))?;
    if !cleared {
        return Err(format!(
            "DFlash prompt cache rewind failed at position {draft_clear_from}"
        ));
    }
    rt.primed = false;
    rt.pending.clear();
    rt.rounds = 0;
    rt.drafted = 0;
    rt.accepted = 0;
    rt.adaptation_count = 0;
    rt.adaptive_rounds = 0;
    rt.adaptive_drafted = 0;
    rt.adaptive_matched = 0;
    Ok(())
}

pub(super) fn truncate_for_prompt_cache(
    target: &mut LlamaContext<'_>,
    rt: &mut DflashRuntime<'_>,
    token_count: u32,
) -> Result<(), String> {
    let keep = token_count;
    let target_trimmed = target
        .clear_kv_cache_seq(Some(0), Some(keep), None)
        .map_err(|e| format!("failed to trim target prompt cache: {e}"))?;
    if !target_trimmed {
        return Err(format!(
            "target prompt cache trim failed at position {token_count}"
        ));
    }
    let draft_trimmed = rt
        .draft
        .clear_kv_cache_seq(Some(0), Some(keep), None)
        .map_err(|e| format!("failed to trim DFlash prompt cache: {e}"))?;
    if !draft_trimmed {
        return Err(format!(
            "DFlash prompt cache trim failed at position {token_count}"
        ));
    }
    rt.pending.clear();
    Ok(())
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

fn record_adaptive_round(rt: &mut DflashRuntime<'_>, drafted: usize, matched: usize) {
    rt.adaptive_rounds += 1;
    rt.adaptive_drafted += drafted as u64;
    rt.adaptive_matched += matched as u64;
    if rt.adaptive_rounds < DFLASH_ADAPT_WINDOW_ROUNDS {
        return;
    }
    let next = adjusted_draft_length(
        rt.draft_n,
        rt.draft_n_max,
        rt.adaptive_drafted,
        rt.adaptive_matched,
    );
    if next != rt.draft_n {
        rt.draft_n = next;
        rt.adaptation_count += 1;
    }
    rt.adaptive_rounds = 0;
    rt.adaptive_drafted = 0;
    rt.adaptive_matched = 0;
}

fn gather_features(
    rt: &mut DflashRuntime<'_>,
    target: &LlamaContext<'_>,
    row_offset: usize,
    n_rows: usize,
) -> Result<(), String> {
    let n_embd_enc = rt.target_layers.len() * rt.n_embd_tgt;
    rt.features.clear();
    rt.features.resize(n_rows * n_embd_enc, 0.0);

    for (k, layer) in rt.target_layers.clone().iter().enumerate() {
        let rows = target
            .embeddings_layer_inp(*layer, row_offset + n_rows)
            .map_err(|e| format!("failed to read target layer {layer} embeddings: {e}"))?;
        for row in 0..n_rows {
            let src_start = (row_offset + row) * rt.n_embd_tgt;
            let src = rows
                .get(src_start..src_start + rt.n_embd_tgt)
                .ok_or_else(|| format!("target layer {layer} embeddings are truncated"))?;
            let dst_start = row * n_embd_enc + k * rt.n_embd_tgt;
            rt.features[dst_start..dst_start + rt.n_embd_tgt].copy_from_slice(src);
        }
    }
    Ok(())
}

pub(super) fn prefill_draft_chunk(
    rt: &mut DflashRuntime<'_>,
    target: &LlamaContext<'_>,
    chunk_tokens: &[LlamaToken],
    chunk_start_pos: i32,
) -> Result<(), String> {
    if chunk_tokens.is_empty() {
        return Ok(());
    }

    for (sub_index, subchunk) in chunk_tokens.chunks(rt.max_batch).enumerate() {
        let row_offset = sub_index * rt.max_batch;
        let sub_start = i32::try_from(row_offset)
            .ok()
            .and_then(|offset| chunk_start_pos.checked_add(offset))
            .ok_or_else(|| "DFlash draft prefill position overflowed i32".to_string())?;
        encode_and_inject(rt, target, subchunk.len(), row_offset, sub_start)?;
    }
    Ok(())
}

fn encode_and_inject(
    rt: &mut DflashRuntime<'_>,
    target: &LlamaContext<'_>,
    n_rows: usize,
    row_offset: usize,
    start_pos: i32,
) -> Result<(), String> {
    gather_features(rt, target, row_offset, n_rows)?;

    let n_embd_enc = rt.target_layers.len() * rt.n_embd_tgt;
    let mut encode_batch = LlamaBatch::new_with_embeddings(n_rows, n_embd_enc, 1);
    for row in 0..n_rows {
        let start = row * n_embd_enc;
        let features = rt.features[start..start + n_embd_enc].to_vec();
        encode_batch
            .add_with_embedding(rt.mask_token, &features, start_pos + row as i32, &[0], true)
            .map_err(|e| format!("failed to build DFlash encoder batch: {e}"))?;
    }
    rt.draft
        .encode(&mut encode_batch)
        .map_err(|e| format!("DFlash encoder pass failed: {e}"))?;

    let mut inject_batch = LlamaBatch::new_with_embeddings(n_rows, rt.n_embd_dec, 1);
    for row in 0..n_rows {
        let encoded = rt
            .draft
            .embeddings_nextn_ith(row as i32)
            .map_err(|e| format!("failed to read DFlash encoder output: {e}"))?
            .to_vec();
        inject_batch
            .add_with_embedding(rt.mask_token, &encoded, start_pos + row as i32, &[0], false)
            .map_err(|e| format!("failed to build DFlash injection batch: {e}"))?;
    }
    rt.draft
        .decode(&mut inject_batch)
        .map_err(|e| format!("DFlash cache injection failed: {e}"))?;
    Ok(())
}

pub(super) fn dflash_round(
    target: &mut LlamaContext<'_>,
    rt: &mut DflashRuntime<'_>,
    sampler: &mut LlamaSampler,
    model: &LlamaModel,
    pos: i32,
    max_pos: i32,
) -> Result<Vec<LlamaToken>, String> {
    rt.rounds += 1;

    let first = sampler.sample(target, -1);
    if !rt.primed {
        rt.primed = true;
        rt.accepted += 1;
        advance_after_accept(target, rt, pos, 0, first)?;
        return Ok(vec![first]);
    }

    let budget = (max_pos - pos - 1).max(0) as usize;
    let steps = rt.draft_n.min(budget);

    let drafted = if steps == 0 || model.is_eog_token(first) {
        Vec::new()
    } else {
        draft_block(rt, first, pos, steps)?
    };
    rt.drafted += drafted.len() as u64;

    if drafted.is_empty() {
        rt.accepted += 1;
        advance_after_accept(target, rt, pos, 0, first)?;
        record_adaptive_round(rt, 0, 0);
        return Ok(vec![first]);
    }

    let mut batch = LlamaBatch::new(drafted.len() + 1, 1);
    batch
        .add(first, pos, &[0], true)
        .map_err(|e| format!("failed to build DFlash verification batch: {e}"))?;
    for (i, token) in drafted.iter().enumerate() {
        batch
            .add(*token, pos + 1 + i as i32, &[0], true)
            .map_err(|e| format!("failed to build DFlash verification batch: {e}"))?;
    }
    target
        .decode(&mut batch)
        .map_err(|e| format!("DFlash verification decode failed: {e}"))?;

    let mut matched = 0usize;
    let mut extra = first;
    for i in 0..drafted.len() {
        let sampled = sampler.sample(target, i as i32);
        if sampled != drafted[i] {
            extra = sampled;
            break;
        }
        matched = i + 1;
        extra = sampler.sample(target, (i + 1) as i32);
        if model.is_eog_token(drafted[i]) {
            break;
        }
    }

    let mut accepted: Vec<LlamaToken> = Vec::with_capacity(matched + 2);
    accepted.push(first);
    accepted.extend_from_slice(&drafted[..matched]);
    accepted.push(extra);

    advance_after_accept(target, rt, pos, matched + 1, extra)?;
    rt.accepted += accepted.len() as u64;
    record_adaptive_round(rt, drafted.len(), matched);

    Ok(accepted)
}

fn draft_block(
    rt: &mut DflashRuntime<'_>,
    id_last: LlamaToken,
    pos: i32,
    steps: usize,
) -> Result<Vec<LlamaToken>, String> {
    let n_block = steps + 1;
    let mut batch = LlamaBatch::new(n_block, 1);
    for i in 0..n_block {
        let token = if i == 0 { id_last } else { rt.mask_token };
        batch
            .add(token, pos + i as i32, &[0], true)
            .map_err(|e| format!("failed to build DFlash noise batch: {e}"))?;
    }
    rt.draft
        .decode(&mut batch)
        .map_err(|e| format!("DFlash draft decode failed: {e}"))?;

    let mut drafted = Vec::with_capacity(steps);
    for i in 1..n_block {
        let logits = rt.draft.get_logits_ith(i as i32);
        let (token, prob) = greedy_token_with_prob(logits);
        if prob < rt.p_min {
            break;
        }
        drafted.push(token);
    }

    let rollback = u32::try_from(pos)
        .map_err(|_| "DFlash draft rollback position does not fit into u32".to_string())?;
    let rolled_back = rt
        .draft
        .clear_kv_cache_seq(Some(0), Some(rollback), None)
        .map_err(|e| format!("failed to roll back DFlash draft KV cache: {e}"))?;
    if !rolled_back {
        return Err(format!(
            "DFlash draft KV rollback failed at position {rollback}"
        ));
    }

    Ok(drafted)
}

fn advance_after_accept(
    target: &mut LlamaContext<'_>,
    rt: &mut DflashRuntime<'_>,
    pos: i32,
    accepted: usize,
    extra: LlamaToken,
) -> Result<(), String> {
    let extra_pos = pos + accepted as i32;
    let rollback_pos = u32::try_from(extra_pos)
        .map_err(|_| "DFlash rollback position does not fit into u32".to_string())?;

    let target_rolled_back = target
        .clear_kv_cache_seq(Some(0), Some(rollback_pos), None)
        .map_err(|e| format!("failed to roll back target KV cache: {e}"))?;
    if !target_rolled_back {
        return Err(format!(
            "target KV rollback failed at position {rollback_pos}"
        ));
    }
    let draft_rolled_back = rt
        .draft
        .clear_kv_cache_seq(Some(0), Some(rollback_pos), None)
        .map_err(|e| format!("failed to roll back DFlash draft KV cache: {e}"))?;
    if !draft_rolled_back {
        return Err(format!(
            "DFlash draft KV rollback failed at position {rollback_pos}"
        ));
    }

    let mut target_batch = LlamaBatch::new(1, 1);
    target_batch
        .add(extra, extra_pos, &[0], true)
        .map_err(|e| format!("failed to build DFlash target advance batch: {e}"))?;
    target
        .decode(&mut target_batch)
        .map_err(|e| format!("failed to advance target with accepted token: {e}"))?;

    encode_and_inject(rt, target, 1, 0, extra_pos)?;
    Ok(())
}

fn greedy_token_with_prob(logits: &[f32]) -> (LlamaToken, f32) {
    let mut best = 0usize;
    let mut max = f32::NEG_INFINITY;
    for (i, &logit) in logits.iter().enumerate() {
        if logit > max {
            max = logit;
            best = i;
        }
    }
    let sum: f32 = logits.iter().map(|&logit| (logit - max).exp()).sum();
    let prob = if sum > 0.0 { 1.0 / sum } else { 0.0 };
    (LlamaToken::new(best as i32), prob)
}

#[cfg(test)]
mod tests {
    use super::adjusted_draft_length;

    #[test]
    fn adaptive_draft_length_halves_low_acceptance() {
        assert_eq!(adjusted_draft_length(8, 8, 16, 7), 4);
        assert_eq!(adjusted_draft_length(1, 8, 16, 0), 1);
    }

    #[test]
    fn adaptive_draft_length_grows_high_acceptance_to_configured_limit() {
        assert_eq!(adjusted_draft_length(3, 6, 10, 8), 4);
        assert_eq!(adjusted_draft_length(6, 6, 10, 10), 6);
    }

    #[test]
    fn adaptive_draft_length_holds_middle_acceptance() {
        assert_eq!(adjusted_draft_length(4, 8, 10, 6), 4);
    }
}
