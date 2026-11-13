mod duplicate_consolidation_tests {
    use super::*;
    use lettuce_memory::*;
    use crate::memory::{memory_adapter, dynamic_memory_run_adapter::load_run_in};

    fn policy() -> MemoryPolicy {
        MemoryPolicy { max_entries: 20, hot_token_budget: 1000, cold_threshold: Score::ZERO, delete_confidence_default: Score::FULL, max_hard_delete_ratio_per_cycle: Score::ZERO, decay_rate: Score::ZERO }
    }

    fn setup(database: &Database) -> (DynamicMemoryAttempt, ConsolidationGroup, MemorySpaceId) {
        let (conversation, space, messages) = conversation_fixture(database);
        let snapshot = MemoryRepository::get(database, space).expect("get").expect("space");
        let mut base = memory_item(MemoryId::new(), "Mara works at the hospital.", 3); base.token_count = Some(5); base.is_pinned = true;
        database.compare_and_apply(MemoryChangeSet { space_id: space, expected_revision: snapshot.revision, items: vec![base.clone()] }).expect("base");
        let attempt = checkpointed_run(database, conversation, space, &messages, "Summary", 10);
        let source = ConsolidationSource { message_id: messages[0].message_id, role: messages[0].role, observed_at: messages[0].effective_time, text: "Mara works nights.".into() };
        let group = ConsolidationGroup { base_id: base.id, base: Some(base), proposals: vec!["Mara works nights.".into(), "Mara works nights.".into()], status: ConsolidationStatus::Pending, response: None, replacement_ids: Vec::new(), applied: None, reason: None };
        database.freeze_duplicate_consolidation(attempt.run_id, attempt.id, ConsolidationPhase { groups: vec![group.clone()], sources: vec![source], omitted_groups: 0 }, TimestampMillis::new(11)).expect("freeze").expect("phase");
        (attempt, group, space)
    }

    fn save(database: &Database, attempt: &DynamicMemoryAttempt, expected: ConsolidationGroup, next: ConsolidationGroup, replacements: Vec<MemoryItem>) -> Result<ConsolidationGroup, DynamicMemoryRunRepositoryError> {
        database.commit_duplicate_consolidation(ConsolidationCommit { run_id: attempt.run_id, attempt_id: attempt.id, expected, next, replacements, policy: policy(), embedding_revision: "test-model".into(), embedding_dimensions: 64 }, TimestampMillis::new(12))
    }

    fn ready(database: &Database, attempt: &DynamicMemoryAttempt, group: ConsolidationGroup) -> ConsolidationGroup {
        let mut dispatched = group.clone(); dispatched.status = ConsolidationStatus::Dispatched;
        let group = save(database, attempt, group, dispatched, vec![]).expect("dispatch");
        let phase = database.load_duplicate_consolidation(attempt.run_id).expect("phase").expect("stored");
        let mut next = group.clone(); next.status = ConsolidationStatus::Ready;
        next.response = Some(ConsolidationResponse::Replace { base_preserved: true, no_conflicts: true, new_information: true, replacements: vec![
            ConsolidationReplacement { category: MemoryCategory::Profile, details: vec![ConsolidationDetail { text: "Mara works at the hospital.".into(), evidence: ConsolidationEvidence::Base { quote: "Mara works at the hospital.".into() } }] },
            ConsolidationReplacement { category: MemoryCategory::Routine, details: vec![ConsolidationDetail { text: "Mara works nights.".into(), evidence: ConsolidationEvidence::Message { message_id: phase.sources[0].message_id, quote: "Mara works nights.".into() } }] },
        ] });
        save(database, attempt, group, next, vec![]).expect("durable response")
    }

    fn replacement_items(group: &ConsolidationGroup, phase: &ConsolidationPhase) -> Vec<MemoryItem> {
        consolidation_texts(group, &phase.sources).expect("valid response").into_iter().map(|text| {
            let mut item = memory_item(MemoryId::new(), &text.text, 12); item.category = Some(text.category); item.source_message_id = text.source_message_id; item.token_count = Some(5); item
        }).collect()
    }

    #[test]
    fn consolidation_receipt_and_all_replacements_commit_together_and_replay_once() {
        let database = Database::open_in_memory().expect("database");
        let (attempt, group, space) = setup(&database);
        let group = ready(&database, &attempt, group);
        let phase = database.load_duplicate_consolidation(attempt.run_id).expect("phase").expect("stored");
        let items = replacement_items(&group, &phase);
        let mut applied = group.clone(); applied.status = ConsolidationStatus::Applied; applied.replacement_ids = items.iter().map(|item| item.id).collect();
        let stored = save(&database, &attempt, group.clone(), applied.clone(), items.clone()).expect("apply");
        assert_eq!(save(&database, &attempt, group, applied, items).expect("replay"), stored);
        let snapshot = MemoryRepository::get(&database, space).expect("get").expect("space");
        assert_eq!(snapshot.items.len(), 3);
        assert!(snapshot.items[0].is_cold && !snapshot.items[0].is_pinned);
        assert!(snapshot.items[1..].iter().all(|item| item.is_pinned && item.supersedes == vec![stored.base_id]));
        assert_eq!(snapshot.successors(stored.base_id).len(), 2);
        let mut trimmed = snapshot.items.clone(); trimmed.pop();
        assert_eq!(database.apply_dynamic_memory_cycle_finish(attempt.id, MemoryChangeSet {
            space_id: space, expected_revision: snapshot.revision, items: trimmed,
        }, TimestampMillis::new(13)), Err(MemoryRepositoryError::Conflict));
        assert_eq!(MemoryRepository::get(&database, space).expect("after rejected trim"), Some(snapshot));
        let connection = database.connection().expect("connection");
        let repairs: i64 = connection.query_row("SELECT count(*) FROM memory_embedding_projections WHERE space_id=?1 AND status='repair_needed'", [space.to_string()], |row| row.get(0)).expect("repairs");
        assert_eq!(repairs, 2);
        let changed: i64 = connection.query_row("SELECT count(*) FROM dynamic_memory_changed_items WHERE run_id=?1", [attempt.run_id.to_string()], |row| row.get(0)).expect("journal");
        assert_eq!(changed, 3);
    }

    #[test]
    fn consolidation_backup_and_source_rewind_preserve_the_operation_boundary() {
        let database = Database::open_in_memory().expect("database");
        let (attempt, group, space) = setup(&database);
        let base = group.base.clone().expect("base");
        let group = ready(&database, &attempt, group);
        let phase = database.load_duplicate_consolidation(attempt.run_id).expect("phase").expect("stored");
        let items = replacement_items(&group, &phase);
        let mut applied = group.clone(); applied.status = ConsolidationStatus::Applied;
        applied.replacement_ids = items.iter().map(|item| item.id).collect();
        let stored = save(&database, &attempt, group, applied, items).expect("apply");
        let conversation_id = database.load_dynamic_memory_run(attempt.run_id).expect("run").conversation_id;
        let restored_job_id = memory_job(&database, conversation_id);
        // This fixture has minimal legacy conversation rows. Exercise the
        // same run exporter/restorer used by full backups without exporting
        // its intentionally incomplete conversation-history graph.
        let mut connection = database.connection().expect("connection");
        let tx = connection.transaction().expect("transaction");
        let backup = crate::backup::backup_adapter::read_dynamic_memory(&tx).expect("export runs");
        let encoded = serde_json::to_vec(&backup).expect("encode backup");
        let backup: lettuce_transfer::DynamicMemoryBackup = serde_json::from_slice(&encoded).expect("decode backup");
        assert_eq!(backup.runs[0].duplicate_consolidation.as_ref().expect("backup receipt").groups, vec![stored.clone()]);
        // Rebind only the test run namespace; immutable original runs may
        // not be deleted. The restored receipt and memory IDs stay exact.
        let mut exported = backup.runs[0].clone();
        let restored_run_id = DynamicMemoryRunId::new();
        let restored_attempt_id = DynamicMemoryAttemptId::new();
        exported.run.id = restored_run_id;
        exported.attempts[0].attempt.run_id = restored_run_id;
        exported.attempts[0].attempt.id = restored_attempt_id;
        exported.attempts[0].attempt.job_id = restored_job_id;
        let checkpoint = exported.summary_checkpoint.as_mut().expect("checkpoint");
        checkpoint.run_id = restored_run_id; checkpoint.attempt_id = restored_attempt_id;
        crate::memory::dynamic_memory_run_adapter::insert_restored_run_in(&tx, &exported).expect("restore run");
        assert_eq!(crate::memory::duplicate_consolidation_adapter::load(&tx, restored_run_id).expect("restore receipt").expect("phase").groups, vec![stored]);
        let current = memory_adapter::get_in(&tx, space).expect("get").expect("space");
        let run = load_run_in(&tx, restored_run_id).expect("run");
        let result = crate::memory::dynamic_memory_rewind_adapter::undo_events(&tx, &current, run.conversation_id, vec![run.id], &[], &mut None, true).expect("undo source effects");
        assert_eq!(result.items, vec![base]);
        tx.commit().expect("commit rewind");
    }

    #[test]
    fn consolidation_sql_failure_rolls_back_all_outputs_base_and_receipt() {
        let database = Database::open_in_memory().expect("database");
        let (attempt, group, space) = setup(&database);
        let group = ready(&database, &attempt, group);
        let phase = database.load_duplicate_consolidation(attempt.run_id).expect("phase").expect("stored");
        let before = MemoryRepository::get(&database, space).expect("get").expect("space");
        let items = replacement_items(&group, &phase);
        let mut applied = group.clone(); applied.status = ConsolidationStatus::Applied; applied.replacement_ids = items.iter().map(|item| item.id).collect();
        database.connection().expect("connection").execute_batch("CREATE TRIGGER fail_second_replacement BEFORE INSERT ON memory_items WHEN NEW.text='Mara works nights.' BEGIN SELECT RAISE(ABORT,'injected failure'); END;").expect("fault");
        assert!(save(&database, &attempt, group.clone(), applied, items).is_err());
        assert_eq!(MemoryRepository::get(&database, space).expect("get"), Some(before));
        assert_eq!(database.load_duplicate_consolidation(attempt.run_id).expect("phase").expect("stored").groups, vec![group]);
    }

    #[test]
    fn consolidation_does_not_replace_a_base_edited_after_the_request() {
        let database = Database::open_in_memory().expect("database");
        let (attempt, group, space) = setup(&database);
        let group = ready(&database, &attempt, group);
        let phase = database.load_duplicate_consolidation(attempt.run_id).expect("phase").expect("stored");
        let items = replacement_items(&group, &phase);
        let mut current = MemoryRepository::get(&database, space).expect("get").expect("space"); current.items[0].text = "A newer user edit".into();
        let current = database.compare_and_apply(MemoryChangeSet { space_id: space, expected_revision: current.revision, items: current.items }).expect("concurrent edit");
        let mut applied = group.clone(); applied.status = ConsolidationStatus::Applied; applied.replacement_ids = items.iter().map(|item| item.id).collect();
        assert_eq!(save(&database, &attempt, group, applied, items), Err(DynamicMemoryRunRepositoryError::Conflict));
        assert_eq!(MemoryRepository::get(&database, space).expect("get"), Some(current));
    }
}
