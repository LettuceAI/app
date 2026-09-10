use std::collections::{BTreeMap, BTreeSet};

use lettuce_types::{GenerationAttemptId, UsageEventId};
use serde::{Deserialize, Serialize};

pub const CONVERSATION_USAGE_BACKUP_VERSION: u32 = 1;
pub const MAX_BACKUP_CONVERSATION_USAGE_EVENTS: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationUsageBackup {
    pub version: u32,
    pub events: Vec<BackupConversationUsage>,
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
        if self.version != CONVERSATION_USAGE_BACKUP_VERSION {
            return Err(ConversationUsageBackupError::InvalidData);
        }
        if self.events.len() > MAX_BACKUP_CONVERSATION_USAGE_EVENTS {
            return Err(ConversationUsageBackupError::LimitExceeded);
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
        let overlaps = jobs.inference.iter().fold(
            BTreeMap::<GenerationAttemptId, Vec<UsageEventId>>::new(),
            |mut values, dispatch| {
                values
                    .entry(dispatch.evidence.logical_attempt_id)
                    .or_default()
                    .push(dispatch.evidence.id);
                values
            },
        );
        let mut ids = BTreeSet::new();
        let mut owners = BTreeSet::new();
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
