//! Multi-token prediction (speculative decoding with a draft head or a
//! separate draft model): detecting a bundled NextN draft, finding an
//! external `mtp-*.gguf` draft beside the model, and the draft/verify rounds
//! with their adaptive draft length.

use std::collections::VecDeque;
use std::path::Path;

use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::context::params::{LlamaContextParams, LlamaContextType};
use llama_cpp_2::gguf::GgufContext;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token::LlamaToken;

pub use crate::request::{MTP_DRAFT_DEFAULT, MTP_DRAFT_MAX};
const MTP_DRAFT_TOP_K: usize = 10;
const MTP_DRAFT_P_MIN: f32 = 0.75;
const MTP_ADAPT_WINDOW_ROUNDS: u32 = 8;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct MtpError(pub String);

fn fail<E: std::fmt::Display>(context: &str) -> impl FnOnce(E) -> MtpError + '_ {
    move |error| MtpError(format!("{context}: {error}"))
}

pub struct MtpRuntime<'m> {
    pub draft: LlamaContext<'m>,
    pub shared: bool,
    pub primed: bool,
    pub draft_n: usize,
    pub draft_n_max: usize,
    pub adaptation_count: u32,
    adaptive_rounds: u32,
    adaptive_drafted: u64,
    adaptive_matched: u64,
    pub max_batch: usize,
    verify_limit: usize,
    pub n_embd: usize,
    pub carry_hidden: Vec<f32>,
    pub h_last: Vec<f32>,
    pub last_token: LlamaToken,
    pub draft_last_row: i32,
    pub pending: VecDeque<LlamaToken>,
    pub rounds: u64,
    pub drafted: u64,
    pub accepted: u64,
}

impl std::fmt::Debug for MtpRuntime<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MtpRuntime")
            .field("shared", &self.shared)
            .field("primed", &self.primed)
            .field("draft_n", &self.draft_n)
            .field("draft_n_max", &self.draft_n_max)
            .field("rounds", &self.rounds)
            .field("drafted", &self.drafted)
            .field("accepted", &self.accepted)
            .finish_non_exhaustive()
    }
}

