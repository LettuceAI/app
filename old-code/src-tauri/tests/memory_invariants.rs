//! Algorithmic invariants for memory decay / trim / selection.
//! These are properties that must hold across many random-looking inputs.
//! Catches regressions where someone "simplifies" the logic and breaks
//! a safety property (e.g. evicting pinned memories).

use lettuceai_lib::chat_manager::memory::dynamic;
use lettuceai_lib::chat_manager::types::MemoryEmbedding;

fn make_mem(id: &str, score: f32, pinned: bool, access_count: u32) -> MemoryEmbedding {
    MemoryEmbedding {
        id: id.into(),
        text: format!("memory {id}"),
        embedding: vec![0.0; 4],
        created_at: 1_700_000_000_000,
        token_count: 10,
        is_cold: false,
        last_accessed_at: 1_700_000_000_000,
        importance_score: score,
        persistence_importance: 1.0,
        prompt_importance: 1.0,
        volatility: 0.4,
        is_pinned: pinned,
        access_count,
        embedding_source_version: None,
        embedding_dimensions: None,
        match_score: None,
        category: None,
        observed_at: None,
        observed_time_precision: None,
        canonical_entities: vec![],
        fact_signature: None,
        fact_polarity: None,
        source_role: None,
        source_message_id: None,
        superseded_by: None,
        superseded_at: None,
        supersedes: vec![],
    }
}

fn batch(n: usize) -> Vec<MemoryEmbedding> {
    (0..n)
        .map(|i| {
            // Deterministic pseudo-random values via linear congruential mixing.
            let seed = (i as u32).wrapping_mul(2654435761);
            let score = ((seed % 1000) as f32) / 1000.0;
            let pinned = i % 7 == 0;
            let access_count = seed % 30;
            make_mem(&format!("m{i:04}"), score, pinned, access_count)
        })
        .collect()
}

#[test]
fn decay_never_increases_importance() {
    for n in [1, 5, 25, 100] {
        let mut mems = batch(n);
        let before: Vec<f32> = mems.iter().map(|m| m.importance_score).collect();
        dynamic::apply_memory_decay(&mut mems, 0.2, 0.0);
        for (i, m) in mems.iter().enumerate() {
            assert!(
                m.importance_score <= before[i] + f32::EPSILON,
                "decay must be monotone non-increasing (n={n}, idx={i})"
            );
        }
    }
}

#[test]
fn decay_preserves_pinned_score_exactly() {
    let mut mems = batch(50);
    let pinned_before: Vec<(String, f32)> = mems
        .iter()
        .filter(|m| m.is_pinned)
        .map(|m| (m.id.clone(), m.importance_score))
        .collect();
    assert!(!pinned_before.is_empty());
    dynamic::apply_memory_decay(&mut mems, 0.9, 0.0);
    for (id, score) in &pinned_before {
        let m = mems.iter().find(|m| &m.id == id).expect("still present");
        assert_eq!(m.importance_score, *score, "pinned id={id} score changed");
        assert!(!m.is_cold, "pinned id={id} went cold");
    }
}

#[test]
fn decay_below_threshold_marks_cold() {
    let mut mems = batch(50);
    for m in mems.iter_mut() {
        m.is_pinned = false;
        m.importance_score = 0.1;
    }
    let (cold_count, ids) = dynamic::apply_memory_decay(&mut mems, 0.5, 0.5);
    assert_eq!(cold_count, ids.len(), "returned id list must match count");
    assert!(cold_count > 0, "below-threshold items should be demoted");
    for id in ids {
        let m = mems.iter().find(|m| m.id == id).expect("present");
        assert!(m.is_cold, "id={} marked in return but not on struct", m.id);
    }
}

#[test]
fn trim_never_evicts_pinned() {
    let mut mems = batch(50);
    let pinned_ids: std::collections::HashSet<String> = mems
        .iter()
        .filter(|m| m.is_pinned)
        .map(|m| m.id.clone())
        .collect();
    let evicted = dynamic::trim_memories_to_max(&mut mems, 10);
    for id in &evicted {
        assert!(
            !pinned_ids.contains(id),
            "pinned id={id} was evicted (violation)"
        );
    }
    let remaining_pinned = mems.iter().filter(|m| m.is_pinned).count();
    assert_eq!(remaining_pinned, pinned_ids.len(), "all pinned must remain");
}

#[test]
fn trim_to_max_caps_length_when_possible() {
    let mut mems = batch(30);
    // Unpin everything so trim can hit the target.
    for m in mems.iter_mut() {
        m.is_pinned = false;
    }
    dynamic::trim_memories_to_max(&mut mems, 10);
    assert!(
        mems.len() <= 10,
        "trim must respect max when no pins force overflow"
    );
}

