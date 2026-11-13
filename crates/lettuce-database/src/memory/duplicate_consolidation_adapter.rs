use lettuce_memory::{ConsolidationCommit, ConsolidationGroup, ConsolidationPhase, ConsolidationStatus, DynamicMemoryAttemptStatus, DynamicMemoryRunRepositoryError, MemoryToolReducer, consolidation_texts, replace_duplicate_group};
use lettuce_types::{DynamicMemoryAttemptId, DynamicMemoryRunId, TimestampMillis};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::{Database, decode_versioned, encode_versioned};
use super::{dynamic_memory_run_adapter::{ensure_attempt_job_in, load_attempt_in, load_run_in, record_changed_items_in}, memory_adapter};

fn storage(_: impl std::fmt::Debug) -> DynamicMemoryRunRepositoryError {
    DynamicMemoryRunRepositoryError::Storage { reason: "duplicate consolidation storage failed" }
}

fn conflict() -> DynamicMemoryRunRepositoryError { DynamicMemoryRunRepositoryError::Conflict }

pub(crate) fn load(connection: &Connection, run_id: DynamicMemoryRunId) -> Result<Option<ConsolidationPhase>, DynamicMemoryRunRepositoryError> {
    let json: Option<String> = connection.query_row("SELECT phase_json FROM dynamic_memory_duplicate_consolidations WHERE run_id=?1", [run_id.to_string()], |row| row.get(0)).optional().map_err(storage)?;
    json.map(|json| decode_versioned(&json, 1).map_err(storage)).transpose()
}

fn owner(connection: &Connection, run_id: DynamicMemoryRunId, attempt_id: DynamicMemoryAttemptId, at: TimestampMillis) -> Result<lettuce_memory::DynamicMemoryRun, DynamicMemoryRunRepositoryError> {
    let run = load_run_in(connection, run_id)?;
    let attempt = load_attempt_in(connection, attempt_id)?;
    ensure_attempt_job_in(connection, &attempt, at)?;
    if attempt.run_id != run_id || attempt.status != DynamicMemoryAttemptStatus::Processing { return Err(conflict()); }
    Ok(run)
}

pub(crate) fn freeze(database: &Database, run_id: DynamicMemoryRunId, attempt_id: DynamicMemoryAttemptId, phase: ConsolidationPhase, at: TimestampMillis) -> Result<Option<ConsolidationPhase>, DynamicMemoryRunRepositoryError> {
    let mut connection = database.connection().map_err(storage)?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
    let run = owner(&tx, run_id, attempt_id, at)?;
    let unique_groups = phase.groups.iter().map(|group| group.base_id).collect::<std::collections::HashSet<_>>();
    if phase.groups.len() > lettuce_memory::CONSOLIDATION_MAX_GROUPS || unique_groups.len() != phase.groups.len()
        || phase.sources.iter().any(|source| !run.source_messages.iter().any(|frozen| frozen.message_id == source.message_id && frozen.role == source.role && frozen.effective_time == source.observed_at))
        || phase.groups.iter().any(|group| group.proposals.len() < 2 || group.applied.is_some() || !group.replacement_ids.is_empty() || group.base.as_ref().is_some_and(|base| base.id != group.base_id) || !matches!(group.status, ConsolidationStatus::Pending | ConsolidationStatus::Skipped))
    { return Err(conflict()); }
    tx.execute("INSERT OR IGNORE INTO dynamic_memory_duplicate_consolidations (run_id,phase_json) VALUES (?1,?2)", params![run_id.to_string(), encode_versioned(&phase, 1).map_err(storage)?]).map_err(storage)?;
    let stored = load(&tx, run_id)?;
    tx.commit().map_err(storage)?;
    Ok(stored)
}

pub(crate) fn commit(database: &Database, mut commit: ConsolidationCommit, at: TimestampMillis) -> Result<ConsolidationGroup, DynamicMemoryRunRepositoryError> {
    let mut connection = database.connection().map_err(storage)?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
    let run = owner(&tx, commit.run_id, commit.attempt_id, at)?;
    let mut phase = load(&tx, run.id)?.ok_or_else(conflict)?;
    let index = phase.groups.iter().position(|group| group.base_id == commit.expected.base_id).ok_or_else(conflict)?;
    // The database assigns the commit clock/revision. A replay may carry
    // either the pre-commit request or the returned durable receipt.
    let mut replay = commit.next.clone(); replay.applied = phase.groups[index].applied.clone();
    if phase.groups[index] == replay { return Ok(phase.groups[index].clone()); }
    if phase.groups[index] != commit.expected || !commit.expected.status.can_transition(commit.next.status)
        || commit.next.base_id != commit.expected.base_id || commit.next.base != commit.expected.base
        || commit.next.proposals != commit.expected.proposals
    { return Err(conflict()); }
    if commit.next.status == ConsolidationStatus::Applied {
        if commit.expected.response != commit.next.response || commit.embedding_revision.trim().is_empty()
            || !matches!(commit.embedding_dimensions, 64 | 128 | 256 | 512 | 768)
        { return Err(conflict()); }
        let texts = consolidation_texts(&commit.next, &phase.sources).map_err(|_| conflict())?;
        if texts.len() != commit.replacements.len()
            || texts.iter().zip(&commit.replacements).any(|(text, item)| item.text != text.text || item.category != Some(text.category) || (text.source_message_id.is_some() && item.source_message_id != text.source_message_id))
            || commit.next.replacement_ids != commit.replacements.iter().map(|item| item.id).collect::<Vec<_>>()
        { return Err(conflict()); }
        let before = memory_adapter::get_in(&tx, run.space_id).map_err(storage)?.ok_or_else(conflict)?;
        let change = replace_duplicate_group(&before, &commit.expected, commit.replacements, &commit.policy, at).map_err(|_| conflict())?;
        // Preflight the exact normal finishing policy: a group is optional,
        // so do not commit it if the next step would drop any of its entries.
        let projected = lettuce_memory::MemorySpaceSnapshot { id: change.space_id, revision: change.expected_revision, items: change.items.clone() };
        let finish = MemoryToolReducer.finish_cycle(&projected, &commit.policy).map_err(storage)?;
        if !finish.trimmed_ids.is_empty() || commit.next.replacement_ids.iter().any(|id| finish.demoted_ids.contains(id)) { return Err(conflict()); }
        let after = memory_adapter::compare_and_apply_in(&tx, &change).map_err(storage)?;
        commit.next.applied = Some(lettuce_memory::ConsolidationApplied { revision: after.revision, at });
        record_changed_items_in(&tx, run.id, &before, &after)?;
        for id in &commit.next.replacement_ids {
            let item = after.items.iter().find(|item| item.id == *id).ok_or_else(conflict)?;
            tx.execute("INSERT INTO memory_embedding_projections (space_id,memory_id,source_revision,dimensions,source_text,status,vector,updated_at) VALUES (?1,?2,?3,?4,?5,'repair_needed',NULL,?6)", params![run.space_id.to_string(), id.to_string(), commit.embedding_revision, i64::from(commit.embedding_dimensions), item.text, at.get()]).map_err(storage)?;
        }
    } else if !commit.replacements.is_empty() || !commit.next.replacement_ids.is_empty() || commit.next.applied.is_some() { return Err(conflict()); }
    phase.groups[index] = commit.next.clone();
    tx.execute("UPDATE dynamic_memory_duplicate_consolidations SET phase_json=?2 WHERE run_id=?1", params![run.id.to_string(), encode_versioned(&phase, 1).map_err(storage)?]).map_err(storage)?;
    tx.commit().map_err(storage)?;
    Ok(commit.next)
}
