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

fn replace_copy_placeholders(parts: &mut [MessagePart], kind: &SelectedConversationCopyKind) {
    let names = match kind {
        SelectedConversationCopyKind::GroupToCharacterFromMessage { placeholder_names }
        | SelectedConversationCopyKind::GroupToCharacter { placeholder_names } => placeholder_names,
        _ => return,
    };
    for part in parts {
        let MessagePart::Text { text } = part else {
            continue;
        };
        let mut output = String::new();
        let mut rest = text.as_str();
        while let Some(start) = rest.find("{{@\"") {
            output.push_str(&rest[..start]);
            let after = &rest[start + 4..];
            let Some(end) = after.find('"') else {
                output.push_str(&rest[start..]);
                rest = "";
                break;
            };
            let name = &after[..end];
            if !name.is_empty()
                && after[end..].starts_with("\"}}")
                && names.iter().any(|candidate| candidate == name)
            {
                output.push_str(name);
                rest = &after[end + 3..];
            } else {
                output.push_str("{{@\"");
                rest = after;
            }
        }
        output.push_str(rest);
        *text = output;
    }
}

fn copy_candidates_in(
    transaction: &rusqlite::Transaction<'_>,
    command: &ConversationContentCopy,
    source: &BackupMessage,
    target: &mut BackupMessage,
    participants: &[lettuce_conversations::ConversationParticipant],
) -> Result<(), ConversationRepositoryError> {
    use lettuce_conversations::{
        GenerationAttempt, GenerationAttemptStatus, GenerationInput, GenerationOperation,
        GenerationTarget, GenerationTurn, GenerationTurnStatus, IdempotencyKey, MessageRole,
    };
    use lettuce_types::{GenerationAttemptId, GenerationTurnId, MessageCandidateId, UsageEventId};
    let parent = target
        .message
        .parent_message_id
        .ok_or_else(|| invalid("copy.candidate_parent"))?;
    let parent_role: String = transaction
        .query_row(
            "SELECT role FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2",
            rusqlite::params![
                command.target_conversation_id.to_string(),
                parent.to_string()
            ],
            |row| row.get(0),
        )
        .map_err(slice::db)?;
    let author = participants
        .iter()
        .find(|participant| participant.role == lettuce_conversations::ParticipantRole::Character)
        .ok_or_else(|| invalid("copy.candidate_author"))?
        .id;
    let mut turns = Vec::new();
    let mut usage = Vec::new();
    let mut previous = None;
    let mut candidate_ids = HashMap::new();
    let mut candidates = source.candidates.clone();
    candidates.sort_by_key(|candidate| (candidate.ordinal, candidate.created_at, candidate.id));
    for mut candidate in candidates {
        let old_id = candidate.id;
        let old_turn = candidate.turn_id;
        let old_attempt = candidate.attempt_id;
        let candidate_id = MessageCandidateId::new();
        let turn_id = GenerationTurnId::new();
        let attempt_id = GenerationAttemptId::new();
        let at = candidate.created_at;
        let usage_id = UsageEventId::new();
        let old_usage =
            crate::usage_adapter::load_turn_usage_in(transaction, &old_turn.to_string())
                .map_err(|_| ConversationRepositoryError::Storage)?
                .into_iter()
                .find(|event| event.record.attempt_id == old_attempt)
                .ok_or_else(|| invalid("copy.candidate_usage"))?;
        let mut event = old_usage;
        event.id = usage_id;
        event.record.turn_id = turn_id;
        event.record.attempt_id = attempt_id;
        event.record.recorded_at = at;
        usage.push(event);
        let (operation, input, generation_target) = match previous {
            Some(prior) => (
                GenerationOperation::Regenerate,
                GenerationInput::ExistingCandidate {
                    message_id: target.message.id,
                    candidate_id: prior,
                },
                GenerationTarget::ExistingCandidate {
                    message_id: target.message.id,
                    prior_candidate_id: prior,
                },
            ),
            None if parent_role == "user" => (
                GenerationOperation::Send,
                GenerationInput::UserMessage { message_id: parent },
                GenerationTarget::NewAssistant {
                    message_id: target.message.id,
                    parent_message_id: Some(parent),
                },
            ),
            None => (
                GenerationOperation::Continue,
                GenerationInput::ExistingHead {
                    head_message_id: parent,
                },
                GenerationTarget::NewAssistant {
                    message_id: target.message.id,
                    parent_message_id: Some(parent),
                },
            ),
        };
        let turn = GenerationTurn {
            id: turn_id,
            conversation_id: command.target_conversation_id,
            branch_id: command.target_branch_id,
            operation,
            input,
            target: generation_target,
            swap_roles: false,
            retry_of_turn_id: None,
            idempotency_key: IdempotencyKey::new(format!("copy.turn.{turn_id}"))
                .map_err(|_| invalid("copy.turn_key"))?,
            correlation_id: None,
            status: GenerationTurnStatus::Succeeded,
            selected_speaker: None,
            guidance: None,
            requested_model_override: None,
            forced_speaker: None,
            resolved_model: candidate.model.clone(),
            prompt: None,
            lorebooks: Vec::new(),
            memory: None,
            candidate_ids: vec![candidate_id],
            selected_candidate_id: Some(candidate_id),
            attempts: vec![GenerationAttempt {
                id: attempt_id,
                turn_id,
                ordinal: 0,
                parent_attempt_id: None,
                status: GenerationAttemptStatus::Succeeded,
                job_idempotency_key: IdempotencyKey::new(format!(
                    "generation.{turn_id}.{attempt_id}"
                ))
                .map_err(|_| invalid("copy.attempt_key"))?,
                job_id: None,
                started_at: Some(at),
                finished_at: Some(at),
                candidate_ids: vec![candidate_id],
                usage_event_id: Some(usage_id),
                failure: None,
            }],
            failure: None,
            revision: Revision::INITIAL,
            created_at: at,
            updated_at: at,
        };
        candidate_ids.insert(old_id, candidate_id);
        if let Some(edit) = source
            .revisions
            .iter()
            .filter(|revision| revision.supersedes_candidate_id == Some(old_id))
            .max_by_key(|revision| revision.sequence)
        {
            candidate.parts = edit.parts.clone();
        }
        candidate.id = candidate_id;
        candidate.message_id = target.message.id;
        candidate.turn_id = turn_id;
        candidate.attempt_id = attempt_id;
        candidate.author_participant_id = author;
        candidate.provider_replay = None;
        candidate.parts.retain(|part| {
            !matches!(
                part,
                MessagePart::ToolCall { .. } | MessagePart::ToolResult { .. }
            )
        });
        replace_copy_placeholders(&mut candidate.parts, &command.kind);
        if source.message.active_render_source == MessageRenderSource::Candidate(old_id) {
            target.message.active_render_source = MessageRenderSource::Candidate(candidate_id);
            target.revisions.clear();
        }
        target.candidates.push(candidate);
        turns.push(turn);
        previous = Some(candidate_id);
    }
    if let MessageRenderSource::Revision(id) = source.message.active_render_source {
        let original = source
            .revisions
            .iter()
            .find(|revision| revision.id == id)
            .ok_or_else(|| invalid("copy.active_revision"))?;
        if let Some(candidate) = original.supersedes_candidate_id {
            target.revisions[0].supersedes_candidate_id = Some(
                *candidate_ids
                    .get(&candidate)
                    .ok_or_else(|| invalid("copy.revision_candidate"))?,
            );
        }
    }
    target.message.role = MessageRole::Assistant;
    let references = turns.iter().collect::<Vec<_>>();
    history::insert_message_with_turns(
        transaction,
        target,
        &references,
        &history::Evidence::usage_only(&usage),
    )?;
    transaction.execute("UPDATE candidate_media_refs SET state = CASE WHEN candidate_id = (SELECT active_candidate_id FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2) THEN 'active' ELSE 'historical' END WHERE conversation_id = ?1 AND candidate_id IN (SELECT id FROM conversation_message_candidates WHERE conversation_id = ?1 AND message_id = ?2)",rusqlite::params![target.message.conversation_id.to_string(),target.message.id.to_string()]).map_err(slice::db)?;
    Ok(())
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
        if target_branch.parent_branch_id.is_some()
            || (target_branch.head_message_id.is_some()
                && !matches!(command.kind, SelectedConversationCopyKind::DirectToGroup))
        {
            return Err(invalid("copy.target_root"));
        }
        match &command.kind {
            SelectedConversationCopyKind::Duplicate => {
                if std::mem::discriminant(&source.conversation.kind)
                    != std::mem::discriminant(&target.conversation.kind)
                {
                    return Err(invalid("copy.target_kind"));
                }
            }
            SelectedConversationCopyKind::GroupToCharacterFromMessage { .. }
            | SelectedConversationCopyKind::GroupToCharacter { .. } => {
                if !matches!(
                    source.conversation.kind,
                    lettuce_conversations::ConversationKind::Group(_)
                ) || !matches!(
                    target.conversation.kind,
                    lettuce_conversations::ConversationKind::Direct(_)
                ) {
                    return Err(invalid("copy.target_kind"));
                }
            }
            SelectedConversationCopyKind::DirectToCharacter => {
                if !matches!(
                    source.conversation.kind,
                    lettuce_conversations::ConversationKind::Direct(_)
                ) || !matches!(
                    target.conversation.kind,
                    lettuce_conversations::ConversationKind::Direct(_)
                ) {
                    return Err(invalid("copy.target_kind"));
                }
            }
            SelectedConversationCopyKind::DirectToGroup => {
                if !matches!(
                    source.conversation.kind,
                    lettuce_conversations::ConversationKind::Direct(_)
                ) || !matches!(
                    target.conversation.kind,
                    lettuce_conversations::ConversationKind::Group(_)
                ) {
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
        let seed_message_id = if matches!(
            command.kind,
            SelectedConversationCopyKind::GroupToCharacter { .. }
        ) {
            None
        } else {
            items.last().map(|item| item.message.id)
        };
        if matches!(
            command.kind,
            SelectedConversationCopyKind::DirectToCharacter
        ) {
            items.retain(|item| item.message.role != lettuce_conversations::MessageRole::Scene);
        }
        if matches!(command.kind, SelectedConversationCopyKind::DirectToGroup) {
            items.retain(|item| {
                matches!(
                    item.message.role,
                    lettuce_conversations::MessageRole::User
                        | lettuce_conversations::MessageRole::Assistant
                )
            });
        }
        self.transaction.execute("INSERT INTO conversation_snapshot_refs (conversation_id,artifact_id) SELECT ?1,artifact_id FROM conversation_snapshot_refs WHERE conversation_id = ?2 ON CONFLICT DO NOTHING",rusqlite::params![command.target_conversation_id.to_string(),command.source_conversation_id.to_string()]).map_err(slice::db)?;

        let mut message_ids = HashMap::new();
        let mut result = Vec::with_capacity(items.len());
        let mut parent = target_branch.head_message_id;
        for item in items {
            let id = MessageId::new();
            let revision_id = MessageRevisionId::new();
            let (mut parts, authored_at) = mutations::shown_parts(&item);
            match &command.kind {
                SelectedConversationCopyKind::Duplicate => parts.retain(|part| {
                    matches!(
                        part,
                        MessagePart::Text { .. } | MessagePart::MediaAsset { .. }
                    )
                }),
                SelectedConversationCopyKind::GroupToCharacter { .. } => {
                    parts.retain(|part| matches!(part, MessagePart::Text { .. }))
                }
                _ => parts.retain(|part| {
                    matches!(
                        part,
                        MessagePart::Text { .. }
                            | MessagePart::MediaAsset { .. }
                            | MessagePart::ReasoningSummary { .. }
                            | MessagePart::Annotation { .. }
                    )
                }),
            }
            replace_copy_placeholders(&mut parts, &command.kind);
            let author = if item.message.role == lettuce_conversations::MessageRole::Assistant
                && !matches!(command.kind, SelectedConversationCopyKind::Duplicate)
            {
                let owner = match &target.conversation.kind {
                    lettuce_conversations::ConversationKind::Direct(details) => {
                        details.character.source_id
                    }
                    lettuce_conversations::ConversationKind::Group(_) => {
                        match &source.conversation.kind {
                            lettuce_conversations::ConversationKind::Direct(details) => {
                                details.character.source_id
                            }
                            _ => return Err(invalid("copy.assistant_owner")),
                        }
                    }
                };
                Some(
                    target
                        .conversation
                        .participants
                        .iter()
                        .find(|participant| {
                            participant.source
                                == lettuce_conversations::ParticipantSource::Character(owner)
                        })
                        .ok_or_else(|| invalid("copy.assistant_owner"))?
                        .id,
                )
            } else {
                match item.message.author_participant_id {
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
                                participant.role == source_participant.role
                                    && (participant.source == source_participant.source
                                        || (participant.role
                                            == lettuce_conversations::ParticipantRole::Character
                                            && matches!(
                                                target.conversation.kind,
                                                lettuce_conversations::ConversationKind::Direct(_)
                                            )))
                            })
                            .ok_or_else(|| invalid("copy.target_participant"))?;
                        Some(target_participant.id)
                    }
                }
            };
            let source_backup = if parent.is_some()
                && matches!(
                    command.kind,
                    SelectedConversationCopyKind::DirectToCharacter
                        | SelectedConversationCopyKind::GroupToCharacterFromMessage { .. }
                ) {
                crate::backup::backup_adapter::read_conversation_message(
                    self.transaction,
                    command.source_conversation_id,
                    item.message.id,
                )
                .map_err(|_| ConversationRepositoryError::Storage)?
            } else {
                None
            };
            let copied_scene_source =
                if item.message.role == lettuce_conversations::MessageRole::Scene {
                    match &item.initial_origin {
                        Some(lettuce_conversations::InitialMessageOrigin::SelectedScene {
                            snapshot_ref,
                        }) => Some(snapshot_ref.clone()),
                        _ => query::hydrate_copied_scene_source(
                            self.transaction,
                            command.source_conversation_id,
                            item.message.id,
                        )?,
                    }
                } else {
                    None
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
            let mut backup = BackupMessage {
                message,
                timeline_ordinal: u64::try_from(ordinal)
                    .map_err(|_| invalid("copy.timeline_ordinal"))?,
                initial_origin: None,
                copied_scene_source,
                revisions: vec![revision],
                candidates: Vec::new(),
                historical_media_revision_ids: Vec::new(),
                historical_media_candidate_ids: Vec::new(),
            };
            if let Some(source_backup) =
                source_backup.filter(|source| !source.candidates.is_empty())
            {
                copy_candidates_in(
                    self.transaction,
                    command,
                    &source_backup,
                    &mut backup,
                    &target.conversation.participants,
                )?;
            } else {
                history::insert_message(self.transaction, &backup)?;
                history::insert_revision(self.transaction, &backup, &backup.revisions[0])?;
                history::insert_copied_scene_source(self.transaction, &backup)?;
            }
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
        if !result.is_empty()
            || matches!(
                command.kind,
                SelectedConversationCopyKind::GroupToCharacter { .. }
            )
        {
            crate::memory::memory_branch_adapter::seed_new_conversation_space_in(
                self.transaction,
                command.source_conversation_id,
                command.source_branch_id,
                (command.target_conversation_id, command.target_branch_id),
                seed_message_id,
                &message_ids,
            )?;
        }
        if let Some(head) = result.last().copied() {
            let at = self.transaction.query_row("SELECT updated_at FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2", rusqlite::params![command.target_conversation_id.to_string(), head.to_string()], |row| row.get::<_, i64>(0)).map_err(slice::db)?;

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

#[derive(Debug)]
pub struct ConversationCopyLaunch {
    pub launch: lettuce_conversations::PreparedConversationLaunch,
    pub source_conversation_id: lettuce_types::ConversationId,
    pub source_revision: Revision,
    pub source_branch_id: lettuce_types::ConversationBranchId,
    pub source_group_revision: Option<(lettuce_types::GroupId, Revision)>,
    pub source_character_revision: Option<(lettuce_types::CharacterId, Revision)>,
    pub through_message_id: Option<MessageId>,
    pub kind: SelectedConversationCopyKind,
    pub companion: Option<(
        lettuce_companions::CompanionStateOwner,
        lettuce_companions::CompanionRuntimeState,
        bool,
    )>,
    pub initialize_companion: bool,
    pub new_group: Option<lettuce_characters::CreateGroupPlan>,
}

impl crate::ApiOperationTransaction<'_, '_> {
    pub fn create_conversation_copy(
        &self,
        draft: ConversationCopyLaunch,
        now: lettuce_types::TimestampMillis,
    ) -> Result<lettuce_conversations::CreateConversationResult, ConversationRepositoryError> {
        if let Some((owner, _, _)) = &draft.companion
            && (owner.conversation_id != draft.launch.plan().conversation_id
                || !matches!(&draft.launch.plan().kind,lettuce_conversations::ConversationKind::Direct(details) if details.character.source_id == owner.character_id))
        {
            return Err(invalid("copy.companion_owner"));
        }
        if let Some(group) = &draft.new_group
            && !matches!(&draft.launch.plan().kind,lettuce_conversations::ConversationKind::Group(details) if details.group.source_id == group.group.id)
        {
            return Err(invalid("copy.group_source"));
        }
        let source =
            slice::hydrate_conversation(self.transaction, draft.source_conversation_id, || {})?;
        if source.conversation.revision != draft.source_revision
            || source.conversation.active_branch_id != draft.source_branch_id
        {
            return Err(ConversationRepositoryError::Conflict);
        }
        if let Some((id, revision)) = draft.source_group_revision {
            let current = crate::catalog::group_adapter::load_details(self.transaction, id)
                .map_err(slice::db)?
                .ok_or(ConversationRepositoryError::NotFound)?;
            if current.group.revision != revision {
                return Err(ConversationRepositoryError::Conflict);
            }
        }
        if let Some((id, revision)) = draft.source_character_revision {
            let current =
                crate::catalog::character_adapter::load_character_details(self.transaction, id)
                    .map_err(|_| ConversationRepositoryError::Storage)?
                    .ok_or(ConversationRepositoryError::NotFound)?;
            if current.character.revision != revision {
                return Err(ConversationRepositoryError::Conflict);
            }
        }
        if let Some(group) = &draft.new_group {
            group.validate().map_err(|_| invalid("copy.group"))?;
            super::super::catalog::group_adapter::insert_group_plan(self.transaction, group)
                .map_err(|error| match error {
                    lettuce_characters::RepositoryError::NotFound => {
                        ConversationRepositoryError::NotFound
                    }
                    lettuce_characters::RepositoryError::AlreadyExists
                    | lettuce_characters::RepositoryError::StaleRevision { .. }
                    | lettuce_characters::RepositoryError::AlreadyActive => {
                        ConversationRepositoryError::Conflict
                    }
                    lettuce_characters::RepositoryError::Storage => {
                        ConversationRepositoryError::Storage
                    }
                    lettuce_characters::RepositoryError::Archived
                    | lettuce_characters::RepositoryError::MissingDefaultRevision
                    | lettuce_characters::RepositoryError::HasDependencies
                    | lettuce_characters::RepositoryError::Invalid(_) => invalid("copy.group"),
                })?;
        }
        let memory = draft.companion.as_ref().map_or(
            super::conversation_creator::MemoryBinding::PerConversation,
            |(owner, _, _)| {
                super::conversation_creator::MemoryBinding::CompanionPool(owner.character_id)
            },
        );
        let companion = draft.companion;
        let initialize = draft.initialize_companion;
        let mut commit = super::conversation_creator::create_on_transaction(
            self.transaction,
            draft.launch,
            now,
            memory,
            |transaction, _| {
                if initialize && let Some((owner, state, time_awareness)) = &companion {
                    super::state_adapter::create_in(transaction, *owner, state, now)
                        .map_err(super::state_adapter::conversation_state_error)?;
                    super::state_adapter::ensure_continuity_episode_in(transaction, *owner, now)
                        .map_err(super::state_adapter::conversation_state_error)?;
                    crate::catalog::character_adapter::ensure_companion_soul_in(
                        transaction,
                        owner.character_id,
                        now,
                    )
                    .map_err(|_| ConversationRepositoryError::Storage)?;
                    if !crate::catalog::character_adapter::companion_soul_shared_in(
                        transaction,
                        owner.character_id,
                    )
                    .map_err(|_| ConversationRepositoryError::Storage)?
                    {
                        crate::companion::soul_adapter::seed_conversation_soul_in(
                            transaction,
                            owner.character_id,
                            owner.conversation_id,
                            now,
                        )
                        .map_err(|_| ConversationRepositoryError::Storage)?;
                    }
                    if *time_awareness {
                        let mut settings =
                            lettuce_conversations::CurrentConversationSettings::inherited(
                                Revision::INITIAL,
                            );
                        settings.companion_clock =
                            Some(lettuce_conversations::CompanionClockSettings {
                                time_awareness_enabled: true,
                                ..Default::default()
                            });
                        let stored =
                            slice::hydrate_conversation(transaction, owner.conversation_id, || {})?
                                .conversation
                                .current_settings;
                        if let Some(current) = stored {
                            settings = current;
                            settings.companion_clock =
                                Some(lettuce_conversations::CompanionClockSettings {
                                    time_awareness_enabled: true,
                                    ..Default::default()
                                });
                        }
                        mutations::write_settings(
                            transaction,
                            owner.conversation_id,
                            &settings,
                            true,
                            now,
                        )?;
                    }
                }
                Ok(())
            },
        )?;
        let target = commit.value.conversation.id;
        let root = commit.value.conversation.active_branch_id;
        self.copy_selected_conversation_content(&ConversationContentCopy {
            source_conversation_id: draft.source_conversation_id,
            source_branch_id: draft.source_branch_id,
            target_conversation_id: target,
            target_branch_id: root,
            through_message_id: draft.through_message_id,
            kind: draft.kind.clone(),
        })?;
        if matches!(draft.kind, SelectedConversationCopyKind::DirectToCharacter) {
            let message = draft
                .through_message_id
                .ok_or_else(|| invalid("copy.lineage_message"))?;
            self.transaction.execute("UPDATE conversations SET origin_conversation_id = ?2,origin_message_id = ?3 WHERE id = ?1",rusqlite::params![target.to_string(),draft.source_conversation_id.to_string(),message.to_string()]).map_err(slice::db)?;
        }
        commit.value.conversation =
            slice::hydrate_conversation(self.transaction, target, || {})?.conversation;
        Ok(commit)
    }
}

impl crate::Database {
    pub fn copy_snapshot_draft(
        &self,
        reference: &lettuce_conversations::ProtectedSnapshotRef,
    ) -> Result<lettuce_conversations::SnapshotArtifactDraft, lettuce_conversations::ArtifactError>
    {
        let mut connection = self
            .connection()
            .map_err(|_| lettuce_conversations::ArtifactError::Storage)?;
        let transaction = connection
            .transaction()
            .map_err(|_| lettuce_conversations::ArtifactError::Storage)?;
        super::conversation_artifact_adapter::snapshot_draft_in(&transaction, reference)
    }
}
