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

impl crate::ApiOperationTransaction<'_, '_> {
    pub fn duplicate_conversation(
        &self,
        command: &lettuce_conversations::DuplicateConversation,
        now: lettuce_types::TimestampMillis,
    ) -> Result<lettuce_conversations::CreateConversationResult, ConversationRepositoryError> {
        use lettuce_conversations::{
            ConversationKind, ConversationParticipantDraft, CreateConversationPlan,
            InitialTimelineDraft, ParticipantRole, ParticipantSource, PreparedConversationLaunch,
            SettingProvenance,
        };
        use lettuce_types::ConversationParticipantId;
        use rusqlite::OptionalExtension;

        if command.source_conversation_id == command.conversation_id {
            return Err(invalid("duplicate.target_conversation"));
        }
        let source =
            slice::hydrate_conversation(self.transaction, command.source_conversation_id, || {})?;
        let participant_ids: HashMap<_, _> = source
            .conversation
            .participants
            .iter()
            .map(|participant| (participant.id, ConversationParticipantId::new()))
            .collect();
        let mut kind = source.conversation.kind.clone();
        if let ConversationKind::Group(details) = &mut kind {
            for member in &mut details.initial_participant_policy.members {
                member.participant_id = *participant_ids
                    .get(&member.participant_id)
                    .ok_or_else(|| invalid("duplicate.participant_policy"))?;
            }
            details.initial_participant_policy.revision = Revision::INITIAL;
            details.initial_participant_policy.created_at = now;
            details.initial_participant_policy.updated_at = now;
        }
        let mut current_settings = source.conversation.current_settings.clone();
        if let Some(settings) = &mut current_settings {
            settings.revision = Revision::INITIAL;
            settings.author_note = None;
            settings.author_note_provenance = SettingProvenance::LaunchInherited;
        }
        let mut participants = Vec::new();
        for participant in source
            .conversation
            .participants
            .iter()
            .filter(|participant| participant.member_snapshot.is_none())
        {
            let mut draft = ConversationParticipantDraft {
                id: participant_ids[&participant.id],
                role: participant.role,
                ordinal: participant.ordinal,
                source: participant.source,
                enabled: participant.enabled,
                muted: participant.muted,
                display_name: participant.display_name.clone(),
                authored_description: participant.authored_description.clone(),
                model_selection: participant.model_selection.clone(),
            };
            match &kind {
                ConversationKind::Direct(details) if draft.role == ParticipantRole::Character => {
                    draft.model_selection = details.model.clone()
                }
                ConversationKind::Group(details) if draft.role == ParticipantRole::Character => {
                    let member = details
                        .group
                        .members
                        .iter()
                        .find(|member| {
                            draft.source == ParticipantSource::Character(member.character.source_id)
                        })
                        .ok_or_else(|| invalid("duplicate.group_member"))?;
                    draft.enabled = member.enabled;
                    draft.muted = member.muted;
                    draft.model_selection = member.model_override.clone();
                }
                _ => {}
            }
            participants.push(draft);
        }
        let plan = CreateConversationPlan {
            conversation_id: command.conversation_id,
            title: command
                .title
                .clone()
                .unwrap_or_else(|| format!("{} (copy)", source.conversation.title)),
            kind,
            participants,
            initial_timeline: InitialTimelineDraft {
                format_version: 1,
                entries: Vec::new(),
            },
            operation: command.operation.clone(),
            current_settings,
        };
        let references = lettuce_conversations::conversation_launch_snapshot_references(&plan);
        let mut unique = std::collections::BTreeMap::new();
        for reference in references {
            if let Some(previous) = unique.insert(reference.artifact_id, reference.clone())
                && previous != *reference
            {
                return Err(invalid("duplicate.snapshot_references"));
            }
        }
        let drafts = unique
            .values()
            .map(|reference| {
                super::conversation_artifact_adapter::snapshot_draft_in(self.transaction, reference)
                    .map_err(ConversationRepositoryError::ArtifactReference)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let launch = PreparedConversationLaunch::new(plan, drafts)
            .map_err(|_| invalid("duplicate.launch"))?;
        let pooled_character: Option<String> = self.transaction.query_row(
            "SELECT pool.character_id FROM conversation_memory_spaces binding JOIN companion_memory_pools pool ON pool.space_id = binding.space_id WHERE binding.conversation_id = ?1 AND binding.pooled = 1",
            [command.source_conversation_id.to_string()], |row| row.get(0),
        ).optional().map_err(slice::db)?;
        if pooled_character.is_some() {
            return Err(ConversationRepositoryError::Unsupported);
        }
        let mut commit = super::conversation_creator::create_on_transaction(
            self.transaction,
            launch,
            now,
            super::conversation_creator::MemoryBinding::PerConversation,
            |_, _| Ok(()),
        )?;
        let mut target = commit.value.conversation.clone();
        target.participants = source
            .conversation
            .participants
            .iter()
            .map(|participant| {
                let mut copied = participant.clone();
                copied.id = participant_ids[&participant.id];
                copied.revision = Revision::INITIAL;
                copied.created_at = now;
                copied.updated_at = now;
                copied
            })
            .collect();
        target
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        slice::save_participants(self.transaction, &target)?;
        self.transaction.execute(
            "INSERT INTO conversation_snapshot_refs (conversation_id,artifact_id) SELECT ?1,artifact_id FROM conversation_snapshot_refs WHERE conversation_id = ?2 ON CONFLICT DO NOTHING",
            rusqlite::params![command.conversation_id.to_string(), command.source_conversation_id.to_string()],
        ).map_err(slice::db)?;
        if command.with_messages {
            self.copy_selected_conversation_content(&ConversationContentCopy {
                source_conversation_id: command.source_conversation_id,
                source_branch_id: source.conversation.active_branch_id,
                target_conversation_id: command.conversation_id,
                target_branch_id: target.active_branch_id,
                through_message_id: None,
                kind: SelectedConversationCopyKind::Duplicate,
            })?;
        }
        commit.value =
            slice::hydrate_conversation(self.transaction, command.conversation_id, || {})?;
        Ok(commit)
    }
}
