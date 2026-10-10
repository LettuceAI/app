use std::collections::{BTreeMap, BTreeSet};

use lettuce_types::{GenerationAttemptId, UsageEventId};
use serde::{Deserialize, Serialize};

pub const CONVERSATION_USAGE_BACKUP_VERSION: u32 = 3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationUsageBackup {
    pub version: u32,
    pub events: Vec<BackupConversationUsage>,
    #[serde(default)]
    pub tombstones: Vec<lettuce_usage::UsageTombstone>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupConversationUsage {
    pub event: lettuce_usage::UsageEvent,
    pub cost_basis: Option<lettuce_usage::UsageCostBasis>,
    pub overlapping_job_inference_ids: Vec<UsageEventId>,
}

impl ConversationUsageBackup {
    pub fn canonicalize_and_validate(
        &mut self,
        runtime: &crate::ConversationRuntimeBackup,
        jobs: &crate::JobBackup,
    ) -> Result<(), ConversationUsageBackupError> {
        if !matches!(self.version, 1 | 2 | CONVERSATION_USAGE_BACKUP_VERSION)
            || (self.version == 1 && !self.tombstones.is_empty())
        {
            return Err(ConversationUsageBackupError::InvalidData);
        }
        self.events
            .sort_by_key(|entry| (entry.event.record.recorded_at, entry.event.id));
        let attempts = runtime
            .conversations
            .iter()
            .flat_map(|conversation| &conversation.turns)
            .flat_map(|turn| {
                turn.turn
                    .attempts
                    .iter()
                    .map(|attempt| ((turn.turn.id, attempt.id), attempt.usage_event_id))
            })
            .collect::<BTreeMap<_, _>>();
        let mut overlaps = jobs.inference.iter().fold(
            BTreeMap::<GenerationAttemptId, Vec<UsageEventId>>::new(),
            |mut values, dispatch| {
                values
                    .entry(dispatch.evidence.logical_attempt_id)
                    .or_default()
                    .push(dispatch.evidence.id);
                values
            },
        );
        for proof in &self.tombstones {
            if let lettuce_usage::UsageTombstone::Dispatch {
                event_id,
                attempt_id,
                ..
            } = proof
            {
                overlaps.entry(*attempt_id).or_default().push(*event_id);
            }
        }
        for ids in overlaps.values_mut() {
            ids.sort();
        }
        let mut ids = BTreeSet::new();
        let mut owners = BTreeSet::new();
        self.tombstones
            .sort_by_key(lettuce_usage::UsageTombstone::key);
        let mut tombstone_keys = BTreeSet::new();
        let runtime_owners = runtime
            .conversations
            .iter()
            .flat_map(|conversation| {
                conversation.turns.iter().flat_map(move |turn| {
                    turn.turn.attempts.iter().map(move |attempt| {
                        (
                            (turn.turn.id, attempt.id),
                            (
                                conversation.conversation_id,
                                attempt.usage_event_id,
                                attempt.job_id,
                            ),
                        )
                    })
                })
            })
            .collect::<BTreeMap<_, _>>();
        for proof in &self.tombstones {
            if !tombstone_keys.insert(proof.key()) {
                return Err(ConversationUsageBackupError::InvalidData);
            }
            match proof {
                lettuce_usage::UsageTombstone::Conversation {
                    event_id,
                    conversation_id,
                    turn_id,
                    attempt_id,
                    job_id,
                } => {
                    if !ids.insert(*event_id)
                        || !owners.insert((*turn_id, *attempt_id))
                        || runtime_owners
                            .get(&(*turn_id, *attempt_id))
                            .is_some_and(|owner| {
                                owner != &(*conversation_id, Some(*event_id), *job_id)
                            })
                        || (runtime
                            .conversations
                            .iter()
                            .any(|conversation| conversation.conversation_id == *conversation_id)
                            && !runtime_owners.contains_key(&(*turn_id, *attempt_id)))
                    {
                        return Err(ConversationUsageBackupError::InvalidData);
                    }
                }
                lettuce_usage::UsageTombstone::Dispatch {
                    event_id,
                    attempt_id,
                    job_id,
                } => {
                    if jobs
                        .inference
                        .iter()
                        .any(|entry| entry.evidence.id == *event_id)
                        || runtime_owners
                            .iter()
                            .any(|((_, id), (_, _, job))| id == attempt_id && job != &Some(*job_id))
                    {
                        return Err(ConversationUsageBackupError::InvalidData);
                    }
                }
                lettuce_usage::UsageTombstone::Legacy { run_id, source_id } => {
                    if run_id.is_empty() || source_id.trim().is_empty() {
                        return Err(ConversationUsageBackupError::InvalidData);
                    }
                }
            }
        }

        for entry in &mut self.events {
            entry.overlapping_job_inference_ids.sort();
            if !ids.insert(entry.event.id)
                || !owners.insert((entry.event.record.turn_id, entry.event.record.attempt_id))
                || entry.event.record.validate().is_err()
                || attempts.get(&(entry.event.record.turn_id, entry.event.record.attempt_id))
                    != Some(&Some(entry.event.id))
                || entry
                    .cost_basis
                    .as_ref()
                    .is_some_and(|basis| basis.calculate(&entry.event).is_err())
                || entry.overlapping_job_inference_ids
                    != overlaps
                        .get(&entry.event.record.attempt_id)
                        .cloned()
                        .unwrap_or_default()
            {
                return Err(ConversationUsageBackupError::InvalidData);
            }
        }
        if attempts
            .values()
            .flatten()
            .any(|usage_id| !ids.contains(usage_id))
        {
            return Err(ConversationUsageBackupError::InvalidData);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConversationUsageBackupError {
    #[error("conversation usage backup exceeds its limit")]
    LimitExceeded,
    #[error("conversation usage backup is invalid")]
    InvalidData,
}
