//! Shared dynamic memory utilities
//!
//! This module provides constants and helper functions for dynamic memory
//! that are shared between chat_manager and group_chat_manager.

use std::collections::{HashMap, HashSet};

use crate::chat_manager::types::{
    DynamicMemorySettings, DynamicMemoryStructuredFallbackFormat, MemoryRetrievalStrategy, Settings,
};

// ============================================================================
// Shared Memory Entry Trait
// ============================================================================

pub trait MemoryEntry {
    fn id(&self) -> &str;
    fn text(&self) -> &str;
    fn embedding(&self) -> &[f32];
    fn token_count(&self) -> u32;
    fn is_cold(&self) -> bool;
    fn set_is_cold(&mut self, value: bool);
    fn is_pinned(&self) -> bool;
    fn importance_score(&self) -> f32;
    fn set_importance_score(&mut self, value: f32);
    fn last_accessed_at(&self) -> u64;
    fn set_last_accessed_at(&mut self, value: u64);
    fn access_count(&self) -> u32;
    fn set_access_count(&mut self, value: u32);
    fn category(&self) -> Option<&str>;
}

impl MemoryEntry for crate::chat_manager::types::MemoryEmbedding {
    fn id(&self) -> &str {
        &self.id
    }
    fn text(&self) -> &str {
        &self.text
    }
    fn embedding(&self) -> &[f32] {
        &self.embedding
    }
    fn token_count(&self) -> u32 {
        self.token_count
    }
    fn is_cold(&self) -> bool {
        self.is_cold
    }
    fn set_is_cold(&mut self, value: bool) {
        self.is_cold = value;
    }
    fn is_pinned(&self) -> bool {
        self.is_pinned
    }
    fn importance_score(&self) -> f32 {
        self.importance_score
    }
    fn set_importance_score(&mut self, value: f32) {
        self.importance_score = value;
    }
    fn last_accessed_at(&self) -> u64 {
        self.last_accessed_at
    }
    fn set_last_accessed_at(&mut self, value: u64) {
        self.last_accessed_at = value;
    }
    fn access_count(&self) -> u32 {
        self.access_count
    }
    fn set_access_count(&mut self, value: u32) {
        self.access_count = value;
    }
    fn category(&self) -> Option<&str> {
        self.category.as_deref()
    }
}

// `storage_manager::group_sessions::MemoryEmbedding` is now a re-export of the
// rich `chat_manager::types::MemoryEmbedding`, so the impl above already covers
// both single-character and group sessions.

// ============================================================================
// Constants
// ============================================================================

pub const FALLBACK_DYNAMIC_WINDOW: u32 = 20;
pub const FALLBACK_DYNAMIC_MAX_ENTRIES: u32 = 50;
pub const FALLBACK_MIN_SIMILARITY: f32 = 0.35;
pub const FALLBACK_RETRIEVAL_LIMIT: u32 = 5;
pub const FALLBACK_RETRIEVAL_STRATEGY: MemoryRetrievalStrategy = MemoryRetrievalStrategy::Smart;
pub const FALLBACK_HOT_MEMORY_TOKEN_BUDGET: u32 = 2000;
pub const FALLBACK_DECAY_RATE: f32 = 0.08;
pub const FALLBACK_COLD_THRESHOLD: f32 = 0.3;
pub const FALLBACK_STRUCTURED_FALLBACK_FORMAT: DynamicMemoryStructuredFallbackFormat =
    DynamicMemoryStructuredFallbackFormat::Xml;
pub const MEMORY_ID_SPACE: u64 = 1_000_000;
// ============================================================================
// Settings Helper Functions
// ============================================================================

/// Check if dynamic memory is enabled in settings
#[allow(dead_code)]
pub fn is_dynamic_memory_enabled(settings: &Settings) -> bool {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|a| a.dynamic_memory.as_ref())
        .map(|dm| dm.enabled)
        .unwrap_or(false)
}

