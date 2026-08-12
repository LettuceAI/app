use super::*;
use llama_cpp_2::context::params::LlamaContextType;
use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::gguf::GgufContext;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::token::LlamaToken;
use std::collections::VecDeque;

pub(super) const MTP_DRAFT_DEFAULT: u32 = 4;
pub(super) const MTP_DRAFT_MAX: u32 = 8;
const MTP_DRAFT_TOP_K: usize = 10;
const MTP_DRAFT_P_MIN: f32 = 0.75;
const MTP_ADAPT_WINDOW_ROUNDS: u32 = 8;

pub(super) struct MtpRuntime<'m> {
    pub(super) draft: LlamaContext<'m>,
    pub(super) shared: bool,
    pub(super) primed: bool,
    pub(super) draft_n: usize,
    pub(super) draft_n_max: usize,
    pub(super) adaptation_count: u32,
    adaptive_rounds: u32,
    adaptive_drafted: u64,
    adaptive_matched: u64,
    pub(super) max_batch: usize,
    pub(super) n_embd: usize,
    pub(super) carry_hidden: Vec<f32>,
    pub(super) h_last: Vec<f32>,
    pub(super) last_token: LlamaToken,
    pub(super) draft_last_row: i32,
    pub(super) pending: VecDeque<LlamaToken>,
    pub(super) rounds: u64,
    pub(super) drafted: u64,
    pub(super) accepted: u64,
}

pub(super) fn model_has_mtp(model_path: &str) -> bool {
    let Some(gguf) = GgufContext::from_file(Path::new(model_path)) else {
        return false;
    };

    let arch_key = gguf.find_key("general.architecture");
    if arch_key < 0 {
        return false;
    }
    let Some(arch) = gguf.val_str(arch_key) else {
        return false;
    };

    let nextn_idx = gguf.find_key(&format!("{arch}.nextn_predict_layers"));
    nextn_idx >= 0 && gguf.val_u32(nextn_idx) > 0
}

pub(super) fn discover_external_mtp(model_path: &str) -> Option<String> {
    let path = Path::new(model_path);
    let model_stem = path.file_name()?.to_str()?.to_lowercase();
    let dir = path.parent()?;
    let entries = std::fs::read_dir(dir).ok()?;

    let mut candidates: Vec<String> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_str()?.to_string();
            let lower = name.to_lowercase();
            if !lower.ends_with(".gguf") {
                return None;
            }
            let stem = lower
                .strip_prefix("mtp-")
                .or_else(|| {
                    lower
                        .strip_suffix("-mtp.gguf")
                        .map(|_| &lower[..lower.len() - 9])
                })?
                .trim_end_matches(".gguf")
                .to_string();
            if stem.is_empty() || !model_stem.contains(&stem) {
                return None;
            }
            Some(entry.path().to_string_lossy().to_string())
        })
        .collect();

    candidates.sort();
    candidates.into_iter().next()
}

pub(super) fn create_runtime<'m>(
    target_model: &LlamaModel,
    draft_model: &'m LlamaModel,
    target_ctx: &LlamaContext<'_>,
    backend: &LlamaBackend,
    draft_params: LlamaContextParams,
    draft_n: usize,
) -> Result<MtpRuntime<'m>, String> {
    let shared = if draft_model.n_embd() == target_model.n_embd() {
        false
    } else if draft_model.n_embd_out() == target_model.n_embd() {
        true
    } else {
        return Err(format!(
            "MTP draft model widths (n_embd {}, n_embd_out {}) do not match target model width {}",
            draft_model.n_embd(),
            draft_model.n_embd_out(),
            target_model.n_embd()
        ));
    };

    let max_batch = draft_n.max(1) as u32 + 1;
    let mut params = draft_params
        .with_ctx_type(LlamaContextType::Mtp)
        .with_ctx_other(target_ctx)
        .with_n_batch(max_batch)
        .with_n_ubatch(max_batch)
        .with_n_outputs_max(max_batch);
    if shared {
        params = params.with_n_rs_seq(0);
    }
    let draft = draft_model
        .new_context(backend, params)
        .map_err(|e| format!("failed to create MTP draft context: {e}"))?;
    let n_embd = usize::try_from(target_model.n_embd())
        .map_err(|_| "model n_embd does not fit into usize".to_string())?;

    let draft_n = draft_n.max(1);
    Ok(MtpRuntime {
        draft,
        shared,
        primed: false,
        draft_n,
        draft_n_max: draft_n,
        adaptation_count: 0,
        adaptive_rounds: 0,
        adaptive_drafted: 0,
        adaptive_matched: 0,
        max_batch: max_batch as usize,
        n_embd,
        carry_hidden: vec![0.0; n_embd],
        h_last: vec![0.0; n_embd],
        last_token: LlamaToken::new(0),
        draft_last_row: 0,
        pending: VecDeque::new(),
        rounds: 0,
        drafted: 0,
        accepted: 0,
    })
}

