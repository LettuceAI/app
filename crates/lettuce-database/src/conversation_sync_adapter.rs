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
    ConversationRepositoryError, GenerationInput, GenerationTarget, GenerationTurn,
    GenerationTurnStatus, IdempotencyKey, InitialMessageOrigin, Message, MessageRenderSource,
    MessageRevision, MessageVisibility, OperationToken, ProtectedSnapshotRef, SnapshotSelection,
};
use lettuce_transfer::{BackupConversation, BackupMessage};
use lettuce_types::{
    CharacterId, ConversationBranchId, ConversationId, MessageId, Revision, TimestampMillis,
};
use lettuce_usage::UsageEvent;
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
    let initial_ids = transaction
        .prepare(
            "SELECT message_id FROM conversation_initial_message_origins WHERE conversation_id = ?1",
        )
        .and_then(|mut statement| {
            statement
                .query_map([id.to_string()], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(storage)?;
    let mut messages = Vec::with_capacity(initial_ids.len());
    for message_id in initial_ids {
        if let Some(message) = crate::backup_adapter::read_conversation_message(
            transaction,
            id,
            message_id.parse().map_err(storage)?,
        )
        .map_err(storage)?
        {
            messages.push(message);
        }
    }
    messages.sort_by_key(|message| (message.timeline_ordinal, message.message.id));
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
    if let Some(lettuce_conversations::ConversationBackground::Image { asset_id }) = conversation
        .current_settings
        .as_ref()
        .and_then(|settings| settings.background)
        && !exists(
            transaction,
            "SELECT EXISTS(SELECT 1 FROM media_assets WHERE id = ?1)",
            [asset_id.to_string()],
        )?
    {
        return Err(ConversationRepositoryError::NotFound);
    }
    let present: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
            [conversation.id.to_string()],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if present {
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

/// One message as sync exchanges it: every revision and candidate, the
/// terminal turns that produced the candidates and their usage. Timeline
/// ordinals, message revisions and provider replay artifacts stay on the
/// device that wrote them, and so does the retry link to a failed turn that
/// produced nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SyncConversationMessage {
    pub message: BackupMessage,
    pub turns: Vec<GenerationTurn>,
    pub usage: Vec<UsageEvent>,
}

pub(crate) fn sync_message_ids(connection: &Connection) -> rusqlite::Result<Vec<String>> {
    connection
        .prepare(
            "SELECT message.conversation_id || ':' || message.id FROM conversation_messages message
              WHERE NOT EXISTS (
                  SELECT 1 FROM sync_conversation_marks mark
                   WHERE mark.conversation_id = message.conversation_id AND mark.changed = mark.scanned
              )
              ORDER BY message.conversation_id, message.timeline_ordinal, message.id",
        )?
        .query_map([], |row| row.get(0))?
        .collect()
}

pub(crate) fn sync_message_id(id: &str) -> Option<(ConversationId, MessageId)> {
    let (conversation, message) = id.split_once(':')?;
    Some((conversation.parse().ok()?, message.parse().ok()?))
}

fn terminal(status: GenerationTurnStatus) -> bool {
    matches!(
        status,
        GenerationTurnStatus::Succeeded
            | GenerationTurnStatus::Failed
            | GenerationTurnStatus::Cancelled
            | GenerationTurnStatus::Interrupted
    )
}

fn live_turn(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    message_id: MessageId,
) -> Result<bool, ConversationRepositoryError> {
    exists(
        transaction,
        "SELECT EXISTS(SELECT 1 FROM conversation_turns WHERE conversation_id = ?1 AND status NOT IN ('succeeded', 'failed', 'cancelled', 'interrupted') AND (target_message_id = ?2 OR user_message_id = ?2 OR head_message_id = ?2 OR candidate_message_id = ?2))",
        params![conversation_id.to_string(), message_id.to_string()],
    )
}

/// The message in its exchanged form, or `None` while a turn on it is still
/// running. Revision sequences and candidate ordinals are numbered by each
/// device, so the exchanged form orders both by creation instead.
pub(crate) fn sync_load_conversation_message(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    message_id: MessageId,
) -> Result<Option<SyncConversationMessage>, ConversationRepositoryError> {
    if live_turn(transaction, conversation_id, message_id)? {
        return Ok(None);
    }
    let Some(mut backup) =
        crate::backup_adapter::read_conversation_message(transaction, conversation_id, message_id)
            .map_err(storage)?
    else {
        return Ok(None);
    };
    let mut turns =
        crate::backup_adapter::read_candidate_turns(transaction, conversation_id, message_id)
            .map_err(storage)?;
    if turns.iter().any(|turn| !terminal(turn.status)) {
        return Ok(None);
    }
    backup.timeline_ordinal = 0;
    backup.message.revision = Revision::INITIAL;
    backup.message.updated_at = backup.message.created_at;
    backup
        .revisions
        .sort_by_key(|revision| (revision.authored_at, revision.id));
    for revision in &mut backup.revisions {
        revision.provider_replay = None;
        revision.sequence = Revision::INITIAL;
    }
    backup
        .candidates
        .sort_by_key(|candidate| (candidate.created_at, candidate.id));
    for candidate in &mut backup.candidates {
        candidate.provider_replay = None;
        candidate.ordinal = 0;
    }
    let mut usage = Vec::new();
    for turn in &mut turns {
        turn.retry_of_turn_id = None;
        let settled = turn
            .attempts
            .iter()
            .filter_map(|attempt| attempt.usage_event_id)
            .collect::<Vec<_>>();
        usage.extend(
            crate::usage_adapter::load_turn_usage_in(transaction, &turn.id.to_string())
                .map_err(storage)?
                .into_iter()
                .filter(|event| settled.contains(&event.id)),
        );
    }
    Ok(Some(SyncConversationMessage {
        message: backup,
        turns,
        usage,
    }))
}

fn exists(
    transaction: &Transaction<'_>,
    sql: &str,
    values: impl rusqlite::Params,
) -> Result<bool, ConversationRepositoryError> {
    transaction
        .query_row(sql, values, |row| row.get(0))
        .map_err(storage)
}

fn require_references(
    transaction: &Transaction<'_>,
    incoming: &SyncConversationMessage,
) -> Result<(), ConversationRepositoryError> {
    let backup = &incoming.message;
    let parts = backup
        .revisions
        .iter()
        .map(|revision| &revision.parts)
        .chain(backup.candidates.iter().map(|candidate| &candidate.parts));
    for parts in parts {
        for (_, asset, _) in history::media_parts(parts)? {
            if !exists(
                transaction,
                "SELECT EXISTS(SELECT 1 FROM media_assets WHERE id = ?1)",
                [asset],
            )? {
                return Err(ConversationRepositoryError::NotFound);
            }
        }
    }
    let message = &backup.message;
    let participants = message
        .author_participant_id
        .into_iter()
        .chain(
            backup
                .candidates
                .iter()
                .map(|candidate| candidate.author_participant_id),
        )
        .chain(incoming.turns.iter().flat_map(|turn| {
            turn.forced_speaker.into_iter().chain(
                turn.selected_speaker
                    .as_ref()
                    .map(|speaker| speaker.participant_id),
            )
        }));
    for participant in participants {
        if !exists(
            transaction,
            "SELECT EXISTS(SELECT 1 FROM conversation_participants WHERE conversation_id = ?1 AND id = ?2)",
            params![message.conversation_id.to_string(), participant.to_string()],
        )? {
            return Err(ConversationRepositoryError::NotFound);
        }
    }
    for turn in &incoming.turns {
        if let Some(prompt) = &turn.prompt
            && !exists(
                transaction,
                "SELECT EXISTS(SELECT 1 FROM prompt_documents WHERE id = ?1)",
                [prompt.document_id.to_string()],
            )?
        {
            return Err(ConversationRepositoryError::NotFound);
        }
        for lorebook in &turn.lorebooks {
            if !exists(
                transaction,
                "SELECT EXISTS(SELECT 1 FROM lorebooks WHERE id = ?1)",
                [lorebook.lorebook_id.to_string()],
            )? {
                return Err(ConversationRepositoryError::NotFound);
            }
        }
    }
    Ok(())
}

fn invalid_message() -> ConversationRepositoryError {
    ConversationRepositoryError::Invalid(lettuce_conversations::ValidationError::InvalidValue {
        field: "sync.message",
    })
}

/// Merges a synced message. Revisions, candidates and their turns are
/// unioned, the render pointer, author and flags follow the incoming
/// snapshot, and a tombstone is never lifted. A new message takes the next
/// local timeline ordinal and becomes its branch head when it extends the
/// current head. Anything it depends on that has not arrived yet reports
/// `NotFound`.
pub(crate) fn sync_merge_conversation_message(
    transaction: &Transaction<'_>,
    incoming: &SyncConversationMessage,
) -> Result<(), ConversationRepositoryError> {
    let message = &incoming.message.message;
    let conversation_id = message.conversation_id;
    let kind: Option<String> = transaction
        .query_row(
            "SELECT kind FROM conversations WHERE id = ?1",
            [conversation_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage)?;
    let Some(kind) = kind else {
        return Err(ConversationRepositoryError::NotFound);
    };
    for turn in &incoming.turns {
        turn.validate(kind == "group")
            .map_err(ConversationRepositoryError::Invalid)?;
        if turn.conversation_id != conversation_id || !terminal(turn.status) {
            return Err(invalid_message());
        }
    }
    let local = sync_load_conversation_message(transaction, conversation_id, message.id)?;
    if local.as_ref() == Some(incoming) {
        return Ok(());
    }
    require_references(transaction, incoming)?;
    let evidence = history::Evidence::usage_only(&incoming.usage);
    let exists_locally = exists(
        transaction,
        "SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2)",
        params![conversation_id.to_string(), message.id.to_string()],
    )?;
    if exists_locally {
        let Some(local) = local else {
            return Err(ConversationRepositoryError::NotFound);
        };
        merge_message(transaction, incoming, &local, &evidence)?;
        refresh_copies(transaction, conversation_id, message.id)?;
    } else {
        insert_synced_message(transaction, incoming, &evidence)?;
    }
    let tombstoned = exists(
        transaction,
        "SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2 AND visibility = 'tombstoned')",
        params![conversation_id.to_string(), message.id.to_string()],
    )?;
    let backup = &incoming.message;
    for revision in &backup.revisions {
        let historical = tombstoned || backup.historical_media_revision_ids.contains(&revision.id);
        transaction
            .execute(
                "UPDATE revision_media_refs SET state = ?3 WHERE conversation_id = ?1 AND message_revision_id = ?2",
                params![
                    conversation_id.to_string(),
                    revision.id.to_string(),
                    if historical { "historical" } else { "active" },
                ],
            )
            .map_err(storage)?;
    }
    for candidate in &backup.candidates {
        let historical = tombstoned
            || backup
                .historical_media_candidate_ids
                .contains(&candidate.id);
        transaction
            .execute(
                "UPDATE candidate_media_refs SET state = ?3 WHERE conversation_id = ?1 AND candidate_id = ?2",
                params![
                    conversation_id.to_string(),
                    candidate.id.to_string(),
                    if historical { "historical" } else { "active" },
                ],
            )
            .map_err(storage)?;
    }
    transaction
        .execute(
            "UPDATE conversations SET revision = revision + 1 WHERE id = ?1",
            [conversation_id.to_string()],
        )
        .map_err(storage)?;
    Ok(())
}

fn insert_synced_message(
    transaction: &Transaction<'_>,
    incoming: &SyncConversationMessage,
    evidence: &history::Evidence<'_>,
) -> Result<(), ConversationRepositoryError> {
    let message = &incoming.message.message;
    let conversation_id = message.conversation_id;
    if incoming.message.initial_origin.is_some() {
        return Err(ConversationRepositoryError::NotFound);
    }
    let branch: Option<(Option<String>, Option<String>)> = transaction
        .query_row(
            "SELECT head_message_id, fork_message_id FROM conversation_branches WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id.to_string(), message.branch_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(storage)?;
    let Some((head, fork)) = branch else {
        return Err(ConversationRepositoryError::NotFound);
    };
    let head = head
        .map(|id| id.parse::<MessageId>())
        .transpose()
        .map_err(storage)?;
    let fork = fork
        .map(|id| id.parse::<MessageId>())
        .transpose()
        .map_err(storage)?;
    if let Some(parent) = message.parent_message_id
        && !exists(
            transaction,
            "SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2)",
            params![conversation_id.to_string(), parent.to_string()],
        )?
    {
        return Err(ConversationRepositoryError::NotFound);
    }
    let next: i64 = transaction
        .query_row(
            "SELECT next_timeline_ordinal FROM conversations WHERE id = ?1",
            [conversation_id.to_string()],
            |row| row.get(0),
        )
        .map_err(storage)?;
    let mut backup = incoming.message.clone();
    backup.timeline_ordinal = u64::try_from(next).map_err(storage)?;
    for (index, revision) in backup.revisions.iter_mut().enumerate() {
        revision.sequence = Revision::new(u64::try_from(index).map_err(storage)? + 1);
    }
    for (index, candidate) in backup.candidates.iter_mut().enumerate() {
        candidate.ordinal = u16::try_from(index).map_err(storage)?;
    }
    let continued = incoming.turns.iter().find_map(|turn| match turn.input {
        GenerationInput::ExistingHead { head_message_id }
            if matches!(turn.target, GenerationTarget::NewAssistant { .. }) =>
        {
            Some(head_message_id)
        }
        _ => None,
    });
    if continued.is_some() {
        history::set_branch_head(transaction, conversation_id, message.branch_id, continued)?;
    }
    let mut turns = incoming.turns.iter().collect::<Vec<_>>();
    turns.sort_by_key(|turn| !matches!(turn.target, GenerationTarget::NewAssistant { .. }));
    history::insert_message_with_turns(transaction, &backup, &turns, evidence)?;
    let extends =
        head == message.parent_message_id || (head.is_none() && message.parent_message_id == fork);
    history::set_branch_head(
        transaction,
        conversation_id,
        message.branch_id,
        if extends { Some(message.id) } else { head },
    )?;
    transaction
        .execute(
            "UPDATE conversations SET next_timeline_ordinal = ?1 WHERE id = ?2",
            params![next + 1, conversation_id.to_string()],
        )
        .map_err(storage)?;
    if !extends {
        settle_concurrent_message(transaction, message, head)?;
    }
    Ok(())
}

fn merge_message(
    transaction: &Transaction<'_>,
    incoming: &SyncConversationMessage,
    local: &SyncConversationMessage,
    evidence: &history::Evidence<'_>,
) -> Result<(), ConversationRepositoryError> {
    let message = &incoming.message.message;
    let current = &local.message.message;
    if message.branch_id != current.branch_id
        || message.parent_message_id != current.parent_message_id
        || message.role != current.role
        || message.created_at != current.created_at
    {
        return Err(invalid_message());
    }
    let conversation_id = message.conversation_id;
    let (last_sequence, last_ordinal): (i64, i64) = transaction
        .query_row(
            "SELECT (SELECT COALESCE(max(sequence), 0) FROM conversation_message_revisions WHERE conversation_id = ?1 AND message_id = ?2), (SELECT COALESCE(max(ordinal), -1) FROM conversation_message_candidates WHERE conversation_id = ?1 AND message_id = ?2)",
            params![conversation_id.to_string(), message.id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(storage)?;
    let known_candidates = local
        .message
        .candidates
        .iter()
        .map(|candidate| candidate.id)
        .collect::<Vec<_>>();
    let mut numbered = incoming.message.clone();
    let mut ordinal = last_ordinal;
    for candidate in &mut numbered.candidates {
        if !known_candidates.contains(&candidate.id) {
            ordinal += 1;
            candidate.ordinal = u16::try_from(ordinal).map_err(storage)?;
        }
    }
    let mut sequence = u64::try_from(last_sequence).map_err(storage)?;
    for turn in &incoming.turns {
        let present = exists(
            transaction,
            "SELECT EXISTS(SELECT 1 FROM conversation_turns WHERE conversation_id = ?1 AND id = ?2)",
            params![conversation_id.to_string(), turn.id.to_string()],
        )?;
        if present {
            continue;
        }
        if matches!(turn.target, GenerationTarget::NewAssistant { .. }) {
            return Err(invalid_message());
        }
        history::insert_turn(transaction, turn)?;
        history::settle_turn(transaction, Some(&numbered), turn, evidence)?;
    }
    let known = local
        .message
        .revisions
        .iter()
        .map(|revision| revision.id)
        .collect::<Vec<_>>();
    for revision in &incoming.message.revisions {
        if !known.contains(&revision.id) {
            sequence += 1;
            history::insert_revision(
                transaction,
                &incoming.message,
                &MessageRevision {
                    sequence: Revision::new(sequence),
                    ..revision.clone()
                },
            )?;
        }
    }
    let (revision_id, candidate_id) = match message.active_render_source {
        MessageRenderSource::Revision(id) => (Some(id.to_string()), None),
        MessageRenderSource::Candidate(id) => (None, Some(id.to_string())),
    };
    let visibility = if current.visibility == MessageVisibility::Tombstoned {
        MessageVisibility::Tombstoned
    } else {
        message.visibility
    };
    transaction
        .execute(
            "UPDATE conversation_messages SET author_participant_id = ?3, active_revision_id = ?4, active_candidate_id = ?5, visibility = ?6, pinned = ?7, scene_edited = ?8, logical_time = ?9, effective_time = ?10, revision = revision + 1 WHERE conversation_id = ?1 AND id = ?2",
            params![
                conversation_id.to_string(),
                message.id.to_string(),
                message.author_participant_id.map(|id| id.to_string()),
                revision_id,
                candidate_id,
                crate::conversation_mutation_kernel::message_visibility_name(visibility),
                i64::from(message.pinned),
                i64::from(message.scene_edited),
                message.logical_time.get(),
                message.effective_time.get(),
            ],
        )
        .map_err(crate::conversation_mutation_kernel::map_constraint)?;
    Ok(())
}

pub(crate) fn sync_branch_ids(connection: &Connection) -> rusqlite::Result<Vec<String>> {
    connection
        .prepare(
            "SELECT conversation_id || ':' || id FROM conversation_branches WHERE parent_branch_id IS NOT NULL ORDER BY conversation_id, created_at, id",
        )?
        .query_map([], |row| row.get(0))?
        .collect()
}

pub(crate) fn sync_branch_id(id: &str) -> Option<(ConversationId, ConversationBranchId)> {
    let (conversation, branch) = id.split_once(':')?;
    Some((conversation.parse().ok()?, branch.parse().ok()?))
}

/// A forked branch in its creation form; its head follows the messages that
/// arrive on it.
pub(crate) fn sync_load_branch(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
) -> Result<Option<ConversationBranch>, ConversationRepositoryError> {
    let exists = exists(
        transaction,
        "SELECT EXISTS(SELECT 1 FROM conversation_branches WHERE conversation_id = ?1 AND id = ?2 AND parent_branch_id IS NOT NULL)",
        params![conversation_id.to_string(), branch_id.to_string()],
    )?;
    if !exists {
        return Ok(None);
    }
    let aggregate = slice::hydrate_conversation(transaction, conversation_id, || {})?;
    Ok(aggregate
        .branches
        .iter()
        .find(|branch| branch.id == branch_id)
        .map(normalized_branch))
}

/// Inserts a synced fork once its parent branch and fork message exist.
/// Branches never change after creation, so an existing one is kept.
pub(crate) fn sync_insert_branch(
    transaction: &Transaction<'_>,
    branch: &ConversationBranch,
) -> Result<(), ConversationRepositoryError> {
    let conversation_id = branch.conversation_id.to_string();
    if exists(
        transaction,
        "SELECT EXISTS(SELECT 1 FROM conversation_branches WHERE conversation_id = ?1 AND id = ?2)",
        params![conversation_id, branch.id.to_string()],
    )? {
        return Ok(());
    }
    let (Some(parent), Some(fork)) = (branch.parent_branch_id, branch.fork_message_id) else {
        return Err(invalid_message());
    };
    if !exists(
        transaction,
        "SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2 AND branch_id = ?3)",
        params![conversation_id, fork.to_string(), parent.to_string()],
    )? {
        return Err(ConversationRepositoryError::NotFound);
    }
    history::insert_branch(transaction, &normalized_branch(branch))?;
    transaction
        .execute(
            "UPDATE conversations SET revision = revision + 1 WHERE id = ?1",
            [conversation_id],
        )
        .map_err(storage)?;
    Ok(())
}

type MessageLink = (Option<MessageId>, ConversationBranchId, TimestampMillis);

fn message_link(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    message_id: MessageId,
) -> Result<MessageLink, ConversationRepositoryError> {
    let (parent, branch, created_at): (Option<String>, String, i64) = transaction
        .query_row(
            "SELECT parent_message_id, branch_id, created_at FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id.to_string(), message_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(storage)?;
    Ok((
        parent.map(|id| id.parse()).transpose().map_err(storage)?,
        branch.parse().map_err(storage)?,
        TimestampMillis::new(created_at),
    ))
}

fn fork_branch_id(conversation_id: ConversationId, first: MessageId) -> ConversationBranchId {
    ConversationBranchId::from_uuid(uuid::Uuid::new_v5(
        &conversation_id.as_uuid(),
        format!("sync-fork:{first}").as_bytes(),
    ))
}

fn fork_copy_id(branch_id: ConversationBranchId, original: MessageId) -> MessageId {
    MessageId::from_uuid(uuid::Uuid::new_v5(
        &branch_id.as_uuid(),
        original.to_string().as_bytes(),
    ))
}

/// Resolves a synced message that does not extend its branch head. When it
/// and the local path answer the same message, the lower message id keeps
/// the path and the other chain is copied, flattened to what it shows, into
/// a fork branch named after the chain's first message, so every device
/// builds the same fork; the user is told when one side is this device's
/// own. A message that continues a chain already moved to a fork is copied
/// after its parent's nearest copy. Nothing moves while a generation runs.
fn settle_concurrent_message(
    transaction: &Transaction<'_>,
    message: &Message,
    head: Option<MessageId>,
) -> Result<(), ConversationRepositoryError> {
    let conversation_id = message.conversation_id;
    let (Some(parent), Some(head)) = (message.parent_message_id, head) else {
        return Ok(());
    };
    if exists(
        transaction,
        "SELECT EXISTS(SELECT 1 FROM conversation_turns WHERE conversation_id = ?1 AND status NOT IN ('succeeded', 'failed', 'cancelled', 'interrupted'))",
        [conversation_id.to_string()],
    )? {
        return Err(ConversationRepositoryError::NotFound);
    }
    let mut path = Vec::new();
    let mut cursor = Some(head);
    while let Some(id) = cursor {
        let (next, branch, _) = message_link(transaction, conversation_id, id)?;
        if branch != message.branch_id {
            break;
        }
        path.push((id, next));
        cursor = next;
    }
    if let Some(index) = path.iter().position(|(_, next)| *next == Some(parent)) {
        let contender = path[..=index]
            .iter()
            .rev()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        let local = authored_locally(transaction, conversation_id, &contender)?;
        if message.id < contender[0] {
            history::set_branch_head(
                transaction,
                conversation_id,
                message.branch_id,
                Some(message.id),
            )?;
            return copy_chain(
                transaction,
                conversation_id,
                parent,
                &contender,
                local.then_some(true),
            );
        }
        return copy_chain(
            transaction,
            conversation_id,
            parent,
            &[message.id],
            local.then_some(false),
        );
    }
    let on_path = |id: MessageId| {
        path.iter().any(|(step, _)| *step == id)
            || path.last().and_then(|(_, next)| *next) == Some(id)
    };
    let mut ancestor = parent;
    loop {
        let (above, branch, _) = message_link(transaction, conversation_id, ancestor)?;
        if branch != message.branch_id {
            return Ok(());
        }
        let fork = fork_branch_id(conversation_id, ancestor);
        let copied_parent = fork_copy_id(fork, parent);
        if exists(
            transaction,
            "SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2)",
            params![conversation_id.to_string(), copied_parent.to_string()],
        )? {
            return copy_messages(
                transaction,
                conversation_id,
                fork,
                copied_parent,
                &[message.id],
            );
        }
        match above {
            Some(above) if !on_path(above) => ancestor = above,
            _ => return Ok(()),
        }
    }
}

fn authored_locally(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    messages: &[MessageId],
) -> Result<bool, ConversationRepositoryError> {
    for message in messages {
        if exists(
            transaction,
            "SELECT EXISTS(SELECT 1 FROM sync_changes change JOIN sync_local_state local ON local.device_id = change.origin_device_id WHERE change.entity_kind = 'conversation_message' AND change.operation = 'insert' AND change.entity_id = ?1)",
            [format!("{conversation_id}:{message}")],
        )? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Copies a chain into its fork. `notice` records the fork for the user
/// (whether it holds this device's former path) when either side is this
/// device's own.
fn copy_chain(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    fork_point: MessageId,
    chain: &[MessageId],
    notice: Option<bool>,
) -> Result<(), ConversationRepositoryError> {
    let Some(first) = chain.first().copied() else {
        return Ok(());
    };
    let fork = fork_branch_id(conversation_id, first);
    let (_, fork_point_branch, _) = message_link(transaction, conversation_id, fork_point)?;
    let (_, _, created_at) = message_link(transaction, conversation_id, first)?;
    let branch = ConversationBranch {
        id: fork,
        conversation_id,
        parent_branch_id: Some(fork_point_branch),
        fork_message_id: Some(fork_point),
        head_message_id: None,
        status: BranchStatus::Active,
        revision: Revision::INITIAL,
        created_at,
        updated_at: created_at,
    };
    if !exists(
        transaction,
        "SELECT EXISTS(SELECT 1 FROM conversation_branches WHERE conversation_id = ?1 AND id = ?2)",
        params![conversation_id.to_string(), fork.to_string()],
    )? {
        history::insert_branch(transaction, &branch)?;
    }
    if let Some(holds_local) = notice {
        transaction
            .execute(
                "INSERT OR IGNORE INTO sync_conversation_forks (conversation_id, branch_id, holds_local, detected_at) VALUES (?1, ?2, ?3, ?4)",
                params![
                    conversation_id.to_string(),
                    fork.to_string(),
                    i64::from(holds_local),
                    TimestampMillis::now().map_err(storage)?.get(),
                ],
            )
            .map_err(storage)?;
    }
    copy_messages(transaction, conversation_id, fork, fork_point, chain)
}

/// What a copy shows: the original's rendered parts as one revision whose id
/// derives from the content, so devices that copied the same content agree.
fn flattened_revision(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    original: MessageId,
    copy_id: MessageId,
) -> Result<(BackupMessage, MessageRevision), ConversationRepositoryError> {
    let source =
        crate::backup_adapter::read_conversation_message(transaction, conversation_id, original)
            .map_err(storage)?
            .ok_or(ConversationRepositoryError::NotFound)?;
    let parts = match source.message.active_render_source {
        MessageRenderSource::Revision(id) => source
            .revisions
            .iter()
            .find(|revision| revision.id == id)
            .map(|revision| revision.parts.clone()),
        MessageRenderSource::Candidate(id) => source
            .candidates
            .iter()
            .find(|candidate| candidate.id == id)
            .map(|candidate| candidate.parts.clone()),
    }
    .ok_or(ConversationRepositoryError::Storage)?;
    let revision = MessageRevision {
        id: lettuce_types::MessageRevisionId::from_uuid(uuid::Uuid::new_v5(
            &copy_id.as_uuid(),
            &slice::encode(&parts)?.into_bytes(),
        )),
        message_id: copy_id,
        sequence: Revision::INITIAL,
        parts,
        authored_at: source.message.created_at,
        source_turn_id: None,
        provider_replay: None,
    };
    Ok((source, revision))
}

fn settle_copy_media(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    copy_id: MessageId,
) -> Result<(), ConversationRepositoryError> {
    transaction
        .execute(
            "UPDATE revision_media_refs SET state = CASE WHEN EXISTS(SELECT 1 FROM conversation_messages message WHERE message.conversation_id = ?1 AND message.id = ?2 AND message.visibility <> 'tombstoned' AND message.active_revision_id = revision_media_refs.message_revision_id) THEN 'active' ELSE 'historical' END WHERE conversation_id = ?1 AND message_revision_id IN (SELECT id FROM conversation_message_revisions WHERE conversation_id = ?1 AND message_id = ?2)",
            params![conversation_id.to_string(), copy_id.to_string()],
        )
        .map_err(storage)?;
    Ok(())
}

fn copy_messages(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    fork: ConversationBranchId,
    mut parent: MessageId,
    chain: &[MessageId],
) -> Result<(), ConversationRepositoryError> {
    for original in chain {
        let copy_id = fork_copy_id(fork, *original);
        if !exists(
            transaction,
            "SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2)",
            params![conversation_id.to_string(), copy_id.to_string()],
        )? {
            let (source, revision) =
                flattened_revision(transaction, conversation_id, *original, copy_id)?;
            let next: i64 = transaction
                .query_row(
                    "SELECT next_timeline_ordinal FROM conversations WHERE id = ?1",
                    [conversation_id.to_string()],
                    |row| row.get(0),
                )
                .map_err(storage)?;
            let backup = BackupMessage {
                message: Message {
                    id: copy_id,
                    branch_id: fork,
                    parent_message_id: Some(parent),
                    active_render_source: MessageRenderSource::Revision(revision.id),
                    revision: Revision::INITIAL,
                    updated_at: source.message.created_at,
                    ..source.message.clone()
                },
                timeline_ordinal: u64::try_from(next).map_err(storage)?,
                initial_origin: None,
                revisions: vec![revision],
                candidates: Vec::new(),
                historical_media_revision_ids: Vec::new(),
                historical_media_candidate_ids: Vec::new(),
            };
            history::insert_message_with_turns(
                transaction,
                &backup,
                &[],
                &history::Evidence::usage_only(&[]),
            )?;
            settle_copy_media(transaction, conversation_id, copy_id)?;
            transaction
                .execute(
                    "UPDATE conversations SET next_timeline_ordinal = ?1 WHERE id = ?2",
                    params![next + 1, conversation_id.to_string()],
                )
                .map_err(storage)?;
        }
        transaction
            .execute(
                "UPDATE conversation_branches SET head_message_id = ?1 WHERE conversation_id = ?2 AND id = ?3 AND (head_message_id IS NULL OR head_message_id = ?4)",
                params![
                    copy_id.to_string(),
                    conversation_id.to_string(),
                    fork.to_string(),
                    parent.to_string(),
                ],
            )
            .map_err(storage)?;
        parent = copy_id;
    }
    Ok(())
}

/// Brings every fork copy of a merged original to what the original now
/// shows, with its flags; a copy's tombstone is never lifted.
fn refresh_copies(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    original: MessageId,
) -> Result<(), ConversationRepositoryError> {
    let branches = transaction
        .prepare("SELECT id FROM conversation_branches WHERE conversation_id = ?1 AND parent_branch_id IS NOT NULL")
        .and_then(|mut statement| {
            statement
                .query_map([conversation_id.to_string()], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(storage)?;
    for branch in branches {
        let branch = branch.parse::<ConversationBranchId>().map_err(storage)?;
        let copy_id = fork_copy_id(branch, original);
        let Some(copy) =
            crate::backup_adapter::read_conversation_message(transaction, conversation_id, copy_id)
                .map_err(storage)?
        else {
            continue;
        };
        let (source, revision) =
            flattened_revision(transaction, conversation_id, original, copy_id)?;
        if !copy.revisions.iter().any(|known| known.id == revision.id) {
            let last: i64 = transaction
                .query_row(
                    "SELECT COALESCE(max(sequence), 0) FROM conversation_message_revisions WHERE conversation_id = ?1 AND message_id = ?2",
                    params![conversation_id.to_string(), copy_id.to_string()],
                    |row| row.get(0),
                )
                .map_err(storage)?;
            history::insert_revision(
                transaction,
                &copy,
                &MessageRevision {
                    sequence: Revision::new(u64::try_from(last).map_err(storage)? + 1),
                    ..revision.clone()
                },
            )?;
        }
        let visibility = if copy.message.visibility == MessageVisibility::Tombstoned {
            MessageVisibility::Tombstoned
        } else {
            source.message.visibility
        };
        transaction
            .execute(
                "UPDATE conversation_messages SET active_revision_id = ?3, active_candidate_id = NULL, visibility = ?4, pinned = ?5, author_participant_id = ?6, revision = revision + 1 WHERE conversation_id = ?1 AND id = ?2",
                params![
                    conversation_id.to_string(),
                    copy_id.to_string(),
                    revision.id.to_string(),
                    crate::conversation_mutation_kernel::message_visibility_name(visibility),
                    i64::from(source.message.pinned),
                    source.message.author_participant_id.map(|id| id.to_string()),
                ],
            )
            .map_err(crate::conversation_mutation_kernel::map_constraint)?;
        settle_copy_media(transaction, conversation_id, copy_id)?;
    }
    Ok(())
}

/// Marks conversations whose messages were all journaled by this scan; those
/// with a message it had to skip (deferred or mid-generation) stay pending.
pub(crate) fn mark_messages_scanned(
    transaction: &Transaction<'_>,
    scanned: &[String],
    skipped: &[String],
) -> rusqlite::Result<()> {
    let pending = skipped
        .iter()
        .filter_map(|id| id.split_once(':').map(|(conversation, _)| conversation))
        .collect::<std::collections::HashSet<_>>();
    let mut done = scanned
        .iter()
        .filter_map(|id| id.split_once(':').map(|(conversation, _)| conversation))
        .filter(|conversation| !pending.contains(conversation))
        .collect::<Vec<_>>();
    done.dedup();
    for conversation in done {
        transaction.execute(
            "INSERT INTO sync_conversation_marks (conversation_id, changed, scanned) VALUES (?1, 0, 0)
             ON CONFLICT(conversation_id) DO UPDATE SET scanned = changed",
            [conversation],
        )?;
    }
    Ok(())
}