/// Get the summary message interval (window size) from settings
pub fn dynamic_window_size(settings: &Settings) -> usize {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|a| a.dynamic_memory.as_ref())
        .map(|dm| dm.summary_message_interval.max(1))
        .unwrap_or(FALLBACK_DYNAMIC_WINDOW) as usize
}

/// Get the maximum number of memory entries from settings
pub fn dynamic_max_entries(settings: &Settings) -> usize {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|a| a.dynamic_memory.as_ref())
        .map(|dm| dm.max_entries.max(1))
        .unwrap_or(FALLBACK_DYNAMIC_MAX_ENTRIES) as usize
}

/// Get the minimum similarity threshold for memory retrieval
pub fn dynamic_min_similarity(settings: &Settings) -> f32 {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|a| a.dynamic_memory.as_ref())
        .map(|dm| dm.min_similarity_threshold)
        .unwrap_or(FALLBACK_MIN_SIMILARITY)
}

/// Get the hot memory token budget from settings
pub fn dynamic_hot_memory_token_budget(settings: &Settings) -> u32 {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|a| a.dynamic_memory.as_ref())
        .map(|dm| dm.hot_memory_token_budget)
        .unwrap_or(FALLBACK_HOT_MEMORY_TOKEN_BUDGET)
}

/// Get the max number of memories to retrieve per turn
pub fn dynamic_retrieval_limit(settings: &Settings) -> usize {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|a| a.dynamic_memory.as_ref())
        .map(|dm| dm.retrieval_limit.max(1))
        .unwrap_or(FALLBACK_RETRIEVAL_LIMIT) as usize
}

/// Get memory retrieval strategy
pub fn dynamic_retrieval_strategy(settings: &Settings) -> MemoryRetrievalStrategy {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|a| a.dynamic_memory.as_ref())
        .map(|dm| dm.retrieval_strategy.clone())
        .unwrap_or_else(|| FALLBACK_RETRIEVAL_STRATEGY.clone())
}

/// Get the decay rate for memory importance scores
pub fn dynamic_decay_rate(settings: &Settings) -> f32 {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|a| a.dynamic_memory.as_ref())
        .map(|dm| dm.decay_rate)
        .unwrap_or(FALLBACK_DECAY_RATE)
}

/// Get the threshold below which memories are demoted to cold storage
pub fn dynamic_cold_threshold(settings: &Settings) -> f32 {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|a| a.dynamic_memory.as_ref())
        .map(|dm| dm.cold_threshold)
        .unwrap_or(FALLBACK_COLD_THRESHOLD)
}

/// Check if context enrichment (semantic search) is enabled
pub fn context_enrichment_enabled(settings: &Settings) -> bool {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|a| a.dynamic_memory.as_ref())
        .map(|dm| dm.context_enrichment_enabled)
        .unwrap_or(true) // Default to enabled
}

pub fn dynamic_memory_structured_fallback_format(
    settings: &Settings,
) -> DynamicMemoryStructuredFallbackFormat {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|a| a.dynamic_memory_structured_fallback_format)
        .unwrap_or(FALLBACK_STRUCTURED_FALLBACK_FORMAT)
}

/// Resolve the effective dynamic memory settings, applying optional overrides.
pub fn effective_dynamic_memory_settings(
    settings: &Settings,
    overrides: Option<&DynamicMemorySettings>,
) -> DynamicMemorySettings {
    if let Some(custom) = overrides {
        return custom.clone();
    }

    if let Some(dynamic) = settings
        .advanced_settings
        .as_ref()
        .and_then(|a| a.dynamic_memory.as_ref())
    {
        return dynamic.clone();
    }

    DynamicMemorySettings {
        enabled: false,
        summary_message_interval: FALLBACK_DYNAMIC_WINDOW,
        max_entries: FALLBACK_DYNAMIC_MAX_ENTRIES,
        min_similarity_threshold: FALLBACK_MIN_SIMILARITY,
        retrieval_limit: FALLBACK_RETRIEVAL_LIMIT,
        retrieval_strategy: FALLBACK_RETRIEVAL_STRATEGY,
        hot_memory_token_budget: FALLBACK_HOT_MEMORY_TOKEN_BUDGET,
        decay_rate: FALLBACK_DECAY_RATE,
        cold_threshold: FALLBACK_COLD_THRESHOLD,
        delete_confidence_default: 0.5,
        max_hard_delete_ratio_per_cycle: 0.5,
        context_enrichment_enabled: true,
        recursive_memory_loops: false,
        recursive_memory_loop_hard_cap: 20,
    }
}

