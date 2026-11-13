mod duplicate_consolidation_flow_tests {
    use super::*;
    use std::sync::{Mutex, atomic::{AtomicUsize, Ordering}};
    use lettuce_conversations::{FinishReason, InferenceCandidate, InferenceOutcome, InferencePort, InferenceRequest, MessagePart, PortError};
    use lettuce_embeddings::{EmbeddingDimensions, EmbeddingRequest, EmbeddingVector};
    use lettuce_jobs::handle::CancellationToken;
    use lettuce_memory::*;
    use lettuce_types::{MemoryId, MessageId};
    use crate::companion::companion_memory_duplicates::{consolidate_duplicates, cycle_duplicate_candidates};

    struct Engine;
    impl crate::MemoryEmbeddingEngine for Engine {
        fn source_revision(&self) -> &str { "consolidation-test" }
        fn dimensions(&self) -> EmbeddingDimensions { EmbeddingDimensions::D64 }
        fn count_tokens(&self, text: &str) -> Result<u32, crate::EmbeddingGenerationError> { Ok(text.split_whitespace().count() as u32) }
        fn embed_memory(&self, request: &EmbeddingRequest, _: &CancellationToken) -> Result<EmbeddingVector, crate::EmbeddingGenerationError> {
            let mut values = vec![0.0; request.dimensions.get()]; values[0] = 1.0;
            Ok(EmbeddingVector { source_revision: self.source_revision().into(), values })
        }
    }

    struct Provider { text: String, calls: AtomicUsize, requests: Mutex<Vec<InferenceRequest>> }
    #[async_trait::async_trait]
    impl InferencePort for Provider {
        async fn run(&self, request: InferenceRequest) -> Result<InferenceOutcome, PortError> {
            self.calls.fetch_add(1, Ordering::SeqCst); self.requests.lock().expect("requests").push(request);
            Ok(InferenceOutcome { provider_response_id: None, candidates: vec![InferenceCandidate { ordinal: 0, parts: vec![MessagePart::Text { text: self.text.clone() }], tool_calls: vec![], provider_replay: None, media: vec![] }], usage: None, finish_reason: FinishReason::Stop, provider_finish_reason: None, provider_request_id: None, warning_codes: vec![] })
        }
    }
    fn provider(text: &str) -> Provider { Provider { text: text.into(), calls: AtomicUsize::new(0), requests: Mutex::new(Vec::new()) } }
    fn policy() -> MemoryPolicy { MemoryPolicy { max_entries: 30, hot_token_budget: 1000, cold_threshold: Score::ZERO, delete_confidence_default: Score::FULL, max_hard_delete_ratio_per_cycle: Score::ZERO, decay_rate: Score::ZERO } }

    async fn fixture() -> (crate::api::tests::Harness, OpenRound, MemoryItem, MemoryDuplicateCandidates) {
        let h = harness(Reply::Text("Mara works night shifts at the hospital."));
        let (chat, _) = replied_chat(&h, "consolidate").await;
        let database = h.context.backend().database();
        let aggregate = lettuce_conversations::ConversationReader::get(database, chat.parse().expect("chat")).expect("conversation");
        let snapshot = MemoryRepository::get_for_branch(database, aggregate.conversation.id, aggregate.conversation.active_branch_id).expect("memory").expect("space");
        let id = MemoryId::new();
        let mut base = MemoryItem::written(id, MemoryShortId::derived(id), "Mara works at the hospital.".into(), h.context.now());
        base.token_count = Some(6); base.is_pinned = true;
        database.compare_and_apply(MemoryChangeSet { space_id: snapshot.id, expected_revision: snapshot.revision, items: vec![base.clone()] }).expect("base");
        let round = open_round(&h, &chat, vec![]).await;
        let candidates = [(id, MemoryDuplicateCandidate { existing_text: Some(base.text.clone()), proposed_texts: vec!["Mara works nights.".into(), "Mara works night shifts.".into()] })].into_iter().collect();
        (h, round, base, candidates)
    }

    fn replacement(base: &MemoryItem, source: MessageId) -> String {
        serde_json::to_string(&ConsolidationResponse::Replace { base_preserved: true, no_conflicts: true, new_information: true, replacements: vec![
            ConsolidationReplacement { category: MemoryCategory::Profile, details: vec![ConsolidationDetail { text: base.text.clone(), evidence: ConsolidationEvidence::Base { quote: base.text.clone() } }] },
            ConsolidationReplacement { category: MemoryCategory::Routine, details: vec![ConsolidationDetail { text: "Mara works night shifts.".into(), evidence: ConsolidationEvidence::Message { message_id: source, quote: "night shifts".into() } }] },
        ] }).expect("response")
    }

    #[tokio::test]
    async fn consolidation_uses_the_memory_profile_and_replays_a_complete_replacement_once() {
        let (h, round, base, candidates) = fixture().await;
        let database = h.context.backend().database();
        let run = database.load_dynamic_memory_run(round.run_id).expect("run");
        let source = run.source_messages.iter().find(|source| source.role == lettuce_conversations::MessageRole::Assistant).expect("assistant").message_id;
        let llm = provider(&replacement(&base, source));
        let groups = consolidate_duplicates(&Engine, database, database, &llm, &run, round.attempt_id, &candidates, &policy(), &round.handle, h.context.now()).await.expect("consolidation");
        assert_eq!(groups[0].status, ConsolidationStatus::Applied);
        assert_eq!(groups[0].replacement_ids.len(), 2);
        let memory = MemoryRepository::get(database, run.space_id).expect("memory").expect("space");
        assert_eq!(memory.items.len(), 3);
        assert!(memory.items[0].is_cold && !memory.items[0].is_pinned);
        assert!(memory.items[1..].iter().all(|item| item.is_pinned && !item.is_cold));
        let requests = llm.requests.lock().expect("requests");
        assert_eq!(requests[0].profile.chat_profile.model_profile_id, run.profile.chat_profile.model_profile_id);
        assert!(requests[0].tools.is_none());
        assert_eq!(requests[0].profile.tool_policy, lettuce_conversations::ToolPolicy::Disabled);
        assert_eq!(requests[0].profile.chat_profile.parameters.visible_max_output_tokens, Some(CONSOLIDATION_MAX_OUTPUT_TOKENS));
        drop(requests);
        let replay = consolidate_duplicates(&Engine, database, database, &llm, &run, round.attempt_id, &candidates, &policy(), &round.handle, h.context.now()).await.expect("replay");
        assert_eq!(replay, groups); assert_eq!(llm.calls.load(Ordering::SeqCst), 1);
        assert!(cycle_duplicate_candidates(database, run.id, round.attempt_id).expect("map").is_empty(), "replacements never enter the ordinary candidate map");
    }

    #[tokio::test]
    async fn consolidation_one_proposal_never_dispatches_but_repeated_texts_are_eligible() {
        let (h, round, _, mut candidates) = fixture().await;
        let database = h.context.backend().database(); let run = database.load_dynamic_memory_run(round.run_id).expect("run");
        let llm = provider(r#"{"decision":"no_change"}"#);
        candidates.values_mut().next().expect("candidate").proposed_texts.truncate(1);
        assert!(consolidate_duplicates(&Engine, database, database, &llm, &run, round.attempt_id, &candidates, &policy(), &round.handle, h.context.now()).await.expect("one").is_empty());
        assert_eq!(llm.calls.load(Ordering::SeqCst), 0);
        let proposal = candidates.values().next().expect("candidate").proposed_texts[0].clone();
        candidates.values_mut().next().expect("candidate").proposed_texts.push(proposal);
        let groups = consolidate_duplicates(&Engine, database, database, &llm, &run, round.attempt_id, &candidates, &policy(), &round.handle, h.context.now()).await.expect("two");
        assert_eq!(llm.calls.load(Ordering::SeqCst), 1); assert_eq!(groups[0].status, ConsolidationStatus::NoChange);
        assert_eq!(MemoryRepository::get(database, run.space_id).expect("memory").expect("space").items.len(), 1);
    }

    #[tokio::test]
    async fn consolidation_recovery_never_redispatches_an_uncertain_request() {
        let (h, round, base, candidates) = fixture().await;
        let database = h.context.backend().database(); let run = database.load_dynamic_memory_run(round.run_id).expect("run");
        let group = ConsolidationGroup { base_id: base.id, base: Some(base), proposals: vec!["one".into(), "two".into()], status: ConsolidationStatus::Pending, response: None, replacement_ids: vec![], applied: None, reason: None };
        database.freeze_duplicate_consolidation(run.id, round.attempt_id, ConsolidationPhase { groups: vec![group.clone()], sources: vec![], omitted_groups: 0 }, h.context.now()).expect("freeze");
        let mut dispatched = group.clone(); dispatched.status = ConsolidationStatus::Dispatched;
        database.commit_duplicate_consolidation(ConsolidationCommit { run_id: run.id, attempt_id: round.attempt_id, expected: group, next: dispatched, replacements: vec![], policy: policy(), embedding_revision: "consolidation-test".into(), embedding_dimensions: 64 }, h.context.now()).expect("dispatch intent");
        let llm = provider("must never be requested");
        let groups = consolidate_duplicates(&Engine, database, database, &llm, &run, round.attempt_id, &candidates, &policy(), &round.handle, h.context.now()).await.expect("recover");
        assert_eq!(llm.calls.load(Ordering::SeqCst), 0); assert_eq!(groups[0].status, ConsolidationStatus::Skipped);
    }

    #[tokio::test]
    async fn consolidation_oversized_or_unsupported_output_keeps_the_base_and_pin() {
        for output in ["not valid JSON".to_owned(), "x".repeat(CONSOLIDATION_MAX_OUTPUT_BYTES + 1)] {
            let (h, round, base, candidates) = fixture().await;
            let database = h.context.backend().database(); let run = database.load_dynamic_memory_run(round.run_id).expect("run");
            let llm = provider(&output);
            let groups = consolidate_duplicates(&Engine, database, database, &llm, &run, round.attempt_id, &candidates, &policy(), &round.handle, h.context.now()).await.expect("rejected");
            assert_eq!(groups[0].status, ConsolidationStatus::Rejected);
            assert_eq!(MemoryRepository::get(database, run.space_id).expect("get").expect("space").items, vec![base]);
            assert_eq!(llm.calls.load(Ordering::SeqCst), 1);
        }
    }

    #[tokio::test]
    async fn consolidation_recovers_a_recorded_response_without_another_inference() {
        let (h, round, base, candidates) = fixture().await;
        let database = h.context.backend().database();
        let run = database.load_dynamic_memory_run(round.run_id).expect("run");
        let sources = crate::companion::companion_memory_inference::materialize_sources(database, &run).expect("frozen sources")
            .into_iter().map(|source| ConsolidationSource { message_id: source.message_id, role: source.role, observed_at: source.effective_time, text: source.text }).collect::<Vec<_>>();
        let source = sources.iter().find(|source| source.role == lettuce_conversations::MessageRole::Assistant).expect("assistant").message_id;
        let pending = ConsolidationGroup { base_id: base.id, base: Some(base.clone()), proposals: candidates[&base.id].proposed_texts.clone(), status: ConsolidationStatus::Pending, response: None, replacement_ids: vec![], applied: None, reason: None };
        database.freeze_duplicate_consolidation(run.id, round.attempt_id, ConsolidationPhase { groups: vec![pending.clone()], sources, omitted_groups: 0 }, h.context.now()).expect("freeze");
        let mut dispatched = pending.clone(); dispatched.status = ConsolidationStatus::Dispatched;
        let commit = |expected, next| ConsolidationCommit { run_id: run.id, attempt_id: round.attempt_id, expected, next, replacements: vec![], policy: policy(), embedding_revision: "consolidation-test".into(), embedding_dimensions: 64 };
        database.commit_duplicate_consolidation(commit(pending, dispatched.clone()), h.context.now()).expect("dispatch intent");
        let mut ready = dispatched.clone(); ready.status = ConsolidationStatus::Ready;
        ready.response = Some(serde_json::from_str(&replacement(&base, source)).expect("response"));
        database.commit_duplicate_consolidation(commit(dispatched, ready), h.context.now()).expect("persist response before crash");
        let llm = provider("must never be requested");
        let groups = consolidate_duplicates(&Engine, database, database, &llm, &run, round.attempt_id, &candidates, &policy(), &round.handle, h.context.now()).await.expect("recovery");
        assert_eq!(llm.calls.load(Ordering::SeqCst), 0);
        assert_eq!(groups[0].status, ConsolidationStatus::Applied);
        assert_eq!(MemoryRepository::get(database, run.space_id).expect("get").expect("space").items.len(), 3);
    }

    #[tokio::test]
    async fn consolidation_bounds_and_foreign_space_ids_skip_without_dispatch() {
        for case in ["proposal_limit", "input_limit", "stale_base"] {
            let (h, round, base, mut candidates) = fixture().await;
            let database = h.context.backend().database(); let run = database.load_dynamic_memory_run(round.run_id).expect("run");
            match case {
                "proposal_limit" => candidates.get_mut(&base.id).expect("candidate").proposed_texts = vec!["same".into(); CONSOLIDATION_MAX_PROPOSALS + 1],
                "input_limit" => candidates.get_mut(&base.id).expect("candidate").proposed_texts = vec!["x".repeat(CONSOLIDATION_MAX_INPUT_BYTES); 2],
                _ => { let candidate = candidates.remove(&base.id).expect("candidate"); candidates.insert(MemoryId::new(), candidate); }
            }
            let llm = provider("must never be requested");
            let groups = consolidate_duplicates(&Engine, database, database, &llm, &run, round.attempt_id, &candidates, &policy(), &round.handle, h.context.now()).await.expect("bounded skip");
            assert_eq!(llm.calls.load(Ordering::SeqCst), 0);
            assert_eq!(groups[0].status, ConsolidationStatus::Skipped);
            assert_eq!(groups[0].reason.as_deref(), Some(case));
            assert_eq!(MemoryRepository::get(database, run.space_id).expect("get").expect("space").items, vec![base]);
        }
    }

    #[tokio::test]
    async fn cancelled_consolidation_preserves_the_base_without_an_inference() {
        let (h, round, base, candidates) = fixture().await;
        let database = h.context.backend().database(); let run = database.load_dynamic_memory_run(round.run_id).expect("run");
        round.handle.request_cancel();
        let llm = provider("must never be requested");
        assert!(matches!(consolidate_duplicates(&Engine, database, database, &llm, &run, round.attempt_id, &candidates, &policy(), &round.handle, h.context.now()).await, Err(crate::CompanionMemoryInferenceError::Cancelled)));
        assert_eq!(llm.calls.load(Ordering::SeqCst), 0);
        assert_eq!(MemoryRepository::get(database, run.space_id).expect("get").expect("space").items, vec![base]);
    }

    #[test]
    #[ignore = "requires an installed Eidos manifest and ONNX Runtime, supplied explicitly by environment"]
    fn consolidation_eidos_retrieval_regression() {
        use crate::MemoryEmbeddingEngine;
        let manifest_path = std::env::var_os("LETTUCE_CONSOLIDATION_EIDOS_MANIFEST").expect("manifest path");
        let runtime = std::env::var_os("LETTUCE_CONSOLIDATION_ORT_LIBRARY").expect("runtime library path");
        let manifest: lettuce_model_hub::InstalledEmbeddingManifest = serde_json::from_slice(&std::fs::read(manifest_path).expect("read manifest")).expect("manifest");
        let engine = crate::EmbeddingService::load(&manifest, &lettuce_embeddings::OnnxRuntimeLink::Dynamic(runtime.into()), EmbeddingDimensions::D768, None).expect("verified Eidos runtime");
        let before = [
            "Mara works at the hospital.", "Mara works night shifts.", "Mara is a surgeon.", "Mara started working at the hospital in 2020.", "Mara supervises two junior doctors.",
            "Jules is a botanist.", "Jules works in a greenhouse.", "Jules started working at the greenhouse in 2018.",
            "Inez is a violin teacher.", "Inez teaches at the conservatory.", "Inez gives lessons on Saturdays.",
            "Mara volunteers at an animal shelter on Sundays.", "Jules plays chess every Friday.", "Inez lives near the railway station.",
        ];
        let after = [
            "Mara is a surgeon at the hospital. She works night shifts.", "Mara started at the hospital in 2020. She supervises two junior doctors.",
            "Jules is a botanist at the greenhouse. Jules has worked there since 2018.",
            "Inez teaches violin at the conservatory. Inez gives lessons on Saturdays.",
            "Mara volunteers at an animal shelter on Sundays.", "Jules plays chess every Friday.", "Inez lives near the railway station.",
        ];
        let queries = [
            ("Where does Mara work?", 0, 0), ("What shifts does Mara work?", 1, 0), ("What is Mara's profession?", 2, 0),
            ("When did Mara start at the hospital?", 3, 1), ("How many junior doctors does Mara supervise?", 4, 1),
            ("What is Jules's profession?", 5, 2), ("Where does Jules work?", 6, 2), ("When did Jules start at the greenhouse?", 7, 2),
            ("What instrument does Inez teach?", 8, 3), ("Where does Inez teach?", 9, 3), ("On which day does Inez give lessons?", 10, 3),
            ("Where does Mara volunteer?", 11, 4), ("When does Jules play chess?", 12, 5), ("Where does Inez live?", 13, 6),
        ];
        let cancellation = CancellationToken::new();
        let embed = |text: &str| engine.embed_memory(&EmbeddingRequest { text: text.into(), dimensions: EmbeddingDimensions::D768 }, &cancellation).expect("embedding");
        let before_vectors = before.iter().map(|text| embed(text)).collect::<Vec<_>>();
        let after_vectors = after.iter().map(|text| embed(text)).collect::<Vec<_>>();
        let mut scores = [[0usize; 2]; 2];
        for (query, expected_before, expected_after) in queries {
            let query = embed(query);
            for (side, (vectors, expected)) in [(&before_vectors, expected_before), (&after_vectors, expected_after)].into_iter().enumerate() {
                let mut ranking = vectors.iter().enumerate().map(|(index, vector)| (index, query.cosine_similarity(vector).expect("cosine"))).collect::<Vec<_>>();
                ranking.sort_by(|left, right| right.1.total_cmp(&left.1));
                scores[side][0] += usize::from(ranking[0].0 == expected);
                scores[side][1] += usize::from(ranking.iter().take(5).any(|(index, _)| *index == expected));
            }
        }
        println!("Eidos consolidation fixture Recall@1: before={}/14 after={}/14; Recall@5: before={}/14 after={}/14", scores[0][0], scores[1][0], scores[0][1], scores[1][1]);
        assert!(scores[1][0] >= scores[0][0], "compact replacement retrieval regressed at rank 1");
        assert!(scores[1][1] >= scores[0][1], "compact replacement retrieval regressed at rank 5");
    }
}
