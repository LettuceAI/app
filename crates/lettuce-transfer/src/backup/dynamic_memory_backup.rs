use std::collections::{BTreeMap, BTreeSet};

use lettuce_memory::{
    DynamicMemoryAttempt, DynamicMemoryBackgroundRoundSettlement, DynamicMemoryInferenceRound,
    DynamicMemoryPendingApproval, DynamicMemoryRun, DynamicMemorySummaryCheckpoint,
    MemoryCycleRevertRecord, PendingSuffixRewind,
};
use serde::{Deserialize, Serialize};

pub const DYNAMIC_MEMORY_BACKUP_VERSION: u32 = 6;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicMemoryBackup {
    pub version: u32,
    pub pending_approvals: Vec<DynamicMemoryPendingApproval>,
    pub pending_suffix_rewinds: Vec<BackupPendingSuffixRewind>,
    pub runs: Vec<BackupDynamicMemoryRun>,
    /// Cycles the user reverted; the runs keep their recorded outcomes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cycle_reverts: Vec<MemoryCycleRevertRecord>,
}

/// A delete-after whose memory rewind is still owed, with the time it was
/// recorded; owed rewinds finish oldest first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupPendingSuffixRewind {
    pub pending: PendingSuffixRewind,
    pub recorded_at: lettuce_types::TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupDynamicMemoryRun {
    pub run: DynamicMemoryRun,
    pub attempts: Vec<BackupDynamicMemoryAttempt>,
    pub summary_checkpoint: Option<DynamicMemorySummaryCheckpoint>,
    pub changed_item_ids: Vec<lettuce_types::MemoryId>,
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
        jobs: &crate::JobBackup,
        memory: &crate::MemoryBackup,
    ) -> Result<(), DynamicMemoryBackupError> {
        if self.version != DYNAMIC_MEMORY_BACKUP_VERSION {
            return Err(DynamicMemoryBackupError::InvalidData);
        }
        self.pending_approvals
            .sort_by_key(|approval| (approval.conversation_id, approval.branch_id));
        for entry in &mut self.runs {
            entry.changed_item_ids.sort();
            if entry.changed_item_ids.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(DynamicMemoryBackupError::InvalidData);
            }
        }
        self.runs.sort_by_key(|entry| entry.run.id);
        self.cycle_reverts.sort_by_key(|record| record.run_id);
        self.pending_suffix_rewinds.sort_by(|left, right| {
            (
                left.recorded_at,
                left.pending.tombstone.conversation_id,
                left.pending.tombstone.operation.key.as_str(),
            )
                .cmp(&(
                    right.recorded_at,
                    right.pending.tombstone.conversation_id,
                    right.pending.tombstone.operation.key.as_str(),
                ))
        });
        let conversations = history
            .conversations
            .iter()
            .map(|entry| entry.aggregate.conversation.id)
            .collect::<BTreeSet<_>>();
        let branches = history
            .conversations
            .iter()
            .flat_map(|entry| {
                entry
                    .aggregate
                    .branches
                    .iter()
                    .map(move |branch| (entry.aggregate.conversation.id, branch.id))
            })
            .collect::<BTreeSet<_>>();
        let spaces = memory
            .spaces
            .iter()
            .flat_map(|entry| {
                std::iter::once(entry.conversation_id)
                    .chain(entry.shared_conversation_ids.iter().copied())
                    .map(move |conversation_id| {
                        ((conversation_id, entry.snapshot.id), &entry.snapshot)
                    })
            })
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
        let mut approval_ids = BTreeSet::new();
        for approval in &self.pending_approvals {
            if !conversations.contains(&approval.conversation_id)
                || !history.conversations.iter().any(|conversation| {
                    conversation.aggregate.conversation.id == approval.conversation_id
                        && conversation
                            .aggregate
                            .branches
                            .iter()
                            .any(|branch| branch.id == approval.branch_id)
                })
                || !approval_ids.insert((approval.conversation_id, approval.branch_id))
                || approval.prompted_message_count == 0
                || approval.pending == approval.skipped
            {
                return Err(DynamicMemoryBackupError::InvalidData);
            }
        }
        let mut owed_keys = BTreeSet::new();
        for owed in &self.pending_suffix_rewinds {
            let tombstone = &owed.pending.tombstone;
            let removed = messages.get(&tombstone.message_id);
            if !owed_keys.insert((
                tombstone.conversation_id,
                tombstone.operation.key.as_str().to_owned(),
            )) || owed.pending.summary_message_interval == 0
                || tombstone.descendants != lettuce_conversations::DescendantPolicy::Tombstone
                || removed.is_none_or(|(conversation_id, _, _)| {
                    *conversation_id != tombstone.conversation_id
                })
                || !messages.contains_key(&owed.pending.after_message_id)
            {
                return Err(DynamicMemoryBackupError::InvalidData);
            }
        }
        let mut run_ids = BTreeSet::new();
        for entry in &mut self.runs {
            if entry.run.validate().is_err()
                || !run_ids.insert(entry.run.id)
                || !conversations.contains(&entry.run.conversation_id)
                || !branches.contains(&(entry.run.conversation_id, entry.run.branch_id))
                || !memory.spaces.iter().any(|space| {
                    space.snapshot.id == entry.run.space_id
                        && space
                            .branch_id
                            .is_none_or(|branch| branch == entry.run.branch_id)
                })
                || spaces
                    .get(&(entry.run.conversation_id, entry.run.space_id))
                    .is_none_or(|snapshot| entry.run.starting_memory.revision > snapshot.revision)
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
            validate_run(entry, &job_ids)?;
        }
        let by_run = self
            .runs
            .iter()
            .map(|entry| (entry.run.id, &entry.run))
            .collect::<BTreeMap<_, _>>();
        let mut reverted = BTreeSet::new();
        for record in &self.cycle_reverts {
            let Some(run) = by_run.get(&record.run_id) else {
                return Err(DynamicMemoryBackupError::InvalidData);
            };
            if record.validate().is_err()
                || !reverted.insert(record.run_id)
                || record.space_id != run.space_id
                || record.conversation_id != run.conversation_id
                || record.source_revision < run.starting_memory.revision
                || spaces
                    .get(&(run.conversation_id, run.space_id))
                    .is_none_or(|snapshot| record.resulting_revision > snapshot.revision)
                || record.restored_summary_run_id.is_some_and(|id| {
                    by_run
                        .get(&id)
                        .is_none_or(|prior| prior.space_id != run.space_id)
                })
            {
                return Err(DynamicMemoryBackupError::InvalidData);
            }
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
            || checkpoint.summary.token_count.is_none()
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DynamicMemoryBackupError {
    #[error("dynamic-memory backup contains invalid data")]
    InvalidData,
}