/// Resolve effective dynamic memory settings for group chats.
pub fn effective_group_dynamic_memory_settings(settings: &Settings) -> DynamicMemorySettings {
    let overrides = settings
        .advanced_settings
        .as_ref()
        .and_then(|a| a.group_dynamic_memory.as_ref());

    effective_dynamic_memory_settings(settings, overrides)
}

// ============================================================================
// Memory ID Generation
// ============================================================================

/// Generate a unique memory ID based on current timestamp
pub fn generate_memory_id() -> String {
    let now = crate::utils::now_millis().unwrap_or(0);
    format!("{:06}", now % MEMORY_ID_SPACE)
}

// ============================================================================
// Memory Helpers
// ============================================================================

/// Normalize text for keyword matching (lowercase, strip punctuation).
pub fn normalize_query_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_space = false;

    for ch in text.chars() {
        if ch.is_alphanumeric() {
            for lower in ch.to_lowercase() {
                out.push(lower);
            }
            last_space = false;
        } else if !last_space {
            out.push(' ');
            last_space = true;
        }
    }

    out.trim().to_string()
}

pub fn extract_keywords(text: &str) -> Vec<String> {
    let normalized = normalize_query_text(text);
    let mut seen = HashSet::new();
    let mut keywords = Vec::new();

    for word in normalized.split_whitespace() {
        if word.len() < 3 {
            continue;
        }
        if seen.insert(word.to_string()) {
            keywords.push(word.to_string());
        }
    }

    keywords
}

fn lexical_overlap_ratio(a: &str, b: &str) -> f32 {
    let a_keywords: HashSet<String> = extract_keywords(a).into_iter().collect();
    let b_keywords: HashSet<String> = extract_keywords(b).into_iter().collect();

    if a_keywords.is_empty() || b_keywords.is_empty() {
        return 0.0;
    }

    let shared = a_keywords.intersection(&b_keywords).count() as f32;
    let largest = a_keywords.len().max(b_keywords.len()) as f32;
    if largest <= 0.0 {
        0.0
    } else {
        shared / largest
    }
}

/// Cosine threshold for treating a candidate memory as a duplicate. Tuned for
/// retrieval-trained embedders (lettuce-emb-v4) which compress raw cosine
/// values: paraphrases that score 0.92 on a vanilla SBERT typically land at
/// 0.78-0.82 here. The previous 0.85 threshold let near-identical summaries
/// slip through at moderate scale.
const DUPLICATE_MEMORY_COSINE_THRESHOLD: f32 = 0.78;

pub fn find_duplicate_memory_reason<E: MemoryEntry>(
    candidate_text: &str,
    candidate_embedding: Option<&[f32]>,
    memories: &[E],
) -> Option<String> {
    let normalized_candidate = normalize_query_text(candidate_text);
    let candidate_word_count = normalized_candidate.split_whitespace().count();

    for existing in memories {
        let normalized_existing = normalize_query_text(existing.text());

        if !normalized_candidate.is_empty() && normalized_candidate == normalized_existing {
            return Some("duplicate (normalized text match)".to_string());
        }

        // Run the embedding check before lexical so paraphrases with low
        // surface overlap but identical meaning still get caught.
        if let Some(new_emb) = candidate_embedding {
            if !existing.embedding().is_empty() {
                let cos = cosine_similarity(new_emb, existing.embedding());
                if cos > DUPLICATE_MEMORY_COSINE_THRESHOLD {
                    return Some(format!(
                        "duplicate (cosine {:.2} > {:.2})",
                        cos, DUPLICATE_MEMORY_COSINE_THRESHOLD
                    ));
                }
            }
        }

        if candidate_word_count >= 3 {
            let overlap = lexical_overlap_ratio(candidate_text, existing.text());
            if overlap >= 0.9 {
                return Some("duplicate (high lexical overlap)".to_string());
            }
        }
    }

    None
}