impl<'m> MtpRuntime<'m> {
    /// A draft context beside `target_ctx`. A draft model as wide as the
    /// target runs its own KV cache; one whose output width matches shares
    /// the target's hidden state instead.
    pub fn new(
        target_model: &LlamaModel,
        draft_model: &'m LlamaModel,
        target_ctx: &LlamaContext<'_>,
        backend: &LlamaBackend,
        draft_params: LlamaContextParams,
        draft_n: usize,
    ) -> Result<Self, MtpError> {
        let shared = if draft_model.n_embd() == target_model.n_embd() {
            false
        } else if draft_model.n_embd_out() == target_model.n_embd() {
            true
        } else {
            return Err(MtpError(format!(
                "MTP draft model widths (n_embd {}, n_embd_out {}) do not match target model width {}",
                draft_model.n_embd(),
                draft_model.n_embd_out(),
                target_model.n_embd()
            )));
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
            .map_err(fail("failed to create MTP draft context"))?;
        let n_embd = usize::try_from(target_model.n_embd())
            .map_err(|_| MtpError("model n_embd does not fit into usize".to_string()))?;

        let draft_n = draft_n.max(1);
        let verify_limit = verify_limit(target_ctx.n_batch() as usize, shared);
        Ok(Self {
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
            verify_limit,
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

    pub fn enable_nextn_embeddings(
        &mut self,
        target: &mut LlamaContext<'_>,
    ) -> Result<(), MtpError> {
        target
            .set_embeddings_nextn(true, false)
            .map_err(fail("failed to enable nextn embeddings on target context"))?;
        self.draft
            .set_embeddings_nextn(true, self.shared)
            .map_err(fail(
                "failed to enable nextn embeddings on MTP draft context",
            ))?;
        Ok(())
    }

    pub fn reset_for_prompt_reuse(&mut self, draft_clear_from: u32) -> Result<(), MtpError> {
        let cleared = if self.shared {
            self.draft.clear_kv_cache();
            true
        } else {
            self.draft
                .clear_kv_cache_seq(Some(0), Some(draft_clear_from), None)
                .map_err(fail("failed to rewind MTP prompt cache"))?
        };
        if !cleared {
            return Err(MtpError(format!(
                "MTP prompt cache rewind failed at position {draft_clear_from}"
            )));
        }
        self.primed = false;
        self.carry_hidden.fill(0.0);
        self.h_last.fill(0.0);
        self.last_token = LlamaToken::new(0);
        self.draft_last_row = 0;
        self.pending.clear();
        self.rounds = 0;
        self.drafted = 0;
        self.accepted = 0;
        self.adaptation_count = 0;
        self.adaptive_rounds = 0;
        self.adaptive_drafted = 0;
        self.adaptive_matched = 0;
        Ok(())
    }

    fn record_adaptive_round(&mut self, drafted: usize, matched: usize) {
        self.adaptive_rounds = self.adaptive_rounds.saturating_add(1);
        self.adaptive_drafted = self.adaptive_drafted.saturating_add(drafted as u64);
        self.adaptive_matched = self.adaptive_matched.saturating_add(matched as u64);
        if self.adaptive_rounds < MTP_ADAPT_WINDOW_ROUNDS {
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
            self.adaptation_count = self.adaptation_count.saturating_add(1);
        }
        self.adaptive_rounds = 0;
        self.adaptive_drafted = 0;
        self.adaptive_matched = 0;
    }

    pub fn set_prefill_carry_from_target(
        &mut self,
        target: &LlamaContext<'_>,
        row: i32,
    ) -> Result<(), MtpError> {
        self.carry_hidden = target
            .embeddings_nextn_ith(row)
            .map_err(fail("failed to restore MTP prompt-cache carry"))?
            .to_vec();
        Ok(())
    }

    pub fn truncate_for_prompt_cache(
        &mut self,
        target: &mut LlamaContext<'_>,
        token_count: u32,
    ) -> Result<(), MtpError> {
        let target_cleared = target
            .clear_kv_cache_seq(Some(0), Some(token_count), None)
            .map_err(fail("failed to trim target prompt cache"))?;
        if !target_cleared {
            return Err(MtpError(format!(
                "target prompt cache trim failed at position {token_count}"
            )));
        }
        if !self.shared {
            let draft_cleared = self
                .draft
                .clear_kv_cache_seq(Some(0), Some(token_count), None)
                .map_err(fail("failed to trim MTP prompt cache"))?;
            if !draft_cleared {
                return Err(MtpError(format!(
                    "MTP prompt cache trim failed at position {token_count}"
                )));
            }
        }
        self.pending.clear();
        Ok(())
    }

    pub fn prefill_draft_chunk(
        &mut self,
        target: &LlamaContext<'_>,
        chunk_tokens: &[LlamaToken],
        chunk_start_pos: i32,
        is_final_chunk: bool,
    ) -> Result<(), MtpError> {
        if !self.shared && chunk_tokens.len() > self.max_batch {
            for (sub_index, subchunk) in chunk_tokens.chunks(self.max_batch).enumerate() {
                let sub_start = sub_index
                    .checked_mul(self.max_batch)
                    .and_then(|offset| i32::try_from(offset).ok())
                    .and_then(|offset| chunk_start_pos.checked_add(offset))
                    .ok_or_else(|| {
                        MtpError("MTP draft prefill position overflowed i32".to_string())
                    })?;
                let sub_end = (sub_index + 1).saturating_mul(self.max_batch);
                let sub_is_final = is_final_chunk && sub_end >= chunk_tokens.len();
                let row_offset = (sub_index * self.max_batch) as i32;
                self.prefill_draft_chunk_inner(
                    target,
                    subchunk,
                    sub_start,
                    sub_is_final,
                    row_offset,
                )?;
            }
            return Ok(());
        }

        self.prefill_draft_chunk_inner(target, chunk_tokens, chunk_start_pos, is_final_chunk, 0)
    }

    fn prefill_draft_chunk_inner(
        &mut self,
        target: &LlamaContext<'_>,
        chunk_tokens: &[LlamaToken],
        chunk_start_pos: i32,
        is_final_chunk: bool,
        target_row_offset: i32,
    ) -> Result<(), MtpError> {
        if chunk_tokens.is_empty() {
            return Ok(());
        }

        if self.shared {
            self.h_last = target
                .embeddings_nextn_ith(chunk_tokens.len() as i32 - 1)
                .map_err(fail("failed to read target nextn embeddings"))?
                .to_vec();
            return Ok(());
        }

        let zero = vec![0.0_f32; self.n_embd];
        let mut batch = LlamaBatch::new_with_embeddings(chunk_tokens.len(), self.n_embd, 1);

        for (i, token) in chunk_tokens.iter().enumerate() {
            let global_pos = chunk_start_pos + i as i32;
            let hidden: &[f32] = if global_pos == 0 {
                &zero
            } else if i == 0 {
                &self.carry_hidden
            } else {
                target
                    .embeddings_nextn_ith(target_row_offset + i as i32 - 1)
                    .map_err(fail("failed to read target nextn embeddings"))?
            };
            let logits = is_final_chunk && i + 1 == chunk_tokens.len();
            batch
                .add_with_embedding(*token, hidden, global_pos, &[0], logits)
                .map_err(fail("failed to build MTP draft prefill batch"))?;
        }

        self.draft
            .decode(&mut batch)
            .map_err(fail("MTP draft prefill decode failed"))?;

        self.carry_hidden = target
            .embeddings_nextn_ith(target_row_offset + chunk_tokens.len() as i32 - 1)
            .map_err(fail("failed to read target nextn embeddings"))?
            .to_vec();
        self.draft_last_row = chunk_tokens.len() as i32 - 1;
        Ok(())
    }

    /// One draft-then-verify round; returns the accepted tokens (at least
    /// the target's own next token).
    pub fn round(
        &mut self,
        target: &mut LlamaContext<'_>,
        sampler: &mut LlamaSampler,
        model: &LlamaModel,
        pos: i32,
        max_pos: i32,
    ) -> Result<Vec<LlamaToken>, MtpError> {
        if self.shared {
            return self.round_shared(target, sampler, model, pos, max_pos);
        }

        self.rounds += 1;
        let prefix_hidden = self.carry_hidden.clone();
        let budget = (max_pos - pos - 1).max(0) as usize;
        let steps = self.draft_n.min(budget).min(self.verify_limit);

        let mut drafted: Vec<LlamaToken> = Vec::with_capacity(steps);
        if steps > 0 {
            let mut h_prev = self
                .draft
                .embeddings_nextn_ith(self.draft_last_row)
                .map_err(fail("failed to read MTP draft nextn embeddings"))?
                .to_vec();

            for step in 0..steps {
                let Some((token, prob)) = greedy_token_with_prob(self.draft.get_logits()) else {
                    break;
                };
                if prob < MTP_DRAFT_P_MIN {
                    break;
                }
                drafted.push(token);
                if model.is_eog_token(token) {
                    break;
                }

                let mut batch = LlamaBatch::new_with_embeddings(1, self.n_embd, 1);
                batch
                    .add_with_embedding(token, &h_prev, pos + step as i32, &[0], true)
                    .map_err(fail("failed to build MTP draft step batch"))?;
                self.draft
                    .decode(&mut batch)
                    .map_err(fail("MTP draft step decode failed"))?;
                self.draft_last_row = 0;
                h_prev = self
                    .draft
                    .embeddings_nextn_ith(0)
                    .map_err(fail("failed to read MTP draft nextn embeddings"))?
                    .to_vec();
            }
        }
        self.drafted += drafted.len() as u64;

        let first = sampler.sample(target, -1);

        if drafted.first() != Some(&first) {
            self.rollback_and_advance(target, pos, 0, first, &prefix_hidden)?;
            self.accepted += 1;
            self.record_adaptive_round(drafted.len(), 0);
            return Ok(vec![first]);
        }

        let mut batch = LlamaBatch::new(drafted.len(), 1);
        for (i, token) in drafted.iter().enumerate() {
            batch
                .add(*token, pos + i as i32, &[0], true)
                .map_err(fail("failed to build MTP verification batch"))?;
        }
        target
            .decode(&mut batch)
            .map_err(fail("MTP verification decode failed"))?;

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
                .map_err(fail("failed to read target nextn embeddings"))?
                .to_vec()
        };
        self.rollback_and_advance(target, pos, matched, extra, &extra_hidden)?;
        self.accepted += accepted.len() as u64;
        self.record_adaptive_round(drafted.len(), matched);

        Ok(accepted)
    }

    fn round_shared(
        &mut self,
        target: &mut LlamaContext<'_>,
        sampler: &mut LlamaSampler,
        model: &LlamaModel,
        pos: i32,
        max_pos: i32,
    ) -> Result<Vec<LlamaToken>, MtpError> {
        self.rounds += 1;

        if !self.primed {
            self.primed = true;
            let first = sampler.sample(target, -1);
            self.carry_hidden = self.h_last.clone();
            self.last_token = first;
            self.accepted += 1;
            return Ok(vec![first]);
        }

        let steps = self
            .draft_n
            .min((max_pos - pos).max(0) as usize)
            .min(self.verify_limit);

        let mut drafted: Vec<LlamaToken> = Vec::with_capacity(steps);
        let mut input = self.last_token;
        let mut h_prev = self.carry_hidden.clone();
        for _ in 0..steps {
            let mut batch = LlamaBatch::new_with_embeddings(1, self.n_embd, 1);
            batch
                .add_with_embedding(input, &h_prev, pos - 1, &[0], true)
                .map_err(fail("failed to build MTP draft step batch"))?;
            self.draft
                .decode(&mut batch)
                .map_err(fail("MTP draft step decode failed"))?;

            let Some((token, prob)) = greedy_token_with_prob(self.draft.get_logits()) else {
                break;
            };
            if prob < MTP_DRAFT_P_MIN {
                break;
            }
            drafted.push(token);
            if model.is_eog_token(token) {
                break;
            }
            h_prev = self
                .draft
                .embeddings_nextn_ith(0)
                .map_err(fail("failed to read MTP draft nextn embeddings"))?
                .to_vec();
            input = token;
        }
        self.drafted += drafted.len() as u64;

        let mut batch = LlamaBatch::new(drafted.len() + 1, 1);
        batch
            .add(self.last_token, pos - 1, &[0], true)
            .map_err(fail("failed to build MTP verification batch"))?;
        for (i, token) in drafted.iter().enumerate() {
            batch
                .add(*token, pos + i as i32, &[0], true)
                .map_err(fail("failed to build MTP verification batch"))?;
        }
        target
            .decode(&mut batch)
            .map_err(fail("MTP verification decode failed"))?;

        let mut matched = 0usize;
        let mut sampled = sampler.sample(target, 0);
        while matched < drafted.len() && sampled == drafted[matched] {
            matched += 1;
            sampled = sampler.sample(target, matched as i32);
        }
        let extra = sampled;

        self.carry_hidden = target
            .embeddings_nextn_ith(matched as i32)
            .map_err(fail("failed to read target nextn embeddings"))?
            .to_vec();

        if matched < drafted.len() {
            let clear_from = u32::try_from(pos + matched as i32)
                .map_err(|_| MtpError("MTP rollback position does not fit into u32".to_string()))?;
            let rolled_back = target
                .clear_kv_cache_seq(Some(0), Some(clear_from), None)
                .map_err(fail("failed to roll back target KV cache"))?;
            if !rolled_back {
                return Err(MtpError(format!(
                    "target KV rollback failed at position {clear_from}"
                )));
            }
        }

        self.last_token = extra;
        let mut accepted = drafted[..matched].to_vec();
        accepted.push(extra);
        self.accepted += accepted.len() as u64;
        self.record_adaptive_round(drafted.len(), matched);

        Ok(accepted)
    }

    fn rollback_and_advance(
        &mut self,
        target: &mut LlamaContext<'_>,
        pos: i32,
        matched: usize,
        extra: LlamaToken,
        extra_hidden: &[f32],
    ) -> Result<(), MtpError> {
        let extra_pos = pos + matched as i32;
        let rollback_pos = u32::try_from(extra_pos)
            .map_err(|_| MtpError("MTP rollback position does not fit into u32".to_string()))?;

        let target_rolled_back = target
            .clear_kv_cache_seq(Some(0), Some(rollback_pos), None)
            .map_err(fail("failed to roll back target KV cache"))?;
        if !target_rolled_back {
            return Err(MtpError(format!(
                "target KV rollback failed at position {rollback_pos}"
            )));
        }

        let draft_rolled_back = self
            .draft
            .clear_kv_cache_seq(Some(0), Some(rollback_pos), None)
            .map_err(fail("failed to roll back MTP draft KV cache"))?;
        if !draft_rolled_back {
            return Err(MtpError(format!(
                "MTP draft KV rollback failed at position {rollback_pos}"
            )));
        }

        let mut target_batch = LlamaBatch::new(1, 1);
        target_batch
            .add(extra, extra_pos, &[0], true)
            .map_err(fail("failed to build MTP target advance batch"))?;
        target
            .decode(&mut target_batch)
            .map_err(fail("failed to advance target with accepted token"))?;
        self.carry_hidden = target
            .embeddings_nextn_ith(0)
            .map_err(fail("failed to read target nextn embeddings"))?
            .to_vec();

        let mut draft_batch = LlamaBatch::new_with_embeddings(1, self.n_embd, 1);
        draft_batch
            .add_with_embedding(extra, extra_hidden, extra_pos, &[0], true)
            .map_err(fail("failed to build MTP draft advance batch"))?;
        self.draft
            .decode(&mut draft_batch)
            .map_err(fail("failed to advance MTP draft with accepted token"))?;
        self.draft_last_row = 0;

        Ok(())
    }
}

/// How many drafts one verification batch can carry within the target's
/// `n_batch`: the drafts alone with an own KV cache, the drafts after the
/// last accepted token with a shared hidden state.
fn verify_limit(target_batch: usize, shared: bool) -> usize {
    if shared {
        target_batch.saturating_sub(1)
    } else {
        target_batch
    }
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

/// The arg-max token and its softmax probability over the top ten logits.
pub(crate) fn greedy_token_with_prob(logits: &[f32]) -> Option<(LlamaToken, f32)> {
    let mut top: Vec<(usize, f32)> = Vec::with_capacity(MTP_DRAFT_TOP_K + 1);
    for (i, &logit) in logits.iter().enumerate() {
        if top.len() < MTP_DRAFT_TOP_K || top.last().is_some_and(|&(_, lowest)| logit > lowest) {
            let insert_at = top.partition_point(|&(_, v)| v >= logit);
            top.insert(insert_at, (i, logit));
            if top.len() > MTP_DRAFT_TOP_K {
                top.pop();
            }
        }
    }
    let &(idx, max) = top.first()?;
    let sum: f32 = top.iter().map(|&(_, logit)| (logit - max).exp()).sum();
    Some((LlamaToken::new(idx as i32), 1.0 / sum))
}

/// Whether the model carries its own NextN draft layers.
#[must_use]
pub fn model_has_mtp(model_path: &str) -> bool {
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

/// The first (by name) `mtp-<stem>*.gguf` or `<stem>-mtp.gguf` beside the
/// model whose stem the model's file name contains.
#[must_use]
pub fn discover_external_mtp(model_path: &str) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn verification_batches_fit_the_target_batch() {
        assert_eq!(verify_limit(512, false), 512);
        assert_eq!(verify_limit(8, false), 8);
        assert_eq!(verify_limit(8, true), 7);
        assert_eq!(verify_limit(1, true), 0);
        assert_eq!(verify_limit(0, true), 0);
    }

    #[test]
    fn adaptive_draft_length_holds_middle_acceptance() {
        assert_eq!(adjusted_draft_length(4, 8, 10, 6), 4);
    }

    #[test]
    fn greedy_draft_token_reports_its_top_k_probability() {
        assert_eq!(greedy_token_with_prob(&[]), None);
        let (token, prob) = greedy_token_with_prob(&[0.0, 5.0, 0.0]).expect("token");
        assert_eq!(token, LlamaToken::new(1));
        let expected = 1.0 / (1.0 + 2.0 * (-5.0_f32).exp());
        assert!((prob - expected).abs() < 1e-6);
        let mut logits = vec![0.0_f32; 20];
        logits[7] = 1.0;
        let (token, prob) = greedy_token_with_prob(&logits).expect("token");
        assert_eq!(token, LlamaToken::new(7));
        let expected = 1.0 / (1.0 + 9.0 * (-1.0_f32).exp());
        assert!((prob - expected).abs() < 1e-6);
    }

    #[test]
    fn external_drafts_match_by_stem_and_the_first_name_wins() {
        let dir = std::env::temp_dir().join(format!("lettuce-mtp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        for name in [
            "Qwen3-8B-Q4.gguf",
            "mtp-qwen3-8b.gguf",
            "qwen3-8b-mtp.gguf",
            "mtp-other.gguf",
            "notes.txt",
        ] {
            std::fs::write(dir.join(name), b"x").expect("file");
        }
        let model = dir.join("Qwen3-8B-Q4.gguf");
        let found = discover_external_mtp(model.to_str().expect("path")).expect("draft");
        assert!(found.ends_with("mtp-qwen3-8b.gguf"));
        std::fs::remove_dir_all(&dir).ok();
    }

    fn greedy_plain(
        model: &LlamaModel,
        backend: &LlamaBackend,
        prompt: &[LlamaToken],
        count: usize,
    ) -> Vec<LlamaToken> {
        let params = LlamaContextParams::default()
            .with_n_ctx(std::num::NonZeroU32::new(256))
            .with_n_batch(64);
        let mut ctx = model.new_context(backend, params).expect("context");
        let mut batch = LlamaBatch::new(64, 1);
        for (i, token) in prompt.iter().enumerate() {
            batch
                .add(*token, i as i32, &[0], i + 1 == prompt.len())
                .expect("batch");
        }
        ctx.decode(&mut batch).expect("prompt");
        let mut sampler = LlamaSampler::greedy();
        let mut out = Vec::new();
        let mut pos = prompt.len() as i32;
        let mut index = batch.n_tokens() - 1;
        while out.len() < count {
            let token = sampler.sample(&ctx, index);
            out.push(token);
            batch.clear();
            batch.add(token, pos, &[0], true).expect("batch");
            pos += 1;
            ctx.decode(&mut batch).expect("decode");
            index = 0;
        }
        out
    }

    #[test]
    #[ignore = "needs LETTUCE_PLAN_MODEL and a draft in LETTUCE_MTP_MODEL or beside it"]
    fn greedy_mtp_matches_plain_greedy_decoding() {
        let Ok(path) = std::env::var("LETTUCE_PLAN_MODEL") else {
            return;
        };
        let draft_path = std::env::var("LETTUCE_MTP_MODEL")
            .ok()
            .or_else(|| discover_external_mtp(&path))
            .expect("external draft");
        let engine = crate::engine::LlamaEngine::new();
        let loaded = engine
            .load(
                None,
                &crate::engine::EngineLoadRequest {
                    request_id: None,
                    model_path: &path,
                    requested_gpu_layers: Some(0),
                    auto_gpu_layer_candidates: None,
                    native_fit_plan: None,
                    gpu_config: crate::engine::LlamaGpuConfig::default(),
                    strict_mode: false,
                    mmproj_path: None,
                    load_bundled_mtp: false,
                    mtp_model_path: Some(&draft_path),
                    mtp_drafter_on_gpu: false,
                    mtp_gpu_fallback_allowed: false,
                    mtp_gpu_device_id: None,
                },
                || {},
            )
            .expect("load");
        let model = &loaded.model;
        let draft_model = loaded.mtp_model.as_ref().expect("draft model");
        let prompt = model
            .str_to_token(
                "The capital of France is Paris. The capital of Germany is",
                llama_cpp_2::model::AddBos::Always,
            )
            .expect("tokens");
        let count = 16;
        let expected = greedy_plain(model, &loaded.backend, &prompt, count);

        let draft_n = MTP_DRAFT_DEFAULT;
        let target_params = LlamaContextParams::default()
            .with_n_ctx(std::num::NonZeroU32::new(256))
            .with_n_batch(64)
            .with_n_outputs_max(draft_n + 1)
            .with_n_rs_seq(draft_n);
        let mut target = model
            .new_context(&loaded.backend, target_params)
            .expect("target context");
        let draft_params = LlamaContextParams::default()
            .with_n_ctx(std::num::NonZeroU32::new(256))
            .with_n_batch(64)
            .with_n_rs_seq(draft_n);
        let mut runtime = MtpRuntime::new(
            model,
            draft_model,
            &target,
            &loaded.backend,
            draft_params,
            draft_n as usize,
        )
        .expect("runtime");
        runtime.enable_nextn_embeddings(&mut target).expect("nextn");

        let mut batch = LlamaBatch::new(64, 1);
        for (i, token) in prompt.iter().enumerate() {
            batch
                .add(*token, i as i32, &[0], i + 1 == prompt.len())
                .expect("batch");
        }
        target.decode(&mut batch).expect("prompt");
        runtime
            .prefill_draft_chunk(&target, &prompt, 0, true)
            .expect("draft prefill");

        let mut sampler = LlamaSampler::greedy();
        let mut produced = Vec::new();
        let mut pos = prompt.len() as i32;
        while produced.len() < count {
            if runtime.pending.is_empty() {
                let accepted = runtime
                    .round(&mut target, &mut sampler, model, pos, 256)
                    .expect("round");
                runtime.pending.extend(accepted);
            }
            let Some(token) = runtime.pending.pop_front() else {
                break;
            };
            produced.push(token);
            pos += 1;
        }
        eprintln!(
            "mtp rounds={} drafted={} accepted={} shared={}",
            runtime.rounds, runtime.drafted, runtime.accepted, runtime.shared
        );
        assert_eq!(produced, expected);
        assert!(runtime.rounds < count as u64);
        drop(runtime);
        drop(target);
        engine.unload().expect("unload");
    }
}
