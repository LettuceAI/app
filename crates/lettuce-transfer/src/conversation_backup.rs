use std::collections::{BTreeMap, BTreeSet};

use lettuce_conversations::{
    ConversationAggregate, ConversationKind, InitialMessageOrigin, Message, MessageCandidate,
    MessagePart, MessageRenderSource, MessageRevision, MessageRole, SnapshotSelection,
    SnapshotSource, TrustedArtifactDescriptor, conversation_settings_snapshot_references,
    conversation_snapshot_references,
};
use lettuce_types::{MessageId, ReplayArtifactId, SnapshotArtifactId};
use serde::{Deserialize, Serialize};

pub const CONVERSATION_HISTORY_BACKUP_VERSION: u32 = 1;
pub const MAX_BACKUP_CONVERSATIONS: usize = 10_000;
pub const MAX_BACKUP_MESSAGES: usize = 200_000;
pub const MAX_BACKUP_MESSAGE_REVISIONS: usize = 400_000;
pub const MAX_BACKUP_MESSAGE_CANDIDATES: usize = 400_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationHistoryBackup {
    pub version: u32,
    pub conversations: Vec<BackupConversation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupConversation {
    pub aggregate: ConversationAggregate,
    pub messages: Vec<BackupMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupMessage {
    pub message: Message,
    pub timeline_ordinal: u64,
    pub initial_origin: Option<InitialMessageOrigin>,
    pub revisions: Vec<MessageRevision>,
    pub candidates: Vec<MessageCandidate>,
}

impl ConversationHistoryBackup {
    pub fn canonicalize_and_validate(
        &mut self,
        media_asset_ids: &BTreeSet<lettuce_types::AssetId>,
    ) -> Result<(), ConversationHistoryBackupError> {
        if self.version != CONVERSATION_HISTORY_BACKUP_VERSION
            || self.conversations.len() > MAX_BACKUP_CONVERSATIONS
        {
            return Err(ConversationHistoryBackupError::InvalidData);
        }
        self.conversations
            .sort_by_key(|value| value.aggregate.conversation.id);
        let mut conversation_ids = BTreeSet::new();
        let mut message_ids = BTreeSet::new();
        let mut revision_ids = BTreeSet::new();
        let mut candidate_ids = BTreeSet::new();
        let mut message_count = 0_usize;
        let mut revision_count = 0_usize;
        let mut candidate_count = 0_usize;
        for conversation in &mut self.conversations {
            let conversation_id = conversation.aggregate.conversation.id;
            if !conversation_ids.insert(conversation_id)
                || conversation.aggregate.validate().is_err()
            {
                return Err(ConversationHistoryBackupError::InvalidData);
            }
            conversation
                .messages
                .sort_by_key(|value| (value.timeline_ordinal, value.message.id));
            validate_conversation(
                conversation,
                media_asset_ids,
                &mut message_ids,
                &mut revision_ids,
                &mut candidate_ids,
            )?;
            message_count = message_count
                .checked_add(conversation.messages.len())
                .ok_or(ConversationHistoryBackupError::LimitExceeded)?;
            revision_count =
                conversation
                    .messages
                    .iter()
                    .try_fold(revision_count, |count, message| {
                        count
                            .checked_add(message.revisions.len())
                            .ok_or(ConversationHistoryBackupError::LimitExceeded)
                    })?;
            candidate_count =
                conversation
                    .messages
                    .iter()
                    .try_fold(candidate_count, |count, message| {
                        count
                            .checked_add(message.candidates.len())
                            .ok_or(ConversationHistoryBackupError::LimitExceeded)
                    })?;
        }
        if message_count > MAX_BACKUP_MESSAGES
            || revision_count > MAX_BACKUP_MESSAGE_REVISIONS
            || candidate_count > MAX_BACKUP_MESSAGE_CANDIDATES
        {
            return Err(ConversationHistoryBackupError::LimitExceeded);
        }
        Ok(())
    }

    pub(crate) fn artifact_descriptors(
        &self,
    ) -> Result<Vec<TrustedArtifactDescriptor>, ConversationHistoryBackupError> {
        let mut snapshots = BTreeMap::<SnapshotArtifactId, _>::new();
        let mut replays = BTreeMap::<ReplayArtifactId, _>::new();
        for backup in &self.conversations {
            let conversation = &backup.aggregate.conversation;
            for reference in conversation_snapshot_references(&conversation.kind) {
                insert_snapshot(&mut snapshots, reference)?;
            }
            for participant in &conversation.participants {
                if let SnapshotSelection::Inherited(model) | SnapshotSelection::Explicit(model) =
                    &participant.model_selection
                {
                    insert_snapshot(&mut snapshots, &model.snapshot_ref)?;
                }
            }
            if let ConversationKind::Group(details) = &conversation.kind {
                for policy in &details.initial_participant_policy.members {
                    if let SnapshotSelection::Inherited(model)
                    | SnapshotSelection::Explicit(model) = &policy.model_override
                    {
                        insert_snapshot(&mut snapshots, &model.snapshot_ref)?;
                    }
                }
            }
            if let Some(settings) = &conversation.current_settings {
                for reference in conversation_settings_snapshot_references(settings) {
                    insert_snapshot(&mut snapshots, reference)?;
                }
            }
            for message in &backup.messages {
                if let Some(origin) = &message.initial_origin {
                    let reference = match origin {
                        InitialMessageOrigin::SelectedScene { snapshot_ref }
                        | InitialMessageOrigin::StarterMessage { snapshot_ref, .. } => snapshot_ref,
                    };
                    insert_snapshot(&mut snapshots, reference)?;
                }
                for revision in &message.revisions {
                    if let Some(reference) = &revision.provider_replay {
                        insert_replay(&mut replays, reference)?;
                    }
                }
                for candidate in &message.candidates {
                    insert_snapshot(&mut snapshots, &candidate.model.snapshot_ref)?;
                    if let Some(reference) = &candidate.provider_replay {
                        insert_replay(&mut replays, reference)?;
                    }
                }
            }
        }
        Ok(snapshots
            .into_values()
            .map(TrustedArtifactDescriptor::Snapshot)
            .chain(replays.into_values().map(TrustedArtifactDescriptor::Replay))
            .collect())
    }
}

fn insert_snapshot(
    values: &mut BTreeMap<SnapshotArtifactId, lettuce_conversations::ProtectedSnapshotRef>,
    reference: &lettuce_conversations::ProtectedSnapshotRef,
) -> Result<(), ConversationHistoryBackupError> {
    if reference.validate().is_err()
        || values
            .insert(reference.artifact_id, reference.clone())
            .is_some_and(|existing| existing != *reference)
    {
        return Err(ConversationHistoryBackupError::InvalidData);
    }
    Ok(())
}

fn insert_replay(
    values: &mut BTreeMap<ReplayArtifactId, lettuce_conversations::ReplayArtifactRef>,
    reference: &lettuce_conversations::ReplayArtifactRef,
) -> Result<(), ConversationHistoryBackupError> {
    if reference.validate().is_err()
        || values
            .insert(reference.artifact_id, reference.clone())
            .is_some_and(|existing| existing != *reference)
    {
        return Err(ConversationHistoryBackupError::InvalidData);
    }
    Ok(())
}

fn validate_conversation(
    backup: &mut BackupConversation,
    media_asset_ids: &BTreeSet<lettuce_types::AssetId>,
    all_message_ids: &mut BTreeSet<MessageId>,
    all_revision_ids: &mut BTreeSet<lettuce_types::MessageRevisionId>,
    all_candidate_ids: &mut BTreeSet<lettuce_types::MessageCandidateId>,
) -> Result<(), ConversationHistoryBackupError> {
    let conversation_id = backup.aggregate.conversation.id;
    let branch_ids = backup
        .aggregate
        .branches
        .iter()
        .map(|branch| branch.id)
        .collect::<BTreeSet<_>>();
    let branches = backup
        .aggregate
        .branches
        .iter()
        .map(|branch| (branch.id, branch))
        .collect::<BTreeMap<_, _>>();
    let participant_ids = backup
        .aggregate
        .conversation
        .participants
        .iter()
        .map(|participant| participant.id)
        .collect::<BTreeSet<_>>();
    let local_messages = backup
        .messages
        .iter()
        .map(|entry| {
            (
                entry.message.id,
                (entry.message.branch_id, entry.timeline_ordinal),
            )
        })
        .collect::<BTreeMap<_, _>>();
    if local_messages.len() != backup.messages.len() {
        return Err(ConversationHistoryBackupError::InvalidData);
    }
    for branch in &backup.aggregate.branches {
        if branch.fork_message_id.is_some_and(|id| {
            local_messages
                .get(&id)
                .is_none_or(|(message_branch_id, _)| {
                    Some(*message_branch_id) != branch.parent_branch_id
                })
        }) || branch.head_message_id.is_some_and(|id| {
            local_messages
                .get(&id)
                .is_none_or(|(message_branch_id, _)| *message_branch_id != branch.id)
        }) {
            return Err(ConversationHistoryBackupError::InvalidData);
        }
    }
    let mut starter_ids = BTreeSet::new();
    let mut scene_origins = 0_usize;
    for (index, entry) in backup.messages.iter_mut().enumerate() {
        let expected_ordinal =
            u64::try_from(index + 1).map_err(|_| ConversationHistoryBackupError::LimitExceeded)?;
        if entry.timeline_ordinal != expected_ordinal
            || entry.message.conversation_id != conversation_id
            || !branch_ids.contains(&entry.message.branch_id)
            || !all_message_ids.insert(entry.message.id)
            || entry.message.validate().is_err()
            || entry
                .message
                .author_participant_id
                .is_some_and(|id| !participant_ids.contains(&id))
        {
            return Err(ConversationHistoryBackupError::InvalidData);
        }
        validate_parent_link(entry, &local_messages, &branches)?;
        validate_initial_origin(entry, &mut starter_ids, &mut scene_origins)?;
        entry.revisions.sort_by_key(|revision| revision.sequence);
        entry.candidates.sort_by_key(|candidate| candidate.ordinal);
        let mut revision_sequences = BTreeSet::new();
        for revision in &entry.revisions {
            if revision.message_id != entry.message.id
                || revision.validate().is_err()
                || !all_revision_ids.insert(revision.id)
                || !revision_sequences.insert(revision.sequence)
                || has_unknown_media(&revision.parts, media_asset_ids)
            {
                return Err(ConversationHistoryBackupError::InvalidData);
            }
        }
        let mut candidate_ordinals = BTreeSet::new();
        for candidate in &entry.candidates {
            if candidate.message_id != entry.message.id
                || candidate.validate().is_err()
                || !participant_ids.contains(&candidate.author_participant_id)
                || !all_candidate_ids.insert(candidate.id)
                || !candidate_ordinals.insert(candidate.ordinal)
                || has_unknown_media(&candidate.parts, media_asset_ids)
            {
                return Err(ConversationHistoryBackupError::InvalidData);
            }
        }
        let active_present = match entry.message.active_render_source {
            MessageRenderSource::Revision(id) => {
                entry.revisions.iter().any(|revision| revision.id == id)
            }
            MessageRenderSource::Candidate(id) => {
                entry.candidates.iter().any(|candidate| candidate.id == id)
            }
        };
        if !active_present {
            return Err(ConversationHistoryBackupError::InvalidData);
        }
    }
    Ok(())
}

fn validate_parent_link(
    entry: &BackupMessage,
    messages: &BTreeMap<MessageId, (lettuce_types::ConversationBranchId, u64)>,
    branches: &BTreeMap<
        lettuce_types::ConversationBranchId,
        &lettuce_conversations::ConversationBranch,
    >,
) -> Result<(), ConversationHistoryBackupError> {
    let branch = branches
        .get(&entry.message.branch_id)
        .ok_or(ConversationHistoryBackupError::InvalidData)?;
    let Some(parent_id) = entry.message.parent_message_id else {
        return if branch.parent_branch_id.is_none() && entry.timeline_ordinal == 1 {
            Ok(())
        } else {
            Err(ConversationHistoryBackupError::InvalidData)
        };
    };
    let (parent_branch_id, parent_ordinal) = messages
        .get(&parent_id)
        .ok_or(ConversationHistoryBackupError::InvalidData)?;
    if *parent_ordinal >= entry.timeline_ordinal {
        return Err(ConversationHistoryBackupError::InvalidData);
    }
    if *parent_branch_id != entry.message.branch_id
        && (branch.parent_branch_id != Some(*parent_branch_id)
            || branch.fork_message_id != Some(parent_id))
    {
        return Err(ConversationHistoryBackupError::InvalidData);
    }
    Ok(())
}

fn validate_initial_origin(
    entry: &BackupMessage,
    starter_ids: &mut BTreeSet<lettuce_types::StarterMessageId>,
    scene_origins: &mut usize,
) -> Result<(), ConversationHistoryBackupError> {
    let Some(origin) = &entry.initial_origin else {
        return Ok(());
    };
    match origin {
        InitialMessageOrigin::SelectedScene { snapshot_ref } => {
            *scene_origins += 1;
            if *scene_origins > 1
                || entry.timeline_ordinal != 1
                || entry.message.role != MessageRole::Scene
                || entry.message.author_participant_id.is_some()
                || !matches!(snapshot_ref.source, SnapshotSource::Scene(_))
                || snapshot_ref.validate().is_err()
            {
                return Err(ConversationHistoryBackupError::InvalidData);
            }
        }
        InitialMessageOrigin::StarterMessage {
            snapshot_ref,
            starter_message_id,
        } => {
            if !starter_ids.insert(*starter_message_id)
                || !matches!(
                    entry.message.role,
                    MessageRole::User | MessageRole::Assistant
                )
                || entry.message.author_participant_id.is_none()
                || !matches!(snapshot_ref.source, SnapshotSource::Starter(_))
                || snapshot_ref.validate().is_err()
            {
                return Err(ConversationHistoryBackupError::InvalidData);
            }
        }
    }
    Ok(())
}

fn has_unknown_media(
    parts: &[MessagePart],
    media_asset_ids: &BTreeSet<lettuce_types::AssetId>,
) -> bool {
    parts.iter().any(|part| match part {
        MessagePart::MediaAsset { asset_id, .. } => !media_asset_ids.contains(asset_id),
        _ => false,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConversationHistoryBackupError {
    #[error("conversation history backup exceeds its limit")]
    LimitExceeded,
    #[error("conversation history backup is invalid")]
    InvalidData,
}