pub fn calculate_hot_memory_tokens<E: MemoryEntry>(memories: &[E]) -> u32 {
    memories
        .iter()
        .filter(|m| !m.is_cold())
        .map(|m| m.token_count())
        .sum()
}

/// Ensure pinned memories are not cold. Returns number of fixed entries.
pub fn ensure_pinned_hot<E: MemoryEntry>(memories: &mut [E]) -> usize {
    let mut fixed = 0;
    for mem in memories.iter_mut() {
        if mem.is_pinned() && mem.is_cold() {
            mem.set_is_cold(false);
            mem.set_importance_score(1.0);
            fixed += 1;
        }
    }
    fixed
}

/// Enforce hot memory budget by demoting oldest non-pinned memories. Returns demoted IDs.
pub fn enforce_hot_memory_budget<E: MemoryEntry>(memories: &mut [E], budget: u32) -> Vec<String> {
    let mut current_tokens = calculate_hot_memory_tokens(memories);
    if current_tokens <= budget {
        return Vec::new();
    }

    let mut hot_indices: Vec<(usize, u64)> = memories
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.is_cold() && !m.is_pinned())
        .map(|(i, m)| (i, m.last_accessed_at()))
        .collect();

    hot_indices.sort_by_key(|(_, accessed)| *accessed);

    let mut demoted = Vec::new();
    for (idx, _) in hot_indices {
        if current_tokens <= budget {
            break;
        }
        let tokens_freed = memories[idx].token_count();
        memories[idx].set_is_cold(true);
        demoted.push(memories[idx].id().to_string());
        current_tokens = current_tokens.saturating_sub(tokens_freed);
    }

    demoted
}

/// Apply decay; demote memories that fall below threshold. Returns (decayed_count, demoted_ids).
pub fn apply_memory_decay<E: MemoryEntry>(
    memories: &mut [E],
    decay_rate: f32,
    cold_threshold: f32,
) -> (usize, Vec<String>) {
    let mut decayed = 0;
    let mut demoted = Vec::new();

    for mem in memories.iter_mut() {
        if mem.is_cold() || mem.is_pinned() {
            continue;
        }

        let adaptive_rate = decay_rate / (1.0 + (mem.access_count() as f32).sqrt());
        let next_score = (mem.importance_score() - adaptive_rate).max(0.0);
        mem.set_importance_score(next_score);
        decayed += 1;

        if mem.importance_score() < cold_threshold {
            mem.set_is_cold(true);
            demoted.push(mem.id().to_string());
        }
    }

    (decayed, demoted)
}

/// Promote cold memories by ID. Returns the ids that were promoted so callers
/// can persist the change via a narrow DB update instead of a full
/// `save_session`.
pub fn promote_cold_memories<E: MemoryEntry>(
    memories: &mut [E],
    memory_ids: &[String],
    now: u64,
) -> Vec<String> {
    let mut promoted = Vec::new();
    for mem in memories.iter_mut() {
        if memory_ids.contains(&mem.id().to_string()) && mem.is_cold() {
            mem.set_is_cold(false);
            mem.set_importance_score(0.7);
            mem.set_last_accessed_at(now);
            mem.set_access_count(mem.access_count().saturating_add(1));
            promoted.push(mem.id().to_string());
        }
    }
    promoted
}

