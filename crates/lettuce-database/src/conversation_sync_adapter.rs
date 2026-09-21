//! Conversation snapshots exchanged by sync.
//!
//! A conversation root carries what exists from creation on: the
//! conversation with its participants and settings, the root branch and the
//! initial (scene and starter) messages in their creation form. Values that
//! every device keeps for itself (conversation and branch revisions, update
//! times, branch heads, the active branch and timeline ordinals) are left out,
//! so a new message does not rewrite the root on every device.

use lettuce_conversations::{
    BranchStatus, Conversation, ConversationAggregate, ConversationBranch,
    ConversationRepositoryError, IdempotencyKey, InitialMessageOrigin, Message,
    MessageRenderSource, MessageRevision, MessageVisibility, OperationToken, ProtectedSnapshotRef,
    SnapshotSelection,
};
use lettuce_transfer::{BackupConversation, BackupMessage};
use lettuce_types::{CharacterId, ConversationId, Revision};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::conversation_history_writer as history;
use crate::conversation_vertical_slice as slice;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum SyncMemoryBinding {
    None,
    Conversation,
    CompanionPool(CharacterId),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SyncInitialMessage {
    pub message: Message,
    pub revision: MessageRevision,
    pub origin: InitialMessageOrigin,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SyncConversationRoot {
    pub conversation: Conversation,
    pub root_branch: ConversationBranch,
    pub memory: SyncMemoryBinding,
    pub initial_messages: Vec<SyncInitialMessage>,
}

fn storage(_: impl std::fmt::Debug) -> ConversationRepositoryError {
    ConversationRepositoryError::Storage
}

pub(crate) fn sync_conversation_ids(connection: &Connection) -> rusqlite::Result<Vec<String>> {
    connection
        .prepare("SELECT id FROM conversations ORDER BY id")?
        .query_map([], |row| row.get(0))?
        .collect()
}

fn normalized_branch(branch: &ConversationBranch) -> ConversationBranch {
    ConversationBranch {
        head_message_id: None,
        status: BranchStatus::Active,
        revision: Revision::INITIAL,
        updated_at: branch.created_at,
        ..branch.clone()
    }
}

fn normalized_initial(backup: &BackupMessage) -> Option<SyncInitialMessage> {
    let origin = backup.initial_origin.clone()?;
    let revision = backup
        .revisions
        .iter()
        .min_by_key(|revision| (revision.sequence, revision.id))?
        .clone();
    Some(SyncInitialMessage {
        message: Message {
            visibility: MessageVisibility::Visible,
            pinned: false,
            scene_edited: false,
            active_render_source: MessageRenderSource::Revision(revision.id),
            revision: Revision::INITIAL,
            updated_at: backup.message.created_at,
            ..backup.message.clone()
        },
        revision,
        origin,
    })
}

fn memory_binding(
    connection: &Connection,
    conversation_id: ConversationId,
) -> Result<SyncMemoryBinding, ConversationRepositoryError> {
    let binding: Option<Option<String>> = connection
        .query_row(
            "SELECT pool.character_id FROM conversation_memory_spaces binding
             LEFT JOIN companion_memory_pools pool ON pool.space_id = binding.space_id
             WHERE binding.conversation_id = ?1",
            [conversation_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage)?;
    Ok(match binding {
        None => SyncMemoryBinding::None,
        Some(None) => SyncMemoryBinding::Conversation,
        Some(Some(character)) => {
            SyncMemoryBinding::CompanionPool(character.parse().map_err(storage)?)
        }
    })
}

pub(crate) fn sync_load_conversation_root(
    transaction: &Transaction<'_>,
    id: &str,
) -> Result<Option<SyncConversationRoot>, ConversationRepositoryError> {
    let id = id.parse::<ConversationId>().map_err(storage)?;
    let exists: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
            [id.to_string()],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if !exists {
        return Ok(None);
    }
    let aggregate = slice::hydrate_conversation(transaction, id, || {})?;
    let root_branch = aggregate
        .branches
        .iter()
        .find(|branch| branch.parent_branch_id.is_none())
        .ok_or(ConversationRepositoryError::Storage)?;
    let messages =
        crate::backup_adapter::read_conversation_messages(transaction, id).map_err(storage)?;
    let conversation = Conversation {
        active_branch_id: root_branch.id,
        revision: Revision::INITIAL,
        updated_at: aggregate.conversation.created_at,
        ..aggregate.conversation.clone()
    };
    Ok(Some(SyncConversationRoot {
        conversation,
        root_branch: normalized_branch(root_branch),
        memory: memory_binding(transaction, id)?,
        initial_messages: messages.iter().filter_map(normalized_initial).collect(),
    }))
}

/// Every launch snapshot the root references.
pub(crate) fn root_snapshot_references(root: &SyncConversationRoot) -> Vec<ProtectedSnapshotRef> {
    let conversation = &root.conversation;
    let mut references =
        lettuce_conversations::conversation_snapshot_references(&conversation.kind)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
    for participant in &conversation.participants {
        if let SnapshotSelection::Inherited(model) | SnapshotSelection::Explicit(model) =
            &participant.model_selection
        {
            references.push(model.snapshot_ref.clone());
        }
    }
    if let Some(settings) = &conversation.current_settings {
        references.extend(
            lettuce_conversations::conversation_settings_snapshot_references(settings)
                .into_iter()
                .cloned(),
        );
    }
    for initial in &root.initial_messages {
        match &initial.origin {
            InitialMessageOrigin::SelectedScene { snapshot_ref }
            | InitialMessageOrigin::StarterMessage { snapshot_ref, .. } => {
                references.push(snapshot_ref.clone());
            }
        }
    }
    references
}

fn root_aggregate(root: &SyncConversationRoot) -> ConversationAggregate {
    ConversationAggregate {
        conversation: root.conversation.clone(),
        branches: vec![root.root_branch.clone()],
    }
}

/// Writes a synced conversation root. A new conversation is created with its
/// root branch, initial messages, launch snapshot references, memory binding
/// and create operation; an existing one takes the root's title, lifecycle,
/// participants and settings. A missing snapshot artifact reports `NotFound`
/// (it arrives as its own entity).
pub(crate) fn sync_replace_conversation_root(
    transaction: &Transaction<'_>,
    root: &SyncConversationRoot,
) -> Result<(), ConversationRepositoryError> {
    let conversation = &root.conversation;
    for reference in root_snapshot_references(root) {
        let present: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM conversation_snapshot_artifacts WHERE artifact_id = ?1)",
                [reference.artifact_id.to_string()],
                |row| row.get(0),
            )
            .map_err(storage)?;
        if !present {
            return Err(ConversationRepositoryError::NotFound);
        }
    }
    if let SyncMemoryBinding::CompanionPool(character) = &root.memory {
        let present: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM characters WHERE id = ?1)",
                [character.to_string()],
                |row| row.get(0),
            )
            .map_err(storage)?;
        if !present {
            return Err(ConversationRepositoryError::NotFound);
        }
    }
    let exists: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
            [conversation.id.to_string()],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if exists {
        transaction
            .execute(
                "UPDATE conversations SET lifecycle = ?2, title = ?3, revision = revision + 1 WHERE id = ?1",
                params![
                    conversation.id.to_string(),
                    slice::lifecycle_name(conversation.lifecycle),
                    conversation.title,
                ],
            )
            .map_err(storage)?;
        slice::save_participants(transaction, conversation)?;
        slice::save_settings(transaction, conversation)?;
        if let Some(settings) = &conversation.current_settings {
            for reference in
                lettuce_conversations::conversation_settings_snapshot_references(settings)
            {
                transaction
                    .execute(
                        "INSERT OR IGNORE INTO conversation_snapshot_refs (conversation_id, artifact_id) VALUES (?1, ?2)",
                        params![conversation.id.to_string(), reference.artifact_id.to_string()],
                    )
                    .map_err(storage)?;
            }
        }
        return Ok(());
    }
    slice::save_conversation(transaction, conversation)?;
    match &root.memory {
        SyncMemoryBinding::None => {}
        SyncMemoryBinding::Conversation => {
            crate::memory_adapter::create_conversation_space_in(transaction, conversation.id)?;
        }
        SyncMemoryBinding::CompanionPool(character) => {
            crate::memory_adapter::bind_companion_pool_in(
                transaction,
                conversation.id,
                *character,
            )?;
        }
    }
    history::insert_branch(transaction, &root.root_branch)?;
    let backups = root
        .initial_messages
        .iter()
        .enumerate()
        .map(|(index, initial)| BackupMessage {
            message: initial.message.clone(),
            timeline_ordinal: u64::try_from(index).unwrap_or(u64::MAX).saturating_add(1),
            initial_origin: Some(initial.origin.clone()),
            revisions: vec![initial.revision.clone()],
            candidates: Vec::new(),
            historical_media_revision_ids: Vec::new(),
            historical_media_candidate_ids: Vec::new(),
        })
        .collect::<Vec<_>>();
    let aggregate = root_aggregate(root);
    history::insert_snapshot_refs(
        transaction,
        &BackupConversation {
            aggregate: aggregate.clone(),
            messages: backups.clone(),
        },
    )?;
    for backup in &backups {
        history::insert_message(transaction, backup)?;
        history::insert_revision(transaction, backup, &backup.revisions[0])?;
        if let Some(origin) = &backup.initial_origin {
            history::insert_origin(transaction, backup, origin)?;
        }
    }
    history::set_branch_head(
        transaction,
        conversation.id,
        root.root_branch.id,
        backups.last().map(|backup| backup.message.id),
    )?;
    transaction
        .execute(
            "UPDATE conversations SET next_timeline_ordinal = ?1 WHERE id = ?2",
            params![
                i64::try_from(backups.len()).map_err(storage)? + 1,
                conversation.id.to_string()
            ],
        )
        .map_err(storage)?;
    let token = OperationToken {
        key: IdempotencyKey::new(format!("sync-create-{}", conversation.id)).map_err(storage)?,
        request_digest: lettuce_types::ContentHash::parse(
            blake3::hash(conversation.id.to_string().as_bytes())
                .to_hex()
                .to_string(),
        )
        .map_err(storage)?,
    };
    history::insert_creation_record(
        transaction,
        &aggregate,
        &backups.iter().collect::<Vec<_>>(),
        &token,
    )?;
    Ok(())
}
