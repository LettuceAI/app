use std::collections::HashMap;

use lettuce_conversations::{
    ConversationContentCopy, ConversationRepositoryError, MessagePart, MessageRenderSource,
    MessageRevision, SelectedConversationCopyKind, ValidationError,
};
use lettuce_transfer::BackupMessage;
use lettuce_types::{MessageId, MessageRevisionId, Revision};

use super::{
    conversation_history_writer as history, conversation_mutations as mutations,
    conversation_query as query, conversation_vertical_slice as slice,
};

fn invalid(field: &'static str) -> ConversationRepositoryError {
    ConversationRepositoryError::Invalid(ValidationError::InvalidReference { field })
}

impl crate::ApiOperationTransaction<'_, '_> {
    pub fn copy_selected_conversation_content(
        &self,
        command: &ConversationContentCopy,
    ) -> Result<Vec<MessageId>, ConversationRepositoryError> {
        if command.source_conversation_id == command.target_conversation_id {
            return Err(invalid("copy.target_conversation"));
        }
        let source =
            slice::hydrate_conversation(self.transaction, command.source_conversation_id, || {})?;
        let target =
            slice::hydrate_conversation(self.transaction, command.target_conversation_id, || {})?;
        let source_branch = source
            .branches
            .iter()
            .find(|branch| branch.id == command.source_branch_id)
            .ok_or(ConversationRepositoryError::NotFound)?;
        let target_branch = target
            .branches
            .iter()
            .find(|branch| branch.id == command.target_branch_id)
            .ok_or(ConversationRepositoryError::NotFound)?;
        if target_branch.parent_branch_id.is_some() || target_branch.head_message_id.is_some() {
            return Err(invalid("copy.target_root"));
        }
        match command.kind {
            SelectedConversationCopyKind::Duplicate => {
                if std::mem::discriminant(&source.conversation.kind)
                    != std::mem::discriminant(&target.conversation.kind)
                {
                    return Err(invalid("copy.target_kind"));
                }
            }
        }
        let mut items = query::timeline_items_after(
            self.transaction,
            command.source_conversation_id,
            source_branch.id,
            -1,
        )?;
        if let Some(message_id) = command.through_message_id {
            let index = items
                .iter()
                .position(|item| item.message.id == message_id)
                .ok_or_else(|| invalid("copy.source_message"))?;
            items.truncate(index + 1);
        }
        let seed_message_id = items.last().map(|item| item.message.id);
        let mut message_ids = HashMap::new();
        let mut result = Vec::with_capacity(items.len());
        let mut parent = target_branch.head_message_id;
        for item in items {
            let id = MessageId::new();
            let revision_id = MessageRevisionId::new();
            let (mut parts, authored_at) = mutations::shown_parts(&item);
            parts.retain(|part| {
                matches!(
                    part,
                    MessagePart::Text { .. } | MessagePart::MediaAsset { .. }
                )
            });
            let author = match item.message.author_participant_id {
                None => None,
                Some(source_id) => {
                    let source_participant = source
                        .conversation
                        .participants
                        .iter()
                        .find(|participant| participant.id == source_id)
                        .ok_or_else(|| invalid("copy.source_participant"))?;
                    let target_participant = target
                        .conversation
                        .participants
                        .iter()
                        .find(|participant| {
                            participant.source == source_participant.source
                                && participant.role == source_participant.role
                        })
                        .ok_or_else(|| invalid("copy.target_participant"))?;
                    Some(target_participant.id)
                }
            };
            let mut message = item.message;
            let old_id = message.id;
            message.id = id;
            message.conversation_id = command.target_conversation_id;
            message.branch_id = command.target_branch_id;
            message.parent_message_id = parent;
            message.author_participant_id = author;
            message.active_render_source = MessageRenderSource::Revision(revision_id);
            message.revision = Revision::INITIAL;
            let revision = MessageRevision {
                id: revision_id,
                message_id: id,
                sequence: Revision::INITIAL,
                parts,
                authored_at,
                source_turn_id: None,
                provider_replay: None,
                supersedes_candidate_id: None,
            };
            message
                .validate()
                .map_err(ConversationRepositoryError::Invalid)?;
            revision
                .validate()
                .map_err(ConversationRepositoryError::Invalid)?;
            let ordinal = mutations::allocate_timeline_ordinal(
                self.transaction,
                command.target_conversation_id,
            )?;
            let backup = BackupMessage {
                message,
                timeline_ordinal: u64::try_from(ordinal)
                    .map_err(|_| invalid("copy.timeline_ordinal"))?,
                initial_origin: None,
                revisions: vec![revision],
                candidates: Vec::new(),
                historical_media_revision_ids: Vec::new(),
                historical_media_candidate_ids: Vec::new(),
            };
            history::insert_message(self.transaction, &backup)?;
            history::insert_revision(self.transaction, &backup, &backup.revisions[0])?;
            message_ids.insert(old_id, id);
            result.push(id);
            parent = Some(id);
        }
        history::set_branch_head(
            self.transaction,
            command.target_conversation_id,
            command.target_branch_id,
            parent,
        )?;
        if let Some(head) = result.last().copied() {
            let at = self.transaction.query_row("SELECT updated_at FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2", rusqlite::params![command.target_conversation_id.to_string(), head.to_string()], |row| row.get::<_, i64>(0)).map_err(slice::db)?;
            crate::memory::memory_branch_adapter::seed_new_conversation_space_in(
                self.transaction,
                command.source_conversation_id,
                command.source_branch_id,
                (command.target_conversation_id, command.target_branch_id),
                seed_message_id,
                &message_ids,
            )?;
            self.transaction
                .execute(
                    "UPDATE conversations SET updated_at = max(updated_at, ?2) WHERE id = ?1",
                    rusqlite::params![command.target_conversation_id.to_string(), at],
                )
                .map_err(slice::db)?;
        }
        Ok(result)
    }
}
