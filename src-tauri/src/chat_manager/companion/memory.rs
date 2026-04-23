use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tauri::AppHandle;
use tokio::sync::Mutex as TokioMutex;

use super::{companion_config, current_state, is_companion_mode};
use crate::chat_manager::memory::dynamic::{
    calculate_hot_memory_tokens, cosine_similarity, dynamic_hot_memory_token_budget,
    ensure_pinned_hot, extract_keywords, find_duplicate_memory_reason, generate_memory_id,
    normalize_query_text, search_cold_memory_indices_by_keyword,
};
use crate::chat_manager::storage::save_session;
use crate::chat_manager::types::{
    Character, MemoryEmbedding, MemoryEntityAnchor, Session, Settings, StoredMessage,
};
use crate::embedding;
use crate::embedding::emotion::{classify_text, EmotionClassification};
use crate::embedding::ner::{extract_entities, NamedEntitySpan};
use crate::utils::{log_info, log_warn, now_millis};

const CATEGORY_PROFILE: &str = "profile";
const CATEGORY_RELATIONSHIP: &str = "relationship";
const CATEGORY_PREFERENCE: &str = "preference";
const CATEGORY_BOUNDARY: &str = "boundary";
const CATEGORY_ROUTINE: &str = "routine";
const CATEGORY_EPISODIC: &str = "episodic";
const CATEGORY_MILESTONE: &str = "milestone";
const CATEGORY_EMOTIONAL_SNAPSHOT: &str = "emotional_snapshot";

lazy_static::lazy_static! {
    static ref ROUTER_RUNTIME: Arc<TokioMutex<Option<Vec<PrototypeEmbedding>>>> =
        Arc::new(TokioMutex::new(None));
}

