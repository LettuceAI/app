use std::collections::{BTreeMap, BTreeSet};

use lettuce_memory::{
    DynamicMemoryAttempt, DynamicMemoryBackgroundRoundSettlement, DynamicMemoryInferenceRound,
    DynamicMemoryPendingApproval, DynamicMemoryPreparationPlan, DynamicMemoryRun,
    DynamicMemorySummaryCheckpoint,
};
use serde::{Deserialize, Serialize};

pub const DYNAMIC_MEMORY_BACKUP_VERSION: u32 = 1;
pub const MAX_BACKUP_DYNAMIC_MEMORY_RUNS: usize = 100_000;
pub const MAX_BACKUP_DYNAMIC_MEMORY_ATTEMPTS: usize = 1_000_000;
pub const MAX_BACKUP_DYNAMIC_MEMORY_PREPARATIONS: usize = 1_000_000;
pub const MAX_BACKUP_DYNAMIC_MEMORY_APPROVALS: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicMemoryBackup {
    pub version: u32,
    pub pending_approvals: Vec<DynamicMemoryPendingApproval>,
    pub preparation_plans: Vec<BackupDynamicMemoryPreparation>,
    pub runs: Vec<BackupDynamicMemoryRun>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupDynamicMemoryPreparation {
    pub plan: DynamicMemoryPreparationPlan,
    pub document: String,
    pub document_digest: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupDynamicMemoryRun {
    pub run: DynamicMemoryRun,
    pub attempts: Vec<BackupDynamicMemoryAttempt>,
    pub summary_checkpoint: Option<DynamicMemorySummaryCheckpoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupDynamicMemoryAttempt {
    pub attempt: DynamicMemoryAttempt,
    pub rounds: Vec<BackupDynamicMemoryRound>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupDynamicMemoryRound {
    pub round: DynamicMemoryInferenceRound,
    pub settlement: Option<DynamicMemoryBackgroundRoundSettlement>,
    pub settlement_change_digest: Option<String>,
}

impl DynamicMemoryBackup {
    pub fn canonicalize_and_validate(
        &mut self,
        history: &crate::ConversationHistoryBackup,
        runtime: &crate::ConversationRuntimeBackup,
        jobs: &crate::JobBackup,
        memory: &crate::MemoryBackup,
    ) -> Result<(), DynamicMemoryBackupError> {
        if self.version != DYNAMIC_MEMORY_BACKUP_VERSION
            || self.pending_approvals.len() > MAX_BACKUP_DYNAMIC_MEMORY_APPROVALS
            || self.preparation_plans.len() > MAX_BACKUP_DYNAMIC_MEMORY_PREPARATIONS
            || self.runs.len() > MAX_BACKUP_DYNAMIC_MEMORY_RUNS
        {
            return Err(DynamicMemoryBackupError::InvalidData);
        }
        self.pending_approvals
            .sort_by_key(|approval| approval.conversation_id);
        self.preparation_plans.sort_by_key(|entry| {
            (
                entry.plan.conversation_id,
                entry.plan.turn_id,
                entry.plan.attempt_id,
                entry.plan.first_execution_ordinal,
            )
        });
        self.runs.sort_by_key(|entry| entry.run.id);
        let conversations = history
            .conversations
            .iter()
            .map(|entry| entry.aggregate.conversation.id)
            .collect::<BTreeSet<_>>();
        let spaces = memory
            .spaces
            .iter()
            .map(|entry| (entry.conversation_id, &entry.snapshot))
            .collect::<BTreeMap<_, _>>();
        let messages = history
            .conversations
            .iter()
            .flat_map(|conversation| {
                conversation.messages.iter().map(|message| {
                    let sources = message
                        .revisions
                        .iter()
                        .map(|revision| {
                            lettuce_conversations::MessageRenderSource::Revision(revision.id)
                        })
                        .chain(message.candidates.iter().map(|candidate| {
                            lettuce_conversations::MessageRenderSource::Candidate(candidate.id)
                        }))
                        .collect::<Vec<_>>();
                    (
                        message.message.id,
                        (
                            conversation.aggregate.conversation.id,
                            message.message.role,
                            sources,
                        ),
                    )
                })
            })
            .collect::<BTreeMap<_, _>>();
        let job_ids = jobs
            .jobs
            .iter()
            .map(|entry| entry.snapshot.id)
            .collect::<BTreeSet<_>>();
        let generation_attempts = runtime
            .conversations
            .iter()
            .flat_map(|conversation| {
                conversation.turns.iter().flat_map(move |turn| {
                    turn.turn.attempts.iter().filter_map(move |attempt| {
                        let runtime = turn
                            .attempts
                            .iter()
                            .find(|runtime| runtime.attempt_id == attempt.id)?;
                        Some((
                            (conversation.conversation_id, turn.turn.id, attempt.id),
                            (attempt.job_id, &runtime.tools),
                        ))
                    })
                })
            })
            .collect::<BTreeMap<_, _>>();
        let mut approval_ids = BTreeSet::new();
        for approval in &self.pending_approvals {
            if !conversations.contains(&approval.conversation_id)
                || !approval_ids.insert(approval.conversation_id)
                || approval.prompted_message_count == 0
                || approval.pending == approval.skipped
            {
                return Err(DynamicMemoryBackupError::InvalidData);
            }
        }
        let mut preparation_ids = BTreeSet::new();
        for entry in &self.preparation_plans {
            let plan = &entry.plan;
            let identity = (
                plan.conversation_id,
                plan.turn_id,
                plan.attempt_id,
                plan.first_execution_ordinal,
            );
            let Some((job_id, executions)) =
                generation_attempts.get(&(plan.conversation_id, plan.turn_id, plan.attempt_id))
            else {
                return Err(DynamicMemoryBackupError::InvalidData);
            };
            if plan.validate().is_err()
                || !preparation_ids.insert(identity)
                || Some(plan.job_id) != *job_id
                || spaces
                    .get(&plan.conversation_id)
                    .is_none_or(|snapshot| snapshot.id != plan.space_id)
                || !valid_preparation_document(entry)
                || !valid_digest(&entry.document_digest)
                || plan
                    .execution_ids
                    .iter()
                    .any(|id| !executions.iter().any(|execution| execution.id == *id))
            {
                return Err(DynamicMemoryBackupError::InvalidData);
            }
        }
        let mut run_ids = BTreeSet::new();
        let mut attempt_count = 0usize;
        for entry in &mut self.runs {
            if entry.run.validate().is_err()
                || !run_ids.insert(entry.run.id)
                || !conversations.contains(&entry.run.conversation_id)
                || spaces
                    .get(&entry.run.conversation_id)
                    .is_none_or(|snapshot| {
                        snapshot.id != entry.run.space_id
                            || entry.run.starting_memory.revision > snapshot.revision
                    })
                || entry.run.source_messages.iter().any(|source| {
                    messages.get(&source.message_id).is_none_or(
                        |(conversation_id, role, sources)| {
                            *conversation_id != entry.run.conversation_id
                                || *role != source.role
                                || !sources.contains(&source.render_source)
                        },
                    )
                })
            {
                return Err(DynamicMemoryBackupError::InvalidData);
            }
            entry
                .attempts
                .sort_by_key(|attempt| attempt.attempt.ordinal);
            attempt_count = attempt_count
                .checked_add(entry.attempts.len())
                .ok_or(DynamicMemoryBackupError::InvalidData)?;
            validate_run(entry, &job_ids)?;
        }
        if attempt_count > MAX_BACKUP_DYNAMIC_MEMORY_ATTEMPTS {
            return Err(DynamicMemoryBackupError::InvalidData);
        }
        Ok(())
    }
}

fn validate_run(
    entry: &BackupDynamicMemoryRun,
    job_ids: &BTreeSet<lettuce_types::JobId>,
) -> Result<(), DynamicMemoryBackupError> {
    if entry.attempts.is_empty() {
        return Err(DynamicMemoryBackupError::InvalidData);
    }
    let mut attempts = BTreeMap::new();
    for (ordinal, entry_attempt) in entry.attempts.iter().enumerate() {
        let attempt = &entry_attempt.attempt;
        if attempt.validate().is_err()
            || attempt.run_id != entry.run.id
            || usize::from(attempt.ordinal) != ordinal
            || !job_ids.contains(&attempt.job_id)
            || attempts
                .insert(attempt.id, attempt.retry_parent_id)
                .is_some()
        {
            return Err(DynamicMemoryBackupError::InvalidData);
        }
        if ordinal > 0 && attempt.retry_parent_id != Some(entry.attempts[ordinal - 1].attempt.id) {
            return Err(DynamicMemoryBackupError::InvalidData);
        }
        for (round_ordinal, backup_round) in entry_attempt.rounds.iter().enumerate() {
            let round = &backup_round.round;
            if round.validate().is_err()
                || round.run_id != entry.run.id
                || round.attempt_id != attempt.id
                || usize::from(round.ordinal) != round_ordinal
                || backup_round.settlement.is_some()
                    != backup_round.settlement_change_digest.is_some()
            {
                return Err(DynamicMemoryBackupError::InvalidData);
            }
            if let Some(settlement) = &backup_round.settlement
                && (settlement.run_id != entry.run.id
                    || settlement.attempt_id != attempt.id
                    || settlement.round_ordinal != round.ordinal
                    || settlement.space_id != entry.run.space_id
                    || settlement.results.len() != round.calls.len()
                    || settlement
                        .results
                        .iter()
                        .zip(&round.calls)
                        .any(|(result, call)| result.execution_id != call.id)
                    || !valid_digest(
                        backup_round
                            .settlement_change_digest
                            .as_deref()
                            .ok_or(DynamicMemoryBackupError::InvalidData)?,
                    ))
            {
                return Err(DynamicMemoryBackupError::InvalidData);
            }
        }
    }
    if let Some(checkpoint) = &entry.summary_checkpoint
        && (checkpoint.run_id != entry.run.id
            || checkpoint.summary.space_id != entry.run.space_id
            || checkpoint.summary.validate().is_err()
            || !attempts.contains_key(&checkpoint.attempt_id)
            || checkpoint.resulting_memory_revision.get()
                != checkpoint.expected_memory_revision.get().saturating_add(1))
    {
        return Err(DynamicMemoryBackupError::InvalidData);
    }
    Ok(())
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_preparation_document(entry: &BackupDynamicMemoryPreparation) -> bool {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct VersionedPlan {
        format_version: u32,
        value: DynamicMemoryPreparationPlan,
    }

    blake3::hash(entry.document.as_bytes()).to_hex().as_str() == entry.document_digest
        && serde_json::from_str::<VersionedPlan>(&entry.document)
            .is_ok_and(|document| document.format_version == 1 && document.value == entry.plan)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DynamicMemoryBackupError {
    #[error("dynamic-memory backup contains invalid data")]
    InvalidData,
}