/// Update access tracking for retrieved memories. Returns one `AccessUpdate`
/// per memory touched so callers can flush the new values to the normalised
/// `memory_embeddings` table via a narrow UPDATE rather than a full session
/// save.
///
/// Importance is incremented by `ACCESS_IMPORTANCE_BOOST` (capped at 1.0) per
/// access instead of being slammed to 1.0. This preserves the per-cycle decay
/// signal: frequently retrieved memories drift up, then decay if retrieval
/// stops, instead of staying pinned at 1.0 forever after a single hit.
pub fn mark_memories_accessed<E: MemoryEntry>(
    memories: &mut [E],
    memory_ids: &[String],
    now: u64,
) -> Vec<crate::storage_manager::memory_embeddings::AccessUpdate> {
    const ACCESS_IMPORTANCE_BOOST: f32 = 0.2;
    let mut updates = Vec::new();
    for mem in memories.iter_mut() {
        if memory_ids.contains(&mem.id().to_string()) {
            mem.set_last_accessed_at(now);
            mem.set_access_count(mem.access_count().saturating_add(1));
            let next_score = (mem.importance_score() + ACCESS_IMPORTANCE_BOOST).min(1.0);
            mem.set_importance_score(next_score);
            updates.push(crate::storage_manager::memory_embeddings::AccessUpdate {
                memory_id: mem.id().to_string(),
                importance_score: mem.importance_score(),
                last_accessed_at: mem.last_accessed_at(),
                access_count: mem.access_count(),
            });
        }
    }
    updates
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dim = a.len().min(b.len());
    if dim == 0 {
        return 0.0;
    }
    let dot: f32 = a
        .iter()
        .take(dim)
        .zip(b.iter().take(dim))
        .map(|(x, y)| x * y)
        .sum();
    let norm_a: f32 = a.iter().take(dim).map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().take(dim).map(|x| x * x).sum::<f32>().sqrt();
    let denom = norm_a * norm_b;
    if denom == 0.0 {
        return 0.0;
    }
    dot / denom
}

/// Multiplier applied to cold-memory cosine scores so they only resurface on
/// strong semantic matches. Discourages cold memories from displacing fresh
/// hot ones for borderline queries while keeping them reachable when the
/// match is genuinely good.
const COLD_MEMORY_SCORE_MULTIPLIER: f32 = 0.7;

/// Select memories by similarity. Cold memories are included with a score
/// multiplier (`COLD_MEMORY_SCORE_MULTIPLIER`) so they can resurface on
/// strong matches without dominating routine retrieval. Returns (index,
/// score).
pub fn select_relevant_memory_indices<E: MemoryEntry>(
    query_embedding: &[f32],
    memories: &[E],
    limit: usize,
    min_similarity: f32,
) -> Vec<(usize, f32)> {
    let mut scored: Vec<(f32, usize)> = memories
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.embedding().is_empty())
        .map(|(i, m)| {
            let raw = cosine_similarity(query_embedding, m.embedding());
            let score = if m.is_cold() && !m.is_pinned() {
                raw * COLD_MEMORY_SCORE_MULTIPLIER
            } else {
                raw
            };
            (score, i)
        })
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let filtered: Vec<(f32, usize)> = scored
        .into_iter()
        .filter(|(score, _)| *score >= min_similarity)
        .collect();

    // Apply category diversity: max 2 per category in result set
    let mut category_counts: HashMap<String, usize> = HashMap::new();
    let mut result: Vec<(usize, f32)> = Vec::new();
    let mut remaining: Vec<(f32, usize)> = Vec::new();

    for (score, idx) in &filtered {
        let cat = memories[*idx].category().unwrap_or("other").to_string();
        let count = category_counts.entry(cat).or_insert(0);
        if *count < 2 && result.len() < limit {
            *count += 1;
            result.push((*idx, *score));
        } else {
            remaining.push((*score, *idx));
        }
    }

    // Fill remaining slots from overflow candidates
    for (score, idx) in remaining {
        if result.len() >= limit {
            break;
        }
        result.push((idx, score));
    }

    result
}