#[derive(Debug, Clone)]
struct CompanionMemoryCandidate {
    text: String,
    category: &'static str,
    pinned: bool,
    importance: f32,
    canonical_entities: Vec<MemoryEntityAnchor>,
    fact_signature: Option<String>,
    fact_polarity: Option<i8>,
    source_role: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrototypeSpeaker {
    User,
    Assistant,
    Any,
}

impl PrototypeSpeaker {
    fn as_role_name(self) -> &'static str {
        match self {
            PrototypeSpeaker::User => "user",
            PrototypeSpeaker::Assistant => "assistant",
            PrototypeSpeaker::Any => "any",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SentencePrototype {
    category: &'static str,
    speaker: PrototypeSpeaker,
    description: &'static str,
    threshold: f32,
    importance: f32,
    pinned: bool,
}

#[derive(Debug, Clone)]
struct PrototypeEmbedding {
    prototype: SentencePrototype,
    embedding: Vec<f32>,
}

#[derive(Debug, Clone)]
struct SentenceChunk {
    text: String,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateDisposition {
    Create,
    SkipDuplicate,
}

fn config(character: &Character) -> super::CompanionMemoryConfig {
    companion_config(character).memory
}

fn memory_is_active(memory: &MemoryEmbedding) -> bool {
    memory.superseded_by.is_none()
}

fn active_memories(session: &Session) -> Vec<MemoryEmbedding> {
    session
        .memory_embeddings
        .iter()
        .filter(|memory| memory_is_active(memory))
        .cloned()
        .collect()
}

fn active_memory_texts(session: &Session) -> Vec<String> {
    session
        .memory_embeddings
        .iter()
        .filter(|memory| memory_is_active(memory))
        .map(|memory| memory.text.clone())
        .collect()
}

pub fn is_enabled(settings: &Settings, session: &Session, character: &Character) -> bool {
    let dynamic_enabled = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.dynamic_memory.as_ref())
        .map(|dynamic| dynamic.enabled)
        .unwrap_or(false);

    dynamic_enabled
        && character.memory_type.eq_ignore_ascii_case("dynamic")
        && is_companion_mode(session, character)
        && config(character).enabled
}

pub fn prompt_memory_lines(session: &Session, character: &Character) -> Vec<String> {
    if !is_companion_mode(session, character) {
        return Vec::new();
    }

    let cfg = config(character);
    let now = now_millis().unwrap_or_default();
    let mut scored = session
        .memory_embeddings
        .iter()
        .filter(|memory| memory_is_active(memory))
        .filter(|memory| !memory.is_cold || memory.is_pinned)
        .map(|memory| {
            (
                prompt_retention_score(memory, &cfg, now),
                memory.text.trim().to_string(),
            )
        })
        .filter(|(_, text)| !text.is_empty())
        .collect::<Vec<_>>();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored
        .into_iter()
        .take(cfg.retrieval_limit.max(4) as usize)
        .map(|(_, text)| format!("- {}", text))
        .collect()
}

pub async fn select_relevant_memories(
    app: &AppHandle,
    session: &Session,
    character: &Character,
    query: &str,
) -> Vec<MemoryEmbedding> {
    if !is_companion_mode(session, character) || session.memory_embeddings.is_empty() {
        return Vec::new();
    }

    let cfg = config(character);
    let state = current_state(session, &companion_config(character));
    let query = build_retrieval_query(query, &state.active_signals);
    if query.trim().is_empty() {
        return top_prompt_memories(session, character);
    }

    let query_embedding = match embedding::compute_embedding(app.clone(), query.clone()).await {
        Ok(vec) => Some(vec),
        Err(err) => {
            log_warn(
                app,
                "companion_memory",
                format!(
                    "query embedding failed; falling back to keyword scoring: {}",
                    err
                ),
            );
            None
        }
    };

    let query_keywords = extract_keywords(&query);
    let now = now_millis().unwrap_or_default();
    let mut scored = session
        .memory_embeddings
        .iter()
        .enumerate()
        .filter(|(_, memory)| memory_is_active(memory))
        .filter(|(_, memory)| !memory.is_cold || memory.is_pinned)
        .filter_map(|(index, memory)| {
            let cosine = query_embedding
                .as_ref()
                .filter(|_| !memory.embedding.is_empty())
                .map(|embedding| cosine_similarity(embedding, &memory.embedding))
                .unwrap_or(0.0);
            let keyword_overlap = keyword_overlap_score(memory, &query_keywords);
            let score = retrieval_score(memory, cosine, keyword_overlap, &cfg, &state, now);
            let passes_floor = memory.is_pinned || keyword_overlap > 0.0 || cosine >= 0.12;
            if !passes_floor {
                None
            } else {
                Some((index, score, cosine.max(keyword_overlap)))
            }
        })
        .collect::<Vec<_>>();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut category_counts: HashMap<String, usize> = HashMap::new();
    let mut selected = Vec::new();
    for (index, _, score_hint) in scored {
        if selected.len() >= cfg.retrieval_limit as usize {
            break;
        }

        let memory = match session.memory_embeddings.get(index) {
            Some(memory) => memory,
            None => continue,
        };

        let category = memory.category.as_deref().unwrap_or("other").to_string();
        let allowed_per_category = if category == CATEGORY_RELATIONSHIP {
            3
        } else {
            2
        };
        let count = category_counts.entry(category).or_insert(0);
        if *count >= allowed_per_category {
            continue;
        }
        *count += 1;

        let mut cloned = memory.clone();
        cloned.match_score = Some(score_hint);
        selected.push(cloned);
    }

    if !selected.is_empty() {
        return selected;
    }

    let normalized_query = normalize_query_text(&query);
    let active = active_memories(session);
    search_cold_memory_indices_by_keyword(&active, &normalized_query, cfg.retrieval_limit as usize)
        .into_iter()
        .filter_map(|index| active.get(index).cloned())
        .collect()
}

pub async fn process_turn(
    app: &AppHandle,
    session: &mut Session,
    settings: &Settings,
    character: &Character,
) -> Result<(), String> {
    if !is_enabled(settings, session, character) {
        return Ok(());
    }

    let cfg = config(character);
    let now = now_millis()?;
    let fixed = ensure_pinned_hot(&mut session.memory_embeddings);
    if fixed > 0 {
        log_info(
            app,
            "companion_memory",
            format!("restored {} pinned memories to hot", fixed),
        );
    }

    apply_companion_decay(&mut session.memory_embeddings);

    let state = current_state(session, &companion_config(character));
    let candidates = build_candidates(app, session, &state, &cfg).await;

    if candidates.is_empty() {
        demote_over_budget(session, settings, &cfg);
        trim_to_max_entries(session, &cfg);
        session.memories = active_memory_texts(session);
        session.updated_at = now;
        save_session(app, session)?;
        return Ok(());
    }

    let mut seen = HashSet::new();
    let mut created = 0usize;

    for candidate in candidates {
        if !seen.insert(format!(
            "{}::{}",
            candidate.category,
            normalize_query_text(&candidate.text)
        )) {
            continue;
        }

        let embedding =
            match embedding::compute_embedding(app.clone(), candidate.text.clone()).await {
                Ok(vec) => Some(vec),
                Err(err) => {
                    log_warn(
                        app,
                        "companion_memory",
                        format!(
                            "candidate embedding failed for category={} text='{}': {}",
                            candidate.category, candidate.text, err
                        ),
                    );
                    None
                }
            };

        let decision =
            resolve_candidate_write(&candidate, embedding.as_deref(), &session.memory_embeddings);

        if decision == CandidateDisposition::SkipDuplicate {
            log_info(
                app,
                "companion_memory",
                format!(
                    "skipping duplicate companion memory category={} text='{}'",
                    candidate.category, candidate.text
                ),
            );
            continue;
        }

        let superseded_indices = detect_superseded_memories(
            &candidate,
            embedding.as_deref(),
            &session.memory_embeddings,
        );

        let new_memory_id = generate_memory_id();
        let supersedes = superseded_indices
            .iter()
            .filter_map(|index| session.memory_embeddings.get(*index))
            .map(|memory| memory.id.clone())
            .collect::<Vec<_>>();

        let token_count =
            crate::embedding::tokenizer::count_tokens(app, &candidate.text).unwrap_or(0);
        session.memory_embeddings.push(MemoryEmbedding {
            id: new_memory_id.clone(),
            text: candidate.text,
            embedding: embedding.unwrap_or_default(),
            created_at: now,
            token_count,
            is_cold: false,
            last_accessed_at: now,
            importance_score: candidate.importance,
            is_pinned: candidate.pinned,
            access_count: 0,
            match_score: None,
            category: Some(candidate.category.to_string()),
            canonical_entities: candidate.canonical_entities,
            fact_signature: candidate.fact_signature,
            fact_polarity: candidate.fact_polarity,
            source_role: Some(candidate.source_role),
            superseded_by: None,
            superseded_at: None,
            supersedes: supersedes.clone(),
        });

        mark_memories_superseded(session, &superseded_indices, &new_memory_id, now);
        created += 1;
    }

    trim_to_max_entries(session, &cfg);
    demote_over_budget(session, settings, &cfg);

    session.memories = active_memory_texts(session);
    session.updated_at = now;
    save_session(app, session)?;

    log_info(
        app,
        "companion_memory",
        format!(
            "companion memory cycle complete created={} total={} hot={} cold={}",
            created,
            session.memory_embeddings.len(),
            session
                .memory_embeddings
                .iter()
                .filter(|memory| !memory.is_cold)
                .count(),
            session
                .memory_embeddings
                .iter()
                .filter(|memory| memory.is_cold)
                .count(),
        ),
    );

    Ok(())
}

fn top_prompt_memories(session: &Session, character: &Character) -> Vec<MemoryEmbedding> {
    let cfg = config(character);
    let now = now_millis().unwrap_or_default();
    let mut scored = session
        .memory_embeddings
        .iter()
        .filter(|memory| memory_is_active(memory))
        .filter(|memory| !memory.is_cold || memory.is_pinned)
        .cloned()
        .map(|memory| (prompt_retention_score(&memory, &cfg, now), memory))
        .collect::<Vec<_>>();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored
        .into_iter()
        .take(cfg.retrieval_limit as usize)
        .map(|(_, memory)| memory)
        .collect()
}

fn build_retrieval_query(query: &str, active_signals: &[String]) -> String {
    let mut parts = Vec::new();
    let trimmed = query.trim();
    if !trimmed.is_empty() {
        parts.push(trimmed.to_string());
    }

    if !active_signals.is_empty() {
        parts.push(active_signals.join(" "));
    }

    parts.join("\n")
}

fn retrieval_score(
    memory: &MemoryEmbedding,
    cosine: f32,
    keyword_overlap: f32,
    cfg: &super::CompanionMemoryConfig,
    state: &super::CompanionSessionState,
    now: u64,
) -> f32 {
    let mut score = cosine * 1.2 + keyword_overlap * 0.55;
    score += memory.importance_score * 0.35;
    score += (memory.access_count.min(6) as f32) * 0.015;
    score += recency_bonus(memory.created_at, now);
    if memory.is_pinned {
        score += 0.2;
    }

    match memory.category.as_deref().unwrap_or("other") {
        CATEGORY_RELATIONSHIP => {
            if cfg.prioritize_relationship {
                score += 0.18;
            }
            score += (state.relationship_state.trust as f32) * 0.05;
            score += (state.relationship_state.closeness as f32) * 0.05;
        }
        CATEGORY_BOUNDARY | CATEGORY_PREFERENCE => {
            score += 0.16;
        }
        CATEGORY_EPISODIC | CATEGORY_MILESTONE => {
            if cfg.prioritize_episodic {
                score += 0.12;
            }
        }
        CATEGORY_EMOTIONAL_SNAPSHOT => {
            if cfg.use_emotional_snapshots {
                score += 0.08;
            }
            score += (state.relationship_state.tension as f32) * 0.04;
        }
        CATEGORY_PROFILE => score += 0.08,
        CATEGORY_ROUTINE => score += 0.06,
        _ => {}
    }

    score
}

fn prompt_retention_score(
    memory: &MemoryEmbedding,
    cfg: &super::CompanionMemoryConfig,
    now: u64,
) -> f32 {
    if !memory_is_active(memory) {
        return 0.01;
    }

    let mut score = memory.importance_score;
    if memory.is_pinned {
        score += 2.0;
    }
    score += recency_bonus(memory.created_at, now);
    score += (memory.access_count.min(5) as f32) * 0.02;
    match memory.category.as_deref().unwrap_or("other") {
        CATEGORY_BOUNDARY | CATEGORY_PREFERENCE => score += 0.35,
        CATEGORY_RELATIONSHIP => {
            if cfg.prioritize_relationship {
                score += 0.3;
            }
        }
        CATEGORY_EPISODIC | CATEGORY_MILESTONE => {
            if cfg.prioritize_episodic {
                score += 0.16;
            }
        }
        CATEGORY_PROFILE => score += 0.18,
        CATEGORY_EMOTIONAL_SNAPSHOT => {
            if cfg.use_emotional_snapshots {
                score += 0.08;
            }
        }
        CATEGORY_ROUTINE => score += 0.1,
        _ => {}
    }
    score
}

fn recency_bonus(created_at: u64, now: u64) -> f32 {
    if created_at == 0 || now <= created_at {
        return 0.0;
    }
    let age_hours = (now - created_at) as f32 / 3_600_000.0;
    if age_hours <= 12.0 {
        0.1
    } else if age_hours <= 72.0 {
        0.05
    } else if age_hours <= 240.0 {
        0.02
    } else {
        0.0
    }
}

fn keyword_overlap_score(memory: &MemoryEmbedding, query_keywords: &[String]) -> f32 {
    if query_keywords.is_empty() {
        return 0.0;
    }

    let normalized = normalize_query_text(&memory.text);
    let matches = query_keywords
        .iter()
        .filter(|keyword| normalized.contains(keyword.as_str()))
        .count();
    if matches == 0 {
        0.0
    } else {
        (matches as f32) / (query_keywords.len() as f32)
    }
}

async fn build_candidates(
    app: &AppHandle,
    session: &Session,
    state: &super::CompanionSessionState,
    cfg: &super::CompanionMemoryConfig,
) -> Vec<CompanionMemoryCandidate> {
    let recent_messages = recent_conversation_messages(session, 6);
    let mut candidates = Vec::new();

    for message in recent_messages {
        let speaker = if message.role.eq_ignore_ascii_case("user") {
            PrototypeSpeaker::User
        } else if message.role.eq_ignore_ascii_case("assistant") {
            PrototypeSpeaker::Assistant
        } else {
            continue;
        };

        let message_entities = extract_message_entities(app, &message.content).await;
        for sentence in split_sentences(&message.content) {
            let sentence_entities =
                entities_for_sentence(&message_entities, sentence.start, sentence.end);
            let canonical_entities = canonicalize_candidate_entities(session, &sentence_entities);
            let features = SentenceFeatures::new(&sentence.text, &sentence_entities);
            if let Some(candidate) = route_sentence_candidate(
                app,
                speaker,
                &sentence.text,
                &features,
                &canonical_entities,
            )
            .await
            {
                candidates.push(candidate);
            }
        }
    }

    if cfg.use_emotional_snapshots {
        if let Some(snapshot) = emotional_snapshot_candidate(state) {
            candidates.push(snapshot);
        }
    }

    candidates
}

fn recent_conversation_messages(session: &Session, limit: usize) -> Vec<&StoredMessage> {
    session
        .messages
        .iter()
        .rev()
        .filter(|message| {
            message.role.eq_ignore_ascii_case("user")
                || message.role.eq_ignore_ascii_case("assistant")
        })
        .take(limit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn split_sentences(text: &str) -> Vec<SentenceChunk> {
    let mut chunks = Vec::new();
    let mut sentence_start: Option<usize> = None;

    for (index, ch) in text.char_indices() {
        if sentence_start.is_none() && !ch.is_whitespace() {
            sentence_start = Some(index);
        }

        if matches!(ch, '\n' | '.' | '!' | '?' | ';') {
            if let Some(start) = sentence_start.take() {
                if let Some(chunk) = build_sentence_chunk(text, start, index) {
                    chunks.push(chunk);
                }
            }
        }
    }

    if let Some(start) = sentence_start {
        if let Some(chunk) = build_sentence_chunk(text, start, text.len()) {
            chunks.push(chunk);
        }
    }

    chunks
}

fn build_sentence_chunk(text: &str, start: usize, end: usize) -> Option<SentenceChunk> {
    let raw = text.get(start..end)?;
    let trimmed_start = raw.len().saturating_sub(raw.trim_start().len());
    let trimmed_end = raw.len().saturating_sub(raw.trim_end().len());
    let normalized_start = start + trimmed_start;
    let normalized_end = end.saturating_sub(trimmed_end);
    let normalized = collapse_whitespace(text.get(normalized_start..normalized_end)?);
    if normalized.len() < 12 || normalized.len() > 220 {
        return None;
    }

    Some(SentenceChunk {
        text: normalized,
        start: normalized_start,
        end: normalized_end,
    })
}

async fn extract_message_entities(app: &AppHandle, text: &str) -> Vec<NamedEntitySpan> {
    match extract_entities(app, text).await {
        Ok(Some(entities)) => entities,
        Ok(None) => Vec::new(),
        Err(err) => {
            log_warn(
                app,
                "companion_memory",
                format!("message NER extraction failed: {}", err),
            );
            Vec::new()
        }
    }
}

fn entities_for_sentence(
    entities: &[NamedEntitySpan],
    start: usize,
    end: usize,
) -> Vec<NamedEntitySpan> {
    entities
        .iter()
        .filter(|entity| entity.start < end && entity.end > start)
        .cloned()
        .collect()
}

fn canonicalize_candidate_entities(
    session: &Session,
    entities: &[NamedEntitySpan],
) -> Vec<MemoryEntityAnchor> {
    let existing = session
        .memory_embeddings
        .iter()
        .flat_map(|memory| memory.canonical_entities.iter())
        .cloned()
        .collect::<Vec<_>>();

    let mut seen = HashSet::new();
    let mut canonical = Vec::new();
    for entity in entities {
        let normalized_surface = normalize_entity_surface(&entity.text);
        if normalized_surface.is_empty() {
            continue;
        }

        let matched = existing
            .iter()
            .filter(|anchor| anchor.label.eq_ignore_ascii_case(&entity.label))
            .filter_map(|anchor| {
                let score = entity_alias_score(&normalized_surface, &anchor.surface);
                (score >= 0.78).then_some((anchor, score))
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let (canonical_key, canonical_name) = if let Some((anchor, _)) = matched {
            (
                anchor.canonical_key.clone(),
                choose_canonical_name(&entity.text, &anchor.canonical_name),
            )
        } else {
            (
                build_entity_canonical_key(&entity.label, &normalized_surface),
                collapse_whitespace(entity.text.trim()),
            )
        };

        let dedupe_key = format!("{}::{}", entity.label.to_uppercase(), canonical_key);
        if !seen.insert(dedupe_key) {
            continue;
        }

        canonical.push(MemoryEntityAnchor {
            label: entity.label.to_uppercase(),
            surface: collapse_whitespace(entity.text.trim()),
            canonical_key,
            canonical_name,
            confidence: entity.score,
        });
    }

    canonical
}

fn normalize_entity_surface(text: &str) -> String {
    normalize_query_text(text)
        .split_whitespace()
        .filter(|token| !matches!(*token, "a" | "an" | "the" | "my" | "our" | "their"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn build_entity_canonical_key(label: &str, normalized_surface: &str) -> String {
    format!(
        "{}:{}",
        label.to_lowercase(),
        normalized_surface.replace(' ', "_")
    )
}

fn entity_alias_score(a: &str, b: &str) -> f32 {
    let normalized_b = normalize_entity_surface(b);
    if a == normalized_b {
        return 1.0;
    }

    let a_tokens = a.split_whitespace().collect::<HashSet<_>>();
    let b_tokens = normalized_b.split_whitespace().collect::<HashSet<_>>();
    if a_tokens.is_empty() || b_tokens.is_empty() {
        return 0.0;
    }

    let overlap = a_tokens.intersection(&b_tokens).count() as f32;
    let union = a_tokens.union(&b_tokens).count() as f32;
    let jaccard = if union > 0.0 { overlap / union } else { 0.0 };

    if (a.contains(&normalized_b) || normalized_b.contains(a)) && jaccard >= 0.5 {
        return 0.88;
    }

    jaccard
}

fn choose_canonical_name(candidate: &str, existing: &str) -> String {
    let candidate = collapse_whitespace(candidate.trim());
    let existing = collapse_whitespace(existing.trim());
    if candidate.len() > existing.len() {
        candidate
    } else {
        existing
    }
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Debug, Clone)]
struct SentenceFeatures {
    tokens: Vec<String>,
    entity_labels: HashSet<String>,
}

impl SentenceFeatures {
    fn new(sentence: &str, entities: &[NamedEntitySpan]) -> Self {
        Self {
            tokens: normalize_query_text(sentence)
                .split_whitespace()
                .map(|token| token.to_string())
                .collect(),
            entity_labels: entities
                .iter()
                .map(|entity| entity.label.to_uppercase())
                .collect(),
        }
    }

    fn contains_any_token(&self, tokens: &[&str]) -> bool {
        tokens
            .iter()
            .any(|token| self.tokens.iter().any(|existing| existing == token))
    }

    fn has_first_person(&self) -> bool {
        self.contains_any_token(&["i", "me", "my", "mine", "im", "ive", "ill"])
    }

    fn has_second_or_shared_reference(&self) -> bool {
        self.contains_any_token(&["you", "your", "yours", "we", "us", "our"])
    }

    fn has_entity_type(&self, label: &str) -> bool {
        self.entity_labels.contains(label)
    }

    fn has_any_entity_type(&self, labels: &[&str]) -> bool {
        labels.iter().any(|label| self.has_entity_type(label))
    }

    fn entity_type_count(&self) -> usize {
        self.entity_labels.len()
    }
}

fn router_prototypes() -> Vec<SentencePrototype> {
    vec![
        SentencePrototype {
            category: CATEGORY_BOUNDARY,
            speaker: PrototypeSpeaker::User,
            description:
                "a person sets a personal limit, refusal, comfort boundary, or asks the other person to stop something",
            threshold: 0.34,
            importance: 1.0,
            pinned: true,
        },
        SentencePrototype {
            category: CATEGORY_PREFERENCE,
            speaker: PrototypeSpeaker::User,
            description:
                "a person states a personal preference, liking, dislike, favorite, or what they enjoy",
            threshold: 0.33,
            importance: 0.94,
            pinned: false,
        },
        SentencePrototype {
            category: CATEGORY_PROFILE,
            speaker: PrototypeSpeaker::User,
            description:
                "a person states a stable fact about their identity, background, work, home, history, or life circumstances",
            threshold: 0.31,
            importance: 0.9,
            pinned: false,
        },
        SentencePrototype {
            category: CATEGORY_ROUTINE,
            speaker: PrototypeSpeaker::User,
            description:
                "a person describes a recurring habit, routine, schedule, or usual behavior",
            threshold: 0.34,
            importance: 0.82,
            pinned: false,
        },
        SentencePrototype {
            category: CATEGORY_EPISODIC,
            speaker: PrototypeSpeaker::Any,
            description:
                "people make a plan, promise, future commitment, or discuss an upcoming shared action",
            threshold: 0.34,
            importance: 0.9,
            pinned: false,
        },
        SentencePrototype {
            category: CATEGORY_RELATIONSHIP,
            speaker: PrototypeSpeaker::Any,
            description:
                "a person expresses affection, trust, gratitude, apology, reassurance, emotional closeness, or care toward the other person",
            threshold: 0.33,
            importance: 0.92,
            pinned: false,
        },
        SentencePrototype {
            category: CATEGORY_MILESTONE,
            speaker: PrototypeSpeaker::Any,
            description:
                "a major turning point in the relationship such as reconciliation, breakup, confession, deep promise, or changed relationship status",
            threshold: 0.35,
            importance: 0.96,
            pinned: true,
        },
    ]
}

async fn prototype_embeddings(app: &AppHandle) -> Result<Vec<PrototypeEmbedding>, String> {
    let mut cache = ROUTER_RUNTIME.lock().await;
    if let Some(runtime) = cache.as_ref() {
        return Ok(runtime.clone());
    }

    let mut embedded = Vec::new();
    for prototype in router_prototypes() {
        let embedding =
            embedding::compute_embedding(app.clone(), prototype.description.to_string()).await?;
        embedded.push(PrototypeEmbedding {
            prototype,
            embedding,
        });
    }

    *cache = Some(embedded.clone());
    Ok(embedded)
}

async fn classify_sentence_emotion(
    app: &AppHandle,
    sentence: &str,
) -> Option<EmotionClassification> {
    if sentence.len() < 12 {
        return None;
    }

    match classify_text(app, sentence).await {
        Ok(result) => result,
        Err(err) => {
            log_warn(
                app,
                "companion_memory",
                format!("sentence emotion classification failed: {}", err),
            );
            None
        }
    }
}

async fn route_sentence_candidate(
    app: &AppHandle,
    speaker: PrototypeSpeaker,
    sentence: &str,
    features: &SentenceFeatures,
    entities: &[MemoryEntityAnchor],
) -> Option<CompanionMemoryCandidate> {
    let sentence_embedding =
        match embedding::compute_embedding(app.clone(), sentence.to_string()).await {
            Ok(embedding) => embedding,
            Err(err) => {
                log_warn(
                    app,
                    "companion_memory",
                    format!("sentence embedding failed: {}", err),
                );
                return None;
            }
        };

    let emotion = classify_sentence_emotion(app, sentence).await;
    let prototypes = match prototype_embeddings(app).await {
        Ok(prototypes) => prototypes,
        Err(err) => {
            log_warn(
                app,
                "companion_memory",
                format!("prototype embedding initialization failed: {}", err),
            );
            return None;
        }
    };

    let mut best: Option<(SentencePrototype, f32)> = None;
    for prototype in prototypes {
        if !prototype_matches_speaker(prototype.prototype.speaker, speaker) {
            continue;
        }
        if !prototype_matches_structure(prototype.prototype.category, features, speaker) {
            continue;
        }

        let mut score = cosine_similarity(&sentence_embedding, &prototype.embedding);
        if prototype.prototype.category == CATEGORY_RELATIONSHIP
            || prototype.prototype.category == CATEGORY_MILESTONE
        {
            score += relationship_emotion_strength(emotion.as_ref()) * 0.22;
            score += apology_emotion_strength(emotion.as_ref()) * 0.12;
        }
        score += entity_signal_bonus(prototype.prototype.category, features, emotion.as_ref());

        if score < prototype.prototype.threshold {
            continue;
        }

        match best {
            Some((_, best_score)) if score <= best_score => {}
            _ => best = Some((prototype.prototype, score)),
        }
    }

    let (prototype, score) = best?;
    let pinned = prototype.pinned
        || ((prototype.category == CATEGORY_RELATIONSHIP
            || prototype.category == CATEGORY_MILESTONE)
            && relationship_emotion_strength(emotion.as_ref()) >= 0.75);
    let importance = if score > 0.72 {
        (prototype.importance + 0.04).min(1.0)
    } else {
        prototype.importance
    };

    Some(candidate(
        prototype.category,
        format_memory_text(prototype.category, speaker, sentence, entities),
        pinned,
        importance,
        entities.to_vec(),
        derive_fact_signature(prototype.category, sentence, entities),
        derive_fact_polarity(prototype.category, sentence),
        speaker.as_role_name().to_string(),
    ))
}

fn prototype_matches_speaker(expected: PrototypeSpeaker, actual: PrototypeSpeaker) -> bool {
    expected == PrototypeSpeaker::Any || expected == actual
}

fn prototype_matches_structure(
    category: &str,
    features: &SentenceFeatures,
    speaker: PrototypeSpeaker,
) -> bool {
    match category {
        CATEGORY_BOUNDARY | CATEGORY_PREFERENCE | CATEGORY_PROFILE | CATEGORY_ROUTINE => {
            speaker == PrototypeSpeaker::User && features.has_first_person()
        }
        CATEGORY_EPISODIC => {
            features.has_first_person() || features.has_second_or_shared_reference()
        }
        CATEGORY_RELATIONSHIP | CATEGORY_MILESTONE => {
            features.has_first_person() && features.has_second_or_shared_reference()
        }
        _ => true,
    }
}

fn entity_signal_bonus(
    category: &str,
    features: &SentenceFeatures,
    emotion: Option<&EmotionClassification>,
) -> f32 {
    let distinct_entity_types = features.entity_type_count().min(3) as f32;
    match category {
        CATEGORY_PROFILE => {
            if features.has_any_entity_type(&["PER", "ORG", "LOC", "DATE"]) {
                0.05 + distinct_entity_types * 0.025
            } else {
                0.0
            }
        }
        CATEGORY_ROUTINE => {
            if features.has_any_entity_type(&["DATE", "LOC", "ORG"]) {
                0.04 + distinct_entity_types * 0.02
            } else {
                0.0
            }
        }
        CATEGORY_EPISODIC => {
            if features.has_any_entity_type(&["DATE", "LOC", "PER", "ORG"]) {
                0.06 + distinct_entity_types * 0.03
            } else {
                0.0
            }
        }
        CATEGORY_MILESTONE => {
            let mut bonus = 0.0;
            if features.has_any_entity_type(&["DATE", "LOC"]) {
                bonus += 0.05;
            }
            if relationship_emotion_strength(emotion) >= 0.55 {
                bonus += 0.03;
            }
            bonus
        }
        _ => 0.0,
    }
}

fn derive_fact_signature(
    category: &str,
    sentence: &str,
    entities: &[MemoryEntityAnchor],
) -> Option<String> {
    match category {
        CATEGORY_BOUNDARY | CATEGORY_PREFERENCE | CATEGORY_PROFILE | CATEGORY_ROUTINE => {
            let topic = normalized_topic_key(sentence);
            if topic.is_empty() {
                None
            } else {
                Some(format!("{}::{}", category, topic))
            }
        }
        CATEGORY_EPISODIC | CATEGORY_MILESTONE => {
            let entity_key = entities
                .iter()
                .map(|entity| entity.canonical_key.clone())
                .take(3)
                .collect::<Vec<_>>()
                .join("|");
            let topic = normalized_topic_key(sentence);
            if entity_key.is_empty() && topic.is_empty() {
                None
            } else if entity_key.is_empty() {
                Some(format!("{}::{}", category, topic))
            } else if topic.is_empty() {
                Some(format!("{}::{}", category, entity_key))
            } else {
                Some(format!("{}::{}::{}", category, entity_key, topic))
            }
        }
        _ => None,
    }
}

fn normalized_topic_key(sentence: &str) -> String {
    normalize_query_text(sentence)
        .split_whitespace()
        .filter(|token| {
            !matches!(
                *token,
                "i" | "im"
                    | "ive"
                    | "ill"
                    | "me"
                    | "my"
                    | "mine"
                    | "you"
                    | "your"
                    | "yours"
                    | "we"
                    | "our"
                    | "us"
                    | "the"
                    | "a"
                    | "an"
                    | "to"
                    | "for"
                    | "of"
                    | "and"
                    | "but"
                    | "that"
                    | "this"
                    | "it"
                    | "is"
                    | "are"
                    | "was"
                    | "were"
                    | "be"
                    | "been"
                    | "being"
                    | "do"
                    | "does"
                    | "did"
                    | "have"
                    | "has"
                    | "had"
                    | "would"
                    | "could"
                    | "should"
                    | "can"
                    | "will"
                    | "just"
                    | "really"
                    | "very"
            )
        })
        .take(8)
        .collect::<Vec<_>>()
        .join(" ")
}

fn derive_fact_polarity(category: &str, sentence: &str) -> Option<i8> {
    match category {
        CATEGORY_BOUNDARY | CATEGORY_PREFERENCE | CATEGORY_PROFILE | CATEGORY_ROUTINE
        | CATEGORY_EPISODIC => {
            let normalized = normalize_query_text(sentence);
            let has_negative = normalized.split_whitespace().any(|token| {
                matches!(
                    token,
                    "not"
                        | "dont"
                        | "never"
                        | "no"
                        | "cant"
                        | "wont"
                        | "dislike"
                        | "hate"
                        | "avoid"
                        | "stop"
                        | "against"
                        | "refuse"
                )
            });
            Some(if has_negative { -1 } else { 1 })
        }
        _ => None,
    }
}

fn relationship_emotion_strength(emotion: Option<&EmotionClassification>) -> f32 {
    emotion
        .map(|emotion| {
            emotion
                .labels
                .iter()
                .filter_map(|label| match label.label.as_str() {
                    "love" | "caring" | "gratitude" | "admiration" | "approval" | "desire" => {
                        Some(label.score)
                    }
                    _ => None,
                })
                .fold(0.0_f32, f32::max)
        })
        .unwrap_or(0.0)
}

fn apology_emotion_strength(emotion: Option<&EmotionClassification>) -> f32 {
    emotion
        .map(|emotion| {
            emotion
                .labels
                .iter()
                .filter_map(|label| match label.label.as_str() {
                    "remorse" | "sadness" | "grief" => Some(label.score),
                    _ => None,
                })
                .fold(0.0_f32, f32::max)
        })
        .unwrap_or(0.0)
}

fn format_memory_text(
    category: &str,
    speaker: PrototypeSpeaker,
    sentence: &str,
    entities: &[MemoryEntityAnchor],
) -> String {
    let label = match (category, speaker) {
        (CATEGORY_BOUNDARY, _) => "User boundary",
        (CATEGORY_PREFERENCE, _) => "User preference",
        (CATEGORY_PROFILE, _) => "User fact",
        (CATEGORY_ROUTINE, _) => "User routine",
        (CATEGORY_EPISODIC, PrototypeSpeaker::Assistant) => "Companion commitment",
        (CATEGORY_EPISODIC, _) => "Shared plan or promise",
        (CATEGORY_RELATIONSHIP, PrototypeSpeaker::Assistant) => "Companion relationship signal",
        (CATEGORY_RELATIONSHIP, _) => "Relationship signal",
        (CATEGORY_MILESTONE, _) => "Relationship milestone",
        _ => "Companion memory",
    };

    let mut text = format!("{}: {}.", label, trim_trailing_punctuation(sentence));
    if let Some(summary) = summarize_entities(entities) {
        text.push_str(" Key entities: ");
        text.push_str(&summary);
        text.push('.');
    }
    text
}

fn summarize_entities(entities: &[MemoryEntityAnchor]) -> Option<String> {
    let mut seen = HashSet::new();
    let mut ranked = entities
        .iter()
        .filter_map(|entity| {
            let normalized = collapse_whitespace(entity.canonical_name.trim());
            if normalized.is_empty() {
                return None;
            }
            let dedupe = format!("{}::{}", entity.label, normalized.to_lowercase());
            if !seen.insert(dedupe) {
                return None;
            }
            Some((entity.confidence, entity.label.as_str(), normalized))
        })
        .collect::<Vec<_>>();

    if ranked.is_empty() {
        return None;
    }

    ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    Some(
        ranked
            .into_iter()
            .take(3)
            .map(|(_, label, text)| format!("{} [{}]", text, label))
            .collect::<Vec<_>>()
            .join(", "),
    )
}

fn emotional_snapshot_candidate(
    state: &super::CompanionSessionState,
) -> Option<CompanionMemoryCandidate> {
    let mut fragments = Vec::new();
    if state.relationship_state.tension >= 0.62 {
        fragments.push("the exchange carried elevated tension");
    }
    if state.relationship_state.affection >= 0.72 && state.relationship_state.trust >= 0.65 {
        fragments.push("the relationship felt notably warm and trusting");
    }
    if state.emotional_state.expressed.reassurance_need >= 0.6 {
        fragments.push("reassurance need was high");
    }
    if state.emotional_state.expressed.vulnerability >= 0.58 {
        fragments.push("vulnerability was unusually visible");
    }

    if fragments.is_empty() {
        return None;
    }

    Some(candidate(
        CATEGORY_EMOTIONAL_SNAPSHOT,
        format!("Recent emotional tone: {}.", fragments.join(", ")),
        false,
        0.72,
        Vec::new(),
        None,
        None,
        "system".to_string(),
    ))
}

fn candidate(
    category: &'static str,
    text: String,
    pinned: bool,
    importance: f32,
    canonical_entities: Vec<MemoryEntityAnchor>,
    fact_signature: Option<String>,
    fact_polarity: Option<i8>,
    source_role: String,
) -> CompanionMemoryCandidate {
    CompanionMemoryCandidate {
        text: clamp_memory_text(&text),
        category,
        pinned,
        importance,
        canonical_entities,
        fact_signature,
        fact_polarity,
        source_role,
    }
}

fn resolve_candidate_write(
    candidate: &CompanionMemoryCandidate,
    embedding: Option<&[f32]>,
    existing_memories: &[MemoryEmbedding],
) -> CandidateDisposition {
    if let Some(reason) = find_duplicate_memory_reason(
        &candidate.text,
        embedding,
        &active_memory_slice(existing_memories),
    ) {
        let _ = reason;
        return CandidateDisposition::SkipDuplicate;
    }

    let candidate_signature = candidate.fact_signature.as_deref();
    let candidate_topic = normalized_topic_key(&candidate.text);
    let candidate_polarity = candidate.fact_polarity;

    for memory in existing_memories
        .iter()
        .filter(|memory| memory_is_active(memory))
    {
        if memory.category.as_deref() != Some(candidate.category) {
            continue;
        }

        let existing_signature = memory_signature(memory);
        let exact_signature_match = candidate_signature
            .zip(existing_signature.as_deref())
            .is_some_and(|(a, b)| a == b);
        let topic_overlap = fact_topic_overlap(memory, &candidate_topic);
        let entity_overlap = canonical_entity_overlap(memory, &candidate.canonical_entities);
        let cosine = embedding
            .filter(|_| !memory.embedding.is_empty())
            .map(|value| cosine_similarity(value, &memory.embedding))
            .unwrap_or(0.0);

        if exact_signature_match
            && candidate_polarity == memory.fact_polarity
            && (cosine >= 0.74 || topic_overlap >= 0.72 || entity_overlap >= 0.8)
        {
            return CandidateDisposition::SkipDuplicate;
        }
    }

    CandidateDisposition::Create
}

fn detect_superseded_memories(
    candidate: &CompanionMemoryCandidate,
    embedding: Option<&[f32]>,
    existing_memories: &[MemoryEmbedding],
) -> Vec<usize> {
    let mut superseded = Vec::new();
    let candidate_signature = candidate.fact_signature.as_deref();
    let candidate_topic = normalized_topic_key(&candidate.text);
    let candidate_polarity = candidate.fact_polarity;

    for (index, memory) in existing_memories.iter().enumerate() {
        if !memory_is_active(memory) || memory.category.as_deref() != Some(candidate.category) {
            continue;
        }

        let existing_signature = memory_signature(memory);
        let exact_signature_match = candidate_signature
            .zip(existing_signature.as_deref())
            .is_some_and(|(a, b)| a == b);
        let polarity_conflict = candidate_polarity
            .zip(memory.fact_polarity)
            .is_some_and(|(a, b)| a != b);
        let topic_overlap = fact_topic_overlap(memory, &candidate_topic);
        let entity_overlap = canonical_entity_overlap(memory, &candidate.canonical_entities);
        let cosine = embedding
            .filter(|_| !memory.embedding.is_empty())
            .map(|value| cosine_similarity(value, &memory.embedding))
            .unwrap_or(0.0);

        let should_supersede = if exact_signature_match && polarity_conflict {
            true
        } else if exact_signature_match
            && cosine >= 0.64
            && candidate.text.len() > memory.text.len()
        {
            true
        } else {
            polarity_conflict && (topic_overlap >= 0.72 || entity_overlap >= 0.75) && cosine >= 0.52
        };

        if should_supersede {
            superseded.push(index);
        }
    }

    superseded
}

fn mark_memories_superseded(
    session: &mut Session,
    indices: &[usize],
    replacement_id: &str,
    now: u64,
) {
    for index in indices {
        if let Some(memory) = session.memory_embeddings.get_mut(*index) {
            memory.superseded_by = Some(replacement_id.to_string());
            memory.superseded_at = Some(now);
            memory.is_cold = true;
            memory.importance_score = memory.importance_score.min(0.08);
        }
    }
}

fn active_memory_slice(memories: &[MemoryEmbedding]) -> Vec<MemoryEmbedding> {
    memories
        .iter()
        .filter(|memory| memory_is_active(memory))
        .cloned()
        .collect()
}

fn memory_signature(memory: &MemoryEmbedding) -> Option<String> {
    memory.fact_signature.clone().or_else(|| {
        memory
            .category
            .as_deref()
            .and_then(|category| derive_fact_signature(category, &memory_content_text(memory), &[]))
    })
}

fn memory_content_text(memory: &MemoryEmbedding) -> String {
    let without_entities = memory
        .text
        .split(" Key entities:")
        .next()
        .unwrap_or(memory.text.as_str())
        .trim();

    without_entities
        .split_once(':')
        .map(|(_, content)| content.trim().to_string())
        .unwrap_or_else(|| without_entities.to_string())
}

fn fact_topic_overlap(memory: &MemoryEmbedding, candidate_topic: &str) -> f32 {
    if candidate_topic.is_empty() {
        return 0.0;
    }

    let existing_topic = normalized_topic_key(&memory_content_text(memory));
    if existing_topic.is_empty() {
        return 0.0;
    }

    let candidate_tokens = candidate_topic.split_whitespace().collect::<HashSet<_>>();
    let existing_tokens = existing_topic.split_whitespace().collect::<HashSet<_>>();
    if candidate_tokens.is_empty() || existing_tokens.is_empty() {
        return 0.0;
    }

    let overlap = candidate_tokens.intersection(&existing_tokens).count() as f32;
    let union = candidate_tokens.union(&existing_tokens).count() as f32;
    if union == 0.0 {
        0.0
    } else {
        overlap / union
    }
}

fn canonical_entity_overlap(
    memory: &MemoryEmbedding,
    candidate_entities: &[MemoryEntityAnchor],
) -> f32 {
    if memory.canonical_entities.is_empty() || candidate_entities.is_empty() {
        return 0.0;
    }

    let existing = memory
        .canonical_entities
        .iter()
        .map(|entity| format!("{}::{}", entity.label, entity.canonical_key))
        .collect::<HashSet<_>>();
    let candidate = candidate_entities
        .iter()
        .map(|entity| format!("{}::{}", entity.label, entity.canonical_key))
        .collect::<HashSet<_>>();
    let overlap = existing.intersection(&candidate).count() as f32;
    let union = existing.union(&candidate).count() as f32;
    if union == 0.0 {
        0.0
    } else {
        overlap / union
    }
}

fn clamp_memory_text(text: &str) -> String {
    let mut normalized = collapse_whitespace(text);
    if normalized.len() > 260 {
        normalized.truncate(260);
        normalized = normalized
            .trim_end_matches(|ch: char| ch.is_whitespace())
            .to_string();
    }
    normalized
}

fn trim_trailing_punctuation(text: &str) -> String {
    text.trim()
        .trim_end_matches(|ch: char| ch == '.' || ch == '!' || ch == '?' || ch == ';')
        .to_string()
}

fn apply_companion_decay(memories: &mut [MemoryEmbedding]) {
    for memory in memories.iter_mut() {
        if !memory_is_active(memory) {
            memory.is_cold = true;
            memory.importance_score = memory.importance_score.min(0.05);
            continue;
        }

        if memory.is_pinned || memory.is_cold {
            continue;
        }

        let (decay, cold_threshold) = decay_profile(memory.category.as_deref());
        memory.importance_score = (memory.importance_score - decay).max(0.05);
        if memory.importance_score < cold_threshold {
            memory.is_cold = true;
        }
    }
}

fn decay_profile(category: Option<&str>) -> (f32, f32) {
    match category.unwrap_or("other") {
        CATEGORY_BOUNDARY => (0.004, 0.28),
        CATEGORY_PREFERENCE => (0.006, 0.3),
        CATEGORY_PROFILE => (0.007, 0.32),
        CATEGORY_RELATIONSHIP => (0.008, 0.3),
        CATEGORY_ROUTINE => (0.012, 0.33),
        CATEGORY_EPISODIC => (0.018, 0.35),
        CATEGORY_MILESTONE => (0.012, 0.32),
        CATEGORY_EMOTIONAL_SNAPSHOT => (0.03, 0.38),
        _ => (0.015, 0.34),
    }
}

fn trim_to_max_entries(session: &mut Session, cfg: &super::CompanionMemoryConfig) {
    let max_entries = cfg.max_entries as usize;
    if session.memory_embeddings.len() <= max_entries {
        return;
    }

    let now = now_millis().unwrap_or_default();
    let mut scored = session
        .memory_embeddings
        .iter()
        .enumerate()
        .map(|(index, memory)| (index, prompt_retention_score(memory, cfg, now)))
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let keep = scored
        .into_iter()
        .take(max_entries)
        .map(|(index, _)| index)
        .collect::<HashSet<_>>();
    session.memory_embeddings = session
        .memory_embeddings
        .iter()
        .cloned()
        .enumerate()
        .filter_map(|(index, memory)| keep.contains(&index).then_some(memory))
        .collect();
}

fn demote_over_budget(
    session: &mut Session,
    settings: &Settings,
    cfg: &super::CompanionMemoryConfig,
) {
    let budget = dynamic_hot_memory_token_budget(settings);
    if calculate_hot_memory_tokens(&session.memory_embeddings) <= budget {
        return;
    }

    let now = now_millis().unwrap_or_default();
    let mut demotable = session
        .memory_embeddings
        .iter()
        .enumerate()
        .filter(|(_, memory)| !memory.is_pinned && !memory.is_cold)
        .map(|(index, memory)| (index, prompt_retention_score(memory, cfg, now)))
        .collect::<Vec<_>>();
    demotable.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    for (index, _) in demotable {
        if calculate_hot_memory_tokens(&session.memory_embeddings) <= budget {
            break;
        }
        if let Some(memory) = session.memory_embeddings.get_mut(index) {
            memory.is_cold = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_boundary_sentence_creates_boundary_candidate() {
        let features = SentenceFeatures::new(
            "Please don't call me by my full name when I'm stressed",
            &[],
        );
        assert!(prototype_matches_structure(
            CATEGORY_BOUNDARY,
            &features,
            PrototypeSpeaker::User
        ));
    }

    #[test]
    fn fact_signature_normalizes_preference_topic() {
        let signature = derive_fact_signature(
            CATEGORY_PREFERENCE,
            "I really like quiet cafes in the evening",
            &[],
        );

        assert_eq!(
            signature.as_deref(),
            Some("preference::like quiet cafes in evening")
        );
    }

    #[test]
    fn contradiction_detector_supersedes_opposite_preference() {
        let existing = MemoryEmbedding {
            id: "old".to_string(),
            text: "User preference: I like quiet cafes.".to_string(),
            embedding: vec![1.0, 0.0],
            created_at: 0,
            token_count: 0,
            is_cold: false,
            last_accessed_at: 0,
            importance_score: 1.0,
            is_pinned: false,
            access_count: 0,
            match_score: None,
            category: Some(CATEGORY_PREFERENCE.to_string()),
            canonical_entities: Vec::new(),
            fact_signature: Some("preference::like quiet cafes".to_string()),
            fact_polarity: Some(1),
            source_role: Some("user".to_string()),
            superseded_by: None,
            superseded_at: None,
            supersedes: Vec::new(),
        };

        let candidate = CompanionMemoryCandidate {
            text: "User preference: I do not like quiet cafes.".to_string(),
            category: CATEGORY_PREFERENCE,
            pinned: false,
            importance: 0.9,
            canonical_entities: Vec::new(),
            fact_signature: Some("preference::like quiet cafes".to_string()),
            fact_polarity: Some(-1),
            source_role: "user".to_string(),
        };

        let superseded = detect_superseded_memories(&candidate, Some(&[0.96, 0.04]), &[existing]);
        assert_eq!(superseded, vec![0]);
    }

    #[test]
    fn emotional_snapshot_only_created_for_salient_state() {
        let snapshot = emotional_snapshot_candidate(&super::super::CompanionSessionState {
            relationship_state: super::super::RelationshipState {
                affection: 0.76,
                trust: 0.71,
                ..Default::default()
            },
            ..Default::default()
        });

        assert!(snapshot.is_some());
    }
}