pub(super) fn enable_nextn_embeddings(
    target: &mut LlamaContext<'_>,
    rt: &mut MtpRuntime<'_>,
) -> Result<(), String> {
    target
        .set_embeddings_nextn(true, false)
        .map_err(|e| format!("failed to enable nextn embeddings on target context: {e}"))?;
    rt.draft
        .set_embeddings_nextn(true, rt.shared)
        .map_err(|e| format!("failed to enable nextn embeddings on MTP draft context: {e}"))?;
    Ok(())
}

pub(super) fn reset_for_prompt_reuse(
    rt: &mut MtpRuntime<'_>,
    draft_clear_from: u32,
) -> Result<(), String> {
    let cleared = if rt.shared {
        rt.draft.clear_kv_cache();
        true
    } else {
        rt.draft
            .clear_kv_cache_seq(Some(0), Some(draft_clear_from), None)
            .map_err(|e| format!("failed to rewind MTP prompt cache: {e}"))?
    };
    if !cleared {
        return Err(format!(
            "MTP prompt cache rewind failed at position {draft_clear_from}"
        ));
    }
    rt.primed = false;
    rt.carry_hidden.fill(0.0);
    rt.h_last.fill(0.0);
    rt.last_token = LlamaToken::new(0);
    rt.draft_last_row = 0;
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

fn adjusted_draft_length(current: usize, maximum: usize, drafted: u64, matched: u64) -> usize {
    if drafted == 0 || matched.saturating_mul(2) < drafted {
        return (current / 2).max(1);
    }
    if matched.saturating_mul(5) >= drafted.saturating_mul(4) {
        return current.saturating_add(1).min(maximum);
    }
    current
}

fn record_adaptive_round(rt: &mut MtpRuntime<'_>, drafted: usize, matched: usize) {
    rt.adaptive_rounds = rt.adaptive_rounds.saturating_add(1);
    rt.adaptive_drafted = rt.adaptive_drafted.saturating_add(drafted as u64);
    rt.adaptive_matched = rt.adaptive_matched.saturating_add(matched as u64);
    if rt.adaptive_rounds < MTP_ADAPT_WINDOW_ROUNDS {
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
        rt.adaptation_count = rt.adaptation_count.saturating_add(1);
    }
    rt.adaptive_rounds = 0;
    rt.adaptive_drafted = 0;
    rt.adaptive_matched = 0;
}

pub(super) fn set_prefill_carry_from_target(
    rt: &mut MtpRuntime<'_>,
    target: &LlamaContext<'_>,
    row: i32,
) -> Result<(), String> {
    rt.carry_hidden = target
        .embeddings_nextn_ith(row)
        .map_err(|e| format!("failed to restore MTP prompt-cache carry: {e}"))?
        .to_vec();
    Ok(())
}

pub(super) fn truncate_for_prompt_cache(
    target: &mut LlamaContext<'_>,
    rt: &mut MtpRuntime<'_>,
    token_count: u32,
) -> Result<(), String> {
    let target_cleared = target
        .clear_kv_cache_seq(Some(0), Some(token_count), None)
        .map_err(|e| format!("failed to trim target prompt cache: {e}"))?;
    if !target_cleared {
        return Err(format!(
            "target prompt cache trim failed at position {token_count}"
        ));
    }
    if !rt.shared {
        let draft_cleared = rt
            .draft
            .clear_kv_cache_seq(Some(0), Some(token_count), None)
            .map_err(|e| format!("failed to trim MTP prompt cache: {e}"))?;
        if !draft_cleared {
            return Err(format!(
                "MTP prompt cache trim failed at position {token_count}"
            ));
        }
    }
    rt.pending.clear();
    Ok(())
}

pub(super) fn prefill_draft_chunk(
    rt: &mut MtpRuntime<'_>,
    target: &LlamaContext<'_>,
    chunk_tokens: &[LlamaToken],
    chunk_start_pos: i32,
    is_final_chunk: bool,
) -> Result<(), String> {
    if !rt.shared && chunk_tokens.len() > rt.max_batch {
        for (sub_index, subchunk) in chunk_tokens.chunks(rt.max_batch).enumerate() {
            let sub_start = sub_index
                .checked_mul(rt.max_batch)
                .and_then(|offset| i32::try_from(offset).ok())
                .and_then(|offset| chunk_start_pos.checked_add(offset))
                .ok_or_else(|| "MTP draft prefill position overflowed i32".to_string())?;
            let sub_end = (sub_index + 1).saturating_mul(rt.max_batch);
            let sub_is_final = is_final_chunk && sub_end >= chunk_tokens.len();
            let row_offset = (sub_index * rt.max_batch) as i32;
            prefill_draft_chunk_inner(rt, target, subchunk, sub_start, sub_is_final, row_offset)?;
        }
        return Ok(());
    }

    prefill_draft_chunk_inner(rt, target, chunk_tokens, chunk_start_pos, is_final_chunk, 0)
}

fn prefill_draft_chunk_inner(
    rt: &mut MtpRuntime<'_>,
    target: &LlamaContext<'_>,
    chunk_tokens: &[LlamaToken],
    chunk_start_pos: i32,
    is_final_chunk: bool,
    target_row_offset: i32,
) -> Result<(), String> {
    if chunk_tokens.is_empty() {
        return Ok(());
    }

    if rt.shared {
        rt.h_last = target
            .embeddings_nextn_ith(chunk_tokens.len() as i32 - 1)
            .map_err(|e| format!("failed to read target nextn embeddings: {e}"))?
            .to_vec();
        return Ok(());
    }

    let zero = vec![0.0_f32; rt.n_embd];
    let mut batch = LlamaBatch::new_with_embeddings(chunk_tokens.len(), rt.n_embd, 1);

    for (i, token) in chunk_tokens.iter().enumerate() {
        let global_pos = chunk_start_pos + i as i32;
        let hidden: &[f32] = if global_pos == 0 {
            &zero
        } else if i == 0 {
            &rt.carry_hidden
        } else {
            target
                .embeddings_nextn_ith(target_row_offset + i as i32 - 1)
                .map_err(|e| format!("failed to read target nextn embeddings: {e}"))?
        };
        let logits = is_final_chunk && i + 1 == chunk_tokens.len();
        batch
            .add_with_embedding(*token, hidden, global_pos, &[0], logits)
            .map_err(|e| format!("failed to build MTP draft prefill batch: {e}"))?;
    }

    rt.draft
        .decode(&mut batch)
        .map_err(|e| format!("MTP draft prefill decode failed: {e}"))?;

    rt.carry_hidden = target
        .embeddings_nextn_ith(target_row_offset + chunk_tokens.len() as i32 - 1)
        .map_err(|e| format!("failed to read target nextn embeddings: {e}"))?
        .to_vec();
    rt.draft_last_row = chunk_tokens.len() as i32 - 1;
    Ok(())
}

pub(super) fn mtp_round(
    target: &mut LlamaContext<'_>,
    rt: &mut MtpRuntime<'_>,
    sampler: &mut LlamaSampler,
    model: &LlamaModel,
    pos: i32,
    max_pos: i32,
) -> Result<Vec<LlamaToken>, String> {
    if rt.shared {
        return mtp_round_shared(target, rt, sampler, model, pos, max_pos);
    }

    rt.rounds += 1;
    let prefix_hidden = rt.carry_hidden.clone();
    let budget = (max_pos - pos - 1).max(0) as usize;
    let steps = rt.draft_n.min(budget);

    let mut drafted: Vec<LlamaToken> = Vec::with_capacity(steps);
    if steps > 0 {
        let mut h_prev = rt
            .draft
            .embeddings_nextn_ith(rt.draft_last_row)
            .map_err(|e| format!("failed to read MTP draft nextn embeddings: {e}"))?
            .to_vec();

        for step in 0..steps {
            let (token, prob) = greedy_token_with_prob(rt.draft.get_logits());
            if prob < MTP_DRAFT_P_MIN {
                break;
            }
            drafted.push(token);
            if model.is_eog_token(token) {
                break;
            }

            let mut batch = LlamaBatch::new_with_embeddings(1, rt.n_embd, 1);
            batch
                .add_with_embedding(token, &h_prev, pos + step as i32, &[0], true)
                .map_err(|e| format!("failed to build MTP draft step batch: {e}"))?;
            rt.draft
                .decode(&mut batch)
                .map_err(|e| format!("MTP draft step decode failed: {e}"))?;
            rt.draft_last_row = 0;
            h_prev = rt
                .draft
                .embeddings_nextn_ith(0)
                .map_err(|e| format!("failed to read MTP draft nextn embeddings: {e}"))?
                .to_vec();
        }
    }
    rt.drafted += drafted.len() as u64;

    let first = sampler.sample(target, -1);

    if drafted.first() != Some(&first) {
        rollback_and_advance(target, rt, pos, 0, first, &prefix_hidden)?;
        rt.accepted += 1;
        record_adaptive_round(rt, drafted.len(), 0);
        return Ok(vec![first]);
    }

    let mut batch = LlamaBatch::new(drafted.len(), 1);
    for (i, token) in drafted.iter().enumerate() {
        batch
            .add(*token, pos + i as i32, &[0], true)
            .map_err(|e| format!("failed to build MTP verification batch: {e}"))?;
    }
    target
        .decode(&mut batch)
        .map_err(|e| format!("MTP verification decode failed: {e}"))?;

    let mut matched = drafted.len();
    let mut extra = first;
    for i in 0..drafted.len() {
        let sampled = sampler.sample(target, i as i32);
        if i + 1 == drafted.len() || sampled != drafted[i + 1] {
            matched = i + 1;
            extra = sampled;
            break;
        }
    }

    let mut accepted: Vec<LlamaToken> = drafted[..matched].to_vec();
    accepted.push(extra);

    let extra_hidden = if matched == 0 {
        prefix_hidden
    } else {
        target
            .embeddings_nextn_ith(matched as i32 - 1)
            .map_err(|e| format!("failed to read target nextn embeddings: {e}"))?
            .to_vec()
    };
    rollback_and_advance(target, rt, pos, matched, extra, &extra_hidden)?;
    rt.accepted += accepted.len() as u64;
    record_adaptive_round(rt, drafted.len(), matched);

    Ok(accepted)
}

fn mtp_round_shared(
    target: &mut LlamaContext<'_>,
    rt: &mut MtpRuntime<'_>,
    sampler: &mut LlamaSampler,
    model: &LlamaModel,
    pos: i32,
    max_pos: i32,
) -> Result<Vec<LlamaToken>, String> {
    rt.rounds += 1;

    if !rt.primed {
        rt.primed = true;
        let first = sampler.sample(target, -1);
        rt.carry_hidden = rt.h_last.clone();
        rt.last_token = first;
        rt.accepted += 1;
        return Ok(vec![first]);
    }

    let steps = rt.draft_n.min((max_pos - pos).max(0) as usize);

    let mut drafted: Vec<LlamaToken> = Vec::with_capacity(steps);
    let mut input = rt.last_token;
    let mut h_prev = rt.carry_hidden.clone();
    for _ in 0..steps {
        let mut batch = LlamaBatch::new_with_embeddings(1, rt.n_embd, 1);
        batch
            .add_with_embedding(input, &h_prev, pos - 1, &[0], true)
            .map_err(|e| format!("failed to build MTP draft step batch: {e}"))?;
        rt.draft
            .decode(&mut batch)
            .map_err(|e| format!("MTP draft step decode failed: {e}"))?;

        let (token, prob) = greedy_token_with_prob(rt.draft.get_logits());
        if prob < MTP_DRAFT_P_MIN {
            break;
        }
        drafted.push(token);
        if model.is_eog_token(token) {
            break;
        }
        h_prev = rt
            .draft
            .embeddings_nextn_ith(0)
            .map_err(|e| format!("failed to read MTP draft nextn embeddings: {e}"))?
            .to_vec();
        input = token;
    }
    rt.drafted += drafted.len() as u64;

    let mut batch = LlamaBatch::new(drafted.len() + 1, 1);
    batch
        .add(rt.last_token, pos - 1, &[0], true)
        .map_err(|e| format!("failed to build MTP verification batch: {e}"))?;
    for (i, token) in drafted.iter().enumerate() {
        batch
            .add(*token, pos + i as i32, &[0], true)
            .map_err(|e| format!("failed to build MTP verification batch: {e}"))?;
    }
    target
        .decode(&mut batch)
        .map_err(|e| format!("MTP verification decode failed: {e}"))?;

    let mut matched = 0usize;
    let mut sampled = sampler.sample(target, 0);
    while matched < drafted.len() && sampled == drafted[matched] {
        matched += 1;
        sampled = sampler.sample(target, matched as i32);
    }
    let extra = sampled;

    rt.carry_hidden = target
        .embeddings_nextn_ith(matched as i32)
        .map_err(|e| format!("failed to read target nextn embeddings: {e}"))?
        .to_vec();

    if matched < drafted.len() {
        let clear_from = u32::try_from(pos + matched as i32)
            .map_err(|_| "MTP rollback position does not fit into u32".to_string())?;
        let rolled_back = target
            .clear_kv_cache_seq(Some(0), Some(clear_from), None)
            .map_err(|e| format!("failed to roll back target KV cache: {e}"))?;
        if !rolled_back {
            return Err(format!(
                "target KV rollback failed at position {clear_from}"
            ));
        }
    }

    rt.last_token = extra;
    let mut accepted = drafted[..matched].to_vec();
    accepted.push(extra);
    rt.accepted += accepted.len() as u64;
    record_adaptive_round(rt, drafted.len(), matched);

    Ok(accepted)
}

fn rollback_and_advance(
    target: &mut LlamaContext<'_>,
    rt: &mut MtpRuntime<'_>,
    pos: i32,
    matched: usize,
    extra: LlamaToken,
    extra_hidden: &[f32],
) -> Result<(), String> {
    let extra_pos = pos + matched as i32;
    let rollback_pos = u32::try_from(extra_pos)
        .map_err(|_| "MTP rollback position does not fit into u32".to_string())?;

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
        .map_err(|e| format!("failed to roll back MTP draft KV cache: {e}"))?;
    if !draft_rolled_back {
        return Err(format!(
            "MTP draft KV rollback failed at position {rollback_pos}"
        ));
    }

    let mut target_batch = LlamaBatch::new(1, 1);
    target_batch
        .add(extra, extra_pos, &[0], true)
        .map_err(|e| format!("failed to build MTP target advance batch: {e}"))?;
    target
        .decode(&mut target_batch)
        .map_err(|e| format!("failed to advance target with accepted token: {e}"))?;
    rt.carry_hidden = target
        .embeddings_nextn_ith(0)
        .map_err(|e| format!("failed to read target nextn embeddings: {e}"))?
        .to_vec();

    let mut draft_batch = LlamaBatch::new_with_embeddings(1, rt.n_embd, 1);
    draft_batch
        .add_with_embedding(extra, extra_hidden, extra_pos, &[0], true)
        .map_err(|e| format!("failed to build MTP draft advance batch: {e}"))?;
    rt.draft
        .decode(&mut draft_batch)
        .map_err(|e| format!("failed to advance MTP draft with accepted token: {e}"))?;
    rt.draft_last_row = 0;

    Ok(())
}

fn greedy_token_with_prob(logits: &[f32]) -> (LlamaToken, f32) {
    let mut top: Vec<(usize, f32)> = Vec::with_capacity(MTP_DRAFT_TOP_K + 1);
    for (i, &logit) in logits.iter().enumerate() {
        if top.len() < MTP_DRAFT_TOP_K || logit > top.last().expect("top is non-empty").1 {
            let insert_at = top.partition_point(|&(_, v)| v >= logit);
            top.insert(insert_at, (i, logit));
            if top.len() > MTP_DRAFT_TOP_K {
                top.pop();
            }
        }
    }
    let (idx, max) = *top.first().expect("logits must not be empty");
    let sum: f32 = top.iter().map(|&(_, logit)| (logit - max).exp()).sum();
    (LlamaToken::new(idx as i32), 1.0 / sum)
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