/// Select memories by pure cosine score, without category diversity bias.
/// Cold memories are included with `COLD_MEMORY_SCORE_MULTIPLIER` applied.
pub fn select_top_cosine_memory_indices<E: MemoryEntry>(
    query_embedding: &[f32],
    memories: &[E],
    limit: usize,
    min_similarity: f32,
) -> Vec<(usize, f32)> {
    let mut scored: Vec<(f32, usize)> = memories
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.embedding().is_empty())
        .map(|(i, m)| {
            let raw = cosine_similarity(query_embedding, m.embedding());
            let score = if m.is_cold() && !m.is_pinned() {
                raw * COLD_MEMORY_SCORE_MULTIPLIER
            } else {
                raw
            };
            (score, i)
        })
        .filter(|(score, _)| *score >= min_similarity)
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored
        .into_iter()
        .take(limit)
        .map(|(score, idx)| (idx, score))
        .collect()
}

/// Keyword search over cold memories. Returns indices.
pub fn search_cold_memory_indices_by_keyword<E: MemoryEntry>(
    memories: &[E],
    query: &str,
    limit: usize,
) -> Vec<usize> {
    let keywords = extract_keywords(query);
    if keywords.is_empty() {
        return Vec::new();
    }

    let mut matches: Vec<(usize, usize)> = memories
        .iter()
        .enumerate()
        .filter_map(|(idx, mem)| {
            if !mem.is_cold() {
                return None;
            }
            let text = normalize_query_text(mem.text());
            let match_count = keywords
                .iter()
                .filter(|kw| text.contains(kw.as_str()))
                .count();
            if match_count > 0 {
                Some((match_count, idx))
            } else {
                None
            }
        })
        .collect();

    matches.sort_by(|a, b| b.0.cmp(&a.0));
    matches
        .into_iter()
        .take(limit)
        .map(|(_, idx)| idx)
        .collect()
}

/// Trim non-pinned memories until `memories.len() <= max_entries`. Returns the
/// ids that were removed so callers can do a narrow `DELETE` from the
/// normalised table.
///
/// Eviction priority is a composite of `importance_score` and recency, lower = evicted
/// first. Importance dominates so an old-but-foundational memory (e.g. a character
/// backstory created early and rarely re-accessed) is preserved over a freshly-summarised
/// trivial detail. Cold memories naturally fall to the bottom of this ranking because
/// their importance was already reduced by `apply_memory_decay`.
pub fn trim_memories_to_max<E: MemoryEntry>(
    memories: &mut Vec<E>,
    max_entries: usize,
) -> Vec<String> {
    if memories.len() <= max_entries {
        return Vec::new();
    }

    const IMPORTANCE_WEIGHT: f32 = 0.7;
    const RECENCY_WEIGHT: f32 = 0.3;

    // Time bounds for recency normalisation. Only consider non-pinned memories since
    // pinned ones never get evicted regardless.
    let mut min_t = u64::MAX;
    let mut max_t = 0u64;
    for m in memories.iter().filter(|m| !m.is_pinned()) {
        let t = m.last_accessed_at();
        if t < min_t {
            min_t = t;
        }
        if t > max_t {
            max_t = t;
        }
    }
    let range = max_t.saturating_sub(min_t).max(1) as f32;

    let mut candidates: Vec<(f32, String)> = memories
        .iter()
        .filter(|m| !m.is_pinned())
        .map(|m| {
            let recency = (m.last_accessed_at().saturating_sub(min_t) as f32) / range;
            let score = m.importance_score() * IMPORTANCE_WEIGHT + recency * RECENCY_WEIGHT;
            (score, m.id().to_string())
        })
        .collect();

    // Sort ascending: lowest composite score is evicted first.
    candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut remove_ids: HashSet<String> = HashSet::new();
    let mut remaining = memories.len();

    for (_, id) in candidates {
        if remaining <= max_entries {
            break;
        }
        if remove_ids.insert(id) {
            remaining = remaining.saturating_sub(1);
        }
    }

    memories.retain(|m| !remove_ids.contains(m.id()));
    remove_ids.into_iter().collect()
}