#[test]
fn trim_under_max_is_noop() {
    let mut mems = batch(5);
    let snapshot: Vec<String> = mems.iter().map(|m| m.id.clone()).collect();
    let evicted = dynamic::trim_memories_to_max(&mut mems, 100);
    assert!(evicted.is_empty(), "no eviction when under capacity");
    let now: Vec<String> = mems.iter().map(|m| m.id.clone()).collect();
    assert_eq!(snapshot, now);
}

#[test]
fn select_respects_limit_and_min_similarity() {
    // 20 memories with random orthogonal-ish embeddings.
    let mut mems: Vec<MemoryEmbedding> = (0..20)
        .map(|i| {
            let mut e = make_mem(&format!("e{i:02}"), 0.5, false, 0);
            let mut emb = vec![0.0_f32; 20];
            emb[i] = 1.0;
            e.embedding = emb;
            e
        })
        .collect();
    // Add one duplicate-direction memory.
    let mut twin = mems[3].clone();
    twin.id = "twin".into();
    mems.push(twin);

    let mut query = vec![0.0_f32; 20];
    query[3] = 1.0;

    let picked = dynamic::select_relevant_memory_indices(&query, &mems, 5, 0.5);
    assert!(picked.len() <= 5, "must respect limit");
    for (_, sim) in &picked {
        assert!(*sim >= 0.5, "all returned must clear min similarity");
    }
}

#[test]
fn select_with_empty_pool_returns_empty() {
    let empty: Vec<MemoryEmbedding> = vec![];
    let picked = dynamic::select_relevant_memory_indices(&[1.0, 0.0], &empty, 10, 0.0);
    assert!(picked.is_empty());
}

#[test]
fn enforce_budget_never_demotes_pinned() {
    let mut mems = batch(40);
    for m in mems.iter_mut() {
        m.token_count = 200;
    }
    let pinned_ids: std::collections::HashSet<String> = mems
        .iter()
        .filter(|m| m.is_pinned)
        .map(|m| m.id.clone())
        .collect();
    let demoted = dynamic::enforce_hot_memory_budget(&mut mems, 500);
    for id in &demoted {
        assert!(
            !pinned_ids.contains(id),
            "budget enforcement demoted pinned id={id}"
        );
    }
}

#[test]
fn duplicate_detection_idempotent_against_self() {
    let mem = make_mem("x", 1.0, false, 0);
    let mems = vec![mem.clone()];
    let reason = dynamic::find_duplicate_memory_reason(&mem.text, Some(&mem.embedding), &mems);
    assert!(
        reason.is_some(),
        "a memory must always look like a duplicate of itself"
    );
}

#[test]
fn duplicate_detection_independent_of_pool_order() {
    let cand = make_mem("cand", 1.0, false, 0);
    let mut mems = vec![
        make_mem("a", 1.0, false, 0),
        make_mem("b", 1.0, false, 0),
        cand.clone(),
        make_mem("d", 1.0, false, 0),
    ];
    let r1 = dynamic::find_duplicate_memory_reason(&cand.text, Some(&cand.embedding), &mems);
    mems.reverse();
    let r2 = dynamic::find_duplicate_memory_reason(&cand.text, Some(&cand.embedding), &mems);
    assert_eq!(
        r1.is_some(),
        r2.is_some(),
        "duplicate detection should not depend on pool order"
    );
}

#[test]
fn cosine_symmetric() {
    for n in [1, 4, 16, 64] {
        let a: Vec<f32> = (0..n).map(|i| (i as f32) * 0.13).collect();
        let b: Vec<f32> = (0..n).map(|i| 1.0 - (i as f32) * 0.07).collect();
        let ab = dynamic::cosine_similarity(&a, &b);
        let ba = dynamic::cosine_similarity(&b, &a);
        assert!(
            (ab - ba).abs() < 1e-6,
            "cosine should be symmetric (n={n}, ab={ab}, ba={ba})"
        );
    }
}

#[test]
fn cosine_bounded_in_unit_interval() {
    for n in [2, 8, 32] {
        let a: Vec<f32> = (0..n).map(|i| (i as f32 + 1.0) * 0.1).collect();
        let b: Vec<f32> = (0..n).map(|i| (i as f32 * i as f32) * 0.05 + 0.1).collect();
        let s = dynamic::cosine_similarity(&a, &b);
        assert!(
            (-1.0..=1.0).contains(&s),
            "cosine must be in [-1, 1] (got {s})"
        );
    }
}
