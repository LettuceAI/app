use lettuce_types::{ConversationId, TimestampMillis};

use crate::commands::ConversationMutation;
use crate::error::{ConversationRepositoryError, ConversationServiceError};
use crate::generation::GenerationTurn;
use crate::model::ConversationAggregate;
use crate::ports::{
    ArchiveConversationResult, BeginGeneration, ConversationCreator, ConversationOutboxRecord,
    ConversationQuery, ConversationReader, ConversationRepository, ConversationSummary,
    CreateConversationResult, EditMessageResult, ForkBranchResult, GenerationFailureResult,
    GenerationFinalizationResult, GenerationInterruptionResult, GenerationRecoveryResult,
    KeysetPage, MutationCommit, OperationKind, ParticipantPolicyResult, RenameConversationResult,
    RestoreConversationResult, SelectBranchResult, SettingsResult, TimelinePage,
    TombstoneMessageResult, UpdateMessageFlagsResult,
};
use crate::{
    ProposedToolCall, ToolChoice, ToolExecution, ToolExecutionOwner, ToolExecutionRepository,
    ToolExecutionStatus, ToolExecutionTransition, ToolFailure, ToolOutput, ToolRequest,
    ValidationError,
};

/// Thin application-facing façade.  It validates command contracts and
/// delegates atomic mutations to the repository; provider execution belongs
/// to an application workflow that supplies the async ports.
#[derive(Debug)]
pub struct ConversationManager<R> {
    repository: R,
}

impl<R> ConversationManager<R> {
    #[must_use]
    pub const fn new(repository: R) -> Self {
        Self { repository }
    }

    #[must_use]
    pub const fn repository(&self) -> &R {
        &self.repository
    }
}

impl<R: ConversationReader> ConversationManager<R> {
    pub fn get(
        &self,
        id: ConversationId,
    ) -> Result<ConversationAggregate, ConversationServiceError> {
        self.repository.get(id).map_err(Into::into)
    }

    pub fn page(
        &self,
        query: &ConversationQuery,
    ) -> Result<KeysetPage<ConversationSummary>, ConversationServiceError> {
        self.repository.page(query).map_err(Into::into)
    }

    pub fn timeline_page(
        &self,
        conversation_id: lettuce_types::ConversationId,
        branch_id: lettuce_types::ConversationBranchId,
        page: &lettuce_types::PageRequest,
    ) -> Result<TimelinePage, ConversationServiceError> {
        let page = self
            .repository
            .timeline_page(conversation_id, branch_id, page)
            .map_err(ConversationServiceError::from)?;
        if page.conversation_id != conversation_id || page.selected_branch_id != branch_id {
            return Err(ConversationServiceError::Invalid(
                crate::ValidationError::InvalidReference {
                    field: "timeline_page.request_provenance",
                },
            ));
        }
        page.validate_page()?;
        Ok(page)
    }

    pub fn get_message_revision(
        &self,
        id: lettuce_types::MessageRevisionId,
    ) -> Result<crate::content::MessageRevision, ConversationServiceError> {
        self.repository.get_message_revision(id).map_err(Into::into)
    }

    pub fn page_message_revisions(
        &self,
        message_id: lettuce_types::MessageId,
        page: &lettuce_types::PageRequest,
    ) -> Result<KeysetPage<crate::content::MessageRevision>, ConversationServiceError> {
        self.repository
            .page_message_revisions(message_id, page)
            .map_err(Into::into)
    }

    pub fn get_candidate(
        &self,
        id: lettuce_types::MessageCandidateId,
    ) -> Result<crate::content::MessageCandidate, ConversationServiceError> {
        self.repository.get_candidate(id).map_err(Into::into)
    }

    pub fn page_candidates(
        &self,
        message_id: lettuce_types::MessageId,
        page: &lettuce_types::PageRequest,
    ) -> Result<KeysetPage<crate::content::MessageCandidate>, ConversationServiceError> {
        self.repository
            .page_candidates(message_id, page)
            .map_err(Into::into)
    }

    pub fn get_turn(
        &self,
        id: lettuce_types::GenerationTurnId,
    ) -> Result<GenerationTurn, ConversationServiceError> {
        self.repository.get_turn(id).map_err(Into::into)
    }

    pub fn page_turns(
        &self,
        conversation_id: lettuce_types::ConversationId,
        page: &lettuce_types::PageRequest,
    ) -> Result<KeysetPage<GenerationTurn>, ConversationServiceError> {
        self.repository
            .page_turns(conversation_id, page)
            .map_err(Into::into)
    }

    pub fn operation_record(
        &self,
        conversation_id: lettuce_types::ConversationId,
        kind: OperationKind,
        token: &crate::commands::OperationToken,
    ) -> Result<Option<crate::ports::OperationRecord>, ConversationServiceError> {
        self.repository
            .operation_record(conversation_id, kind, token)
            .map_err(Into::into)
    }

    pub fn page_outbox(
        &self,
        conversation_id: lettuce_types::ConversationId,
        page: &lettuce_types::PageRequest,
    ) -> Result<KeysetPage<ConversationOutboxRecord>, ConversationServiceError> {
        self.repository
            .page_outbox(conversation_id, page)
            .map_err(Into::into)
    }
}

impl<R: ConversationCreator> ConversationManager<R> {
    pub fn create(
        &self,
        launch: crate::PreparedConversationLaunch,
        now: TimestampMillis,
    ) -> Result<CreateConversationResult, ConversationServiceError> {
        self.repository.create(launch, now).map_err(Into::into)
    }
}

impl<R: ToolExecutionRepository> ConversationManager<R> {
    pub fn request_tool_executions(
        &self,
        owner: ToolExecutionOwner,
        request: &ToolRequest,
        calls: Vec<ProposedToolCall>,
        now: TimestampMillis,
    ) -> Result<Vec<ToolExecution>, ConversationServiceError> {
        let mut executions = plan_tool_executions(owner, request, calls, now)?;
        if executions.is_empty() {
            return Ok(Vec::new());
        }
        let existing = self.repository.list_tool_executions(
            owner.conversation_id,
            owner.turn_id,
            owner.attempt_id,
        )?;
        let next_ordinal =
            u16::try_from(existing.len()).map_err(|_| ValidationError::OutOfBounds {
                field: "tool_execution.ordinal",
            })?;
        for (offset, execution) in executions.iter_mut().enumerate() {
            execution.ordinal = next_ordinal
                .checked_add(
                    u16::try_from(offset).map_err(|_| ValidationError::OutOfBounds {
                        field: "tool_execution.ordinal",
                    })?,
                )
                .ok_or(ValidationError::OutOfBounds {
                    field: "tool_execution.ordinal",
                })?;
        }
        self.repository
            .append_tool_executions(next_ordinal, &executions)
            .map_err(Into::into)
    }

    pub fn tool_executions(
        &self,
        conversation_id: ConversationId,
        turn_id: lettuce_types::GenerationTurnId,
        attempt_id: lettuce_types::GenerationAttemptId,
    ) -> Result<Vec<ToolExecution>, ConversationServiceError> {
        self.repository
            .list_tool_executions(conversation_id, turn_id, attempt_id)
            .map_err(Into::into)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn transition_tool_execution(
        &self,
        id: lettuce_types::ToolExecutionId,
        expected_revision: lettuce_types::Revision,
        next: ToolExecutionStatus,
        output: Option<ToolOutput>,
        failure: Option<ToolFailure>,
        now: TimestampMillis,
    ) -> Result<ToolExecution, ConversationServiceError> {
        self.repository
            .transition_tool_execution(id, expected_revision, next, output, failure, now)
            .map_err(Into::into)
    }

    pub fn transition_tool_execution_batch(
        &self,
        transitions: &[ToolExecutionTransition],
        now: TimestampMillis,
    ) -> Result<Vec<ToolExecution>, ConversationServiceError> {
        self.repository
            .transition_tool_execution_batch(transitions, now)
            .map_err(Into::into)
    }
}

fn plan_tool_executions(
    owner: ToolExecutionOwner,
    request: &ToolRequest,
    calls: Vec<ProposedToolCall>,
    now: TimestampMillis,
) -> Result<Vec<ToolExecution>, ConversationServiceError> {
    request.validate()?;
    if calls.len() > crate::MAX_TOOL_CALLS_PER_RESPONSE {
        return Err(ValidationError::TooMany {
            field: "tool_executions",
            max: crate::MAX_TOOL_CALLS_PER_RESPONSE,
        }
        .into());
    }
    if calls.is_empty() && !matches!(request.choice, ToolChoice::Auto) {
        return Err(ValidationError::InvalidValue {
            field: "tool_executions.required",
        }
        .into());
    }
    let mut provider_call_ids = std::collections::HashSet::new();
    let mut executions = Vec::with_capacity(calls.len());
    for (ordinal, call) in calls.into_iter().enumerate() {
        call.validate()?;
        let definition = request
            .definitions
            .iter()
            .find(|definition| definition.name == call.name)
            .ok_or(ValidationError::InvalidReference {
                field: "tool_execution.definition",
            })?;
        if matches!(&request.choice, ToolChoice::Named { name } if *name != call.name) {
            return Err(ValidationError::InvalidReference {
                field: "tool_execution.choice",
            }
            .into());
        }
        if let Some(provider_call_id) = call.provider_call_id.as_deref()
            && !provider_call_ids.insert(provider_call_id.to_owned())
        {
            return Err(ValidationError::Duplicate {
                field: "tool_execution.provider_call_id",
            }
            .into());
        }
        let ordinal = u16::try_from(ordinal).map_err(|_| ValidationError::OutOfBounds {
            field: "tool_execution.ordinal",
        })?;
        executions.push(ToolExecution::requested(
            lettuce_types::ToolExecutionId::new(),
            owner,
            ordinal,
            definition,
            call,
            now,
        )?);
    }
    Ok(executions)
}

impl<R: ConversationRepository> ConversationManager<R> {
    pub fn validate_mutation(
        &self,
        mutation: &ConversationMutation,
    ) -> Result<(), ConversationServiceError> {
        mutation.validate().map_err(Into::into)
    }

    pub fn begin_send(
        &self,
        command: &crate::commands::SendConversation,
        now: TimestampMillis,
    ) -> Result<MutationCommit<BeginGeneration>, ConversationServiceError> {
        ConversationMutation::Send(command.clone()).validate()?;
        self.repository.begin_send(command, now).map_err(Into::into)
    }

    pub fn begin_continue(
        &self,
        command: &crate::commands::ContinueConversation,
        now: TimestampMillis,
    ) -> Result<MutationCommit<BeginGeneration>, ConversationServiceError> {
        ConversationMutation::Continue(command.clone()).validate()?;
        self.repository
            .begin_continue(command, now)
            .map_err(Into::into)
    }

    pub fn begin_regenerate(
        &self,
        command: &crate::commands::RegenerateCandidate,
        now: TimestampMillis,
    ) -> Result<MutationCommit<BeginGeneration>, ConversationServiceError> {
        ConversationMutation::Regenerate(command.clone()).validate()?;
        self.repository
            .begin_regenerate(command, now)
            .map_err(Into::into)
    }

    pub fn begin_retry(
        &self,
        command: &crate::commands::RetryGeneration,
        now: TimestampMillis,
    ) -> Result<MutationCommit<BeginGeneration>, ConversationServiceError> {
        ConversationMutation::Retry(command.clone()).validate()?;
        self.repository
            .begin_retry(command, now)
            .map_err(Into::into)
    }

    pub fn cancel(
        &self,
        command: &crate::commands::CancelGeneration,
        now: TimestampMillis,
    ) -> Result<crate::ports::RequestCancellationResult, ConversationServiceError> {
        ConversationMutation::Cancel(command.clone()).validate()?;
        self.repository
            .request_cancellation(command, now)
            .map_err(Into::into)
    }

    pub fn settle_cancellation(
        &self,
        command: &crate::commands::SettleCancellation,
        now: TimestampMillis,
    ) -> Result<crate::ports::SettleCancellationResult, ConversationServiceError> {
        command.validate()?;
        self.repository
            .settle_cancellation(command, now)
            .map_err(Into::into)
    }

    pub fn attach_attempt_job(
        &self,
        command: &crate::commands::AttachAttemptJob,
        now: TimestampMillis,
    ) -> Result<crate::ports::AttachAttemptJobResult, ConversationServiceError> {
        command.validate()?;
        self.repository
            .attach_attempt_job(command, now)
            .map_err(Into::into)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn finalize_generation(
        &self,
        turn_id: lettuce_types::GenerationTurnId,
        attempt_id: lettuce_types::GenerationAttemptId,
        expected_conversation_revision: lettuce_types::Revision,
        expected_turn_revision: lettuce_types::Revision,
        operation: &crate::commands::OperationToken,
        draft: crate::ports::FinalizationDraft,
        usage_event_id: lettuce_types::UsageEventId,
        now: TimestampMillis,
    ) -> Result<GenerationFinalizationResult, ConversationServiceError> {
        if let Some(replay) = &draft.replay {
            self.repository
                .artifact_store()
                .verify_replay(replay)
                .map_err(ConversationRepositoryError::ArtifactReference)?;
        }
        self.repository
            .finalize_generation(
                turn_id,
                attempt_id,
                expected_conversation_revision,
                expected_turn_revision,
                operation,
                draft,
                usage_event_id,
                now,
            )
            .map_err(Into::into)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn fail_generation(
        &self,
        turn_id: lettuce_types::GenerationTurnId,
        attempt_id: lettuce_types::GenerationAttemptId,
        expected_conversation_revision: lettuce_types::Revision,
        expected_turn_revision: lettuce_types::Revision,
        operation: &crate::commands::OperationToken,
        failure: crate::generation::GenerationFailureCode,
        usage_event_id: lettuce_types::UsageEventId,
        now: TimestampMillis,
    ) -> Result<GenerationFailureResult, ConversationServiceError> {
        self.repository
            .fail_generation(
                turn_id,
                attempt_id,
                expected_conversation_revision,
                expected_turn_revision,
                operation,
                failure,
                usage_event_id,
                now,
            )
            .map_err(Into::into)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn interrupt_generation(
        &self,
        turn_id: lettuce_types::GenerationTurnId,
        attempt_id: lettuce_types::GenerationAttemptId,
        expected_conversation_revision: lettuce_types::Revision,
        expected_turn_revision: lettuce_types::Revision,
        operation: &crate::commands::OperationToken,
        usage_event_id: lettuce_types::UsageEventId,
        now: TimestampMillis,
    ) -> Result<GenerationInterruptionResult, ConversationServiceError> {
        self.repository
            .interrupt_generation(
                turn_id,
                attempt_id,
                expected_conversation_revision,
                expected_turn_revision,
                operation,
                usage_event_id,
                now,
            )
            .map_err(Into::into)
    }

    pub fn recover_generation(
        &self,
        turn_id: lettuce_types::GenerationTurnId,
        attempt_id: lettuce_types::GenerationAttemptId,
        expected_conversation_revision: lettuce_types::Revision,
        expected_turn_revision: lettuce_types::Revision,
        operation: &crate::commands::OperationToken,
        now: TimestampMillis,
    ) -> Result<GenerationRecoveryResult, ConversationServiceError> {
        self.repository
            .recover_generation(
                turn_id,
                attempt_id,
                expected_conversation_revision,
                expected_turn_revision,
                operation,
                now,
            )
            .map_err(Into::into)
    }

    pub fn choose_candidate(
        &self,
        command: &crate::commands::ChooseCandidate,
        now: TimestampMillis,
    ) -> Result<MutationCommit<crate::content::Message>, ConversationServiceError> {
        ConversationMutation::Choose(command.clone()).validate()?;
        self.repository
            .choose_candidate(command, now)
            .map_err(Into::into)
    }

    pub fn edit_message(
        &self,
        command: &crate::commands::EditMessage,
        now: TimestampMillis,
    ) -> Result<EditMessageResult, ConversationServiceError> {
        ConversationMutation::Edit(command.clone()).validate()?;
        self.repository
            .edit_message(command, now)
            .map_err(Into::into)
    }

    pub fn update_message_flags(
        &self,
        command: &crate::commands::UpdateMessageFlags,
        now: TimestampMillis,
    ) -> Result<UpdateMessageFlagsResult, ConversationServiceError> {
        ConversationMutation::Flags(command.clone()).validate()?;
        let result = self.repository.update_message_flags(command, now)?;
        command.validate_result(&result.value)?;
        Ok(result)
    }

    pub fn fork_branch(
        &self,
        command: &crate::commands::ForkBranch,
        now: TimestampMillis,
    ) -> Result<ForkBranchResult, ConversationServiceError> {
        ConversationMutation::Fork(command.clone()).validate()?;
        let result = self.repository.fork_branch(command, now)?;
        result.value.validate()?;
        Ok(result)
    }

    pub fn select_branch(
        &self,
        command: &crate::commands::SelectBranch,
        now: TimestampMillis,
    ) -> Result<SelectBranchResult, ConversationServiceError> {
        ConversationMutation::SelectBranch(command.clone()).validate()?;
        self.repository
            .select_branch(command, now)
            .map_err(Into::into)
    }

    pub fn tombstone_message(
        &self,
        command: &crate::commands::TombstoneMessage,
        now: TimestampMillis,
    ) -> Result<TombstoneMessageResult, ConversationServiceError> {
        ConversationMutation::Tombstone(command.clone()).validate()?;
        let result = self.repository.tombstone_message(command, now)?;
        result.value.validate_for_policy(command.descendants)?;
        Ok(result)
    }

    pub fn archive(
        &self,
        command: &crate::commands::ArchiveConversation,
        now: TimestampMillis,
    ) -> Result<ArchiveConversationResult, ConversationServiceError> {
        ConversationMutation::Archive(command.clone()).validate()?;
        self.repository.archive(command, now).map_err(Into::into)
    }

    pub fn restore(
        &self,
        command: &crate::commands::RestoreConversation,
        now: TimestampMillis,
    ) -> Result<RestoreConversationResult, ConversationServiceError> {
        ConversationMutation::Restore(command.clone()).validate()?;
        self.repository.restore(command, now).map_err(Into::into)
    }

    pub fn rename(
        &self,
        command: &crate::commands::RenameConversation,
        now: TimestampMillis,
    ) -> Result<RenameConversationResult, ConversationServiceError> {
        ConversationMutation::Rename(command.clone()).validate()?;
        self.repository.rename(command, now).map_err(Into::into)
    }

    pub fn update_participant_policy(
        &self,
        command: &crate::commands::UpdateParticipantPolicy,
        now: TimestampMillis,
    ) -> Result<ParticipantPolicyResult, ConversationServiceError> {
        ConversationMutation::ParticipantPolicy(command.clone()).validate()?;
        self.repository
            .update_participant_policy(command, now)
            .map_err(Into::into)
    }

    pub fn update_settings(
        &self,
        update: crate::PreparedConversationSettingsUpdate,
        now: TimestampMillis,
    ) -> Result<SettingsResult, ConversationServiceError> {
        update.command().validate()?;
        self.repository
            .update_settings(update, now)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    struct ReaderOnly;

    #[derive(Default)]
    struct ToolRepository {
        executions: Mutex<Vec<ToolExecution>>,
    }

    impl ToolExecutionRepository for ToolRepository {
        fn append_tool_executions(
            &self,
            expected_next_ordinal: u16,
            executions: &[ToolExecution],
        ) -> Result<Vec<ToolExecution>, ConversationRepositoryError> {
            let mut stored = self.executions.lock().expect("tool repository");
            if stored.len() != usize::from(expected_next_ordinal) {
                return Err(ConversationRepositoryError::Conflict);
            }
            stored.extend_from_slice(executions);
            Ok(executions.to_vec())
        }

        fn get_tool_execution(
            &self,
            id: lettuce_types::ToolExecutionId,
        ) -> Result<ToolExecution, ConversationRepositoryError> {
            self.executions
                .lock()
                .expect("tool repository")
                .iter()
                .find(|execution| execution.id == id)
                .cloned()
                .ok_or(ConversationRepositoryError::NotFound)
        }

        fn list_tool_executions(
            &self,
            conversation_id: ConversationId,
            turn_id: lettuce_types::GenerationTurnId,
            attempt_id: lettuce_types::GenerationAttemptId,
        ) -> Result<Vec<ToolExecution>, ConversationRepositoryError> {
            Ok(self
                .executions
                .lock()
                .expect("tool repository")
                .iter()
                .filter(|execution| {
                    execution.conversation_id == conversation_id
                        && execution.turn_id == turn_id
                        && execution.attempt_id == attempt_id
                })
                .cloned()
                .collect())
        }

        fn transition_tool_execution(
            &self,
            _id: lettuce_types::ToolExecutionId,
            _expected_revision: lettuce_types::Revision,
            _next: ToolExecutionStatus,
            _output: Option<ToolOutput>,
            _failure: Option<ToolFailure>,
            _at: TimestampMillis,
        ) -> Result<ToolExecution, ConversationRepositoryError> {
            Err(ConversationRepositoryError::Storage)
        }

        fn transition_tool_execution_batch(
            &self,
            _transitions: &[ToolExecutionTransition],
            _at: TimestampMillis,
        ) -> Result<Vec<ToolExecution>, ConversationRepositoryError> {
            Err(ConversationRepositoryError::Storage)
        }
    }

    impl ConversationReader for ReaderOnly {
        fn get(
            &self,
            _id: lettuce_types::ConversationId,
        ) -> Result<ConversationAggregate, ConversationRepositoryError> {
            Err(ConversationRepositoryError::Storage)
        }

        fn page(
            &self,
            _query: &ConversationQuery,
        ) -> Result<KeysetPage<ConversationSummary>, ConversationRepositoryError> {
            Err(ConversationRepositoryError::Storage)
        }

        fn timeline_page(
            &self,
            _conversation_id: lettuce_types::ConversationId,
            _branch_id: lettuce_types::ConversationBranchId,
            _page: &lettuce_types::PageRequest,
        ) -> Result<TimelinePage, ConversationRepositoryError> {
            Err(ConversationRepositoryError::Storage)
        }

        fn get_message_revision(
            &self,
            _id: lettuce_types::MessageRevisionId,
        ) -> Result<crate::content::MessageRevision, ConversationRepositoryError> {
            Err(ConversationRepositoryError::Storage)
        }

        fn page_message_revisions(
            &self,
            _message_id: lettuce_types::MessageId,
            _page: &lettuce_types::PageRequest,
        ) -> Result<KeysetPage<crate::content::MessageRevision>, ConversationRepositoryError>
        {
            Err(ConversationRepositoryError::Storage)
        }

        fn get_candidate(
            &self,
            _id: lettuce_types::MessageCandidateId,
        ) -> Result<crate::content::MessageCandidate, ConversationRepositoryError> {
            Err(ConversationRepositoryError::Storage)
        }

        fn page_candidates(
            &self,
            _message_id: lettuce_types::MessageId,
            _page: &lettuce_types::PageRequest,
        ) -> Result<KeysetPage<crate::content::MessageCandidate>, ConversationRepositoryError>
        {
            Err(ConversationRepositoryError::Storage)
        }

        fn get_turn(
            &self,
            _id: lettuce_types::GenerationTurnId,
        ) -> Result<GenerationTurn, ConversationRepositoryError> {
            Err(ConversationRepositoryError::Storage)
        }

        fn page_turns(
            &self,
            _conversation_id: lettuce_types::ConversationId,
            _page: &lettuce_types::PageRequest,
        ) -> Result<KeysetPage<GenerationTurn>, ConversationRepositoryError> {
            Err(ConversationRepositoryError::Storage)
        }

        fn operation_record(
            &self,
            _conversation_id: lettuce_types::ConversationId,
            _kind: OperationKind,
            _token: &crate::commands::OperationToken,
        ) -> Result<Option<crate::ports::OperationRecord>, ConversationRepositoryError> {
            Err(ConversationRepositoryError::Storage)
        }

        fn page_outbox(
            &self,
            _conversation_id: lettuce_types::ConversationId,
            _page: &lettuce_types::PageRequest,
        ) -> Result<KeysetPage<ConversationOutboxRecord>, ConversationRepositoryError> {
            Err(ConversationRepositoryError::Storage)
        }
    }

    #[test]
    fn manager_reads_work_with_a_reader_only_repository() {
        let manager = ConversationManager::new(ReaderOnly);
        let result = manager.page(&ConversationQuery {
            lifecycle: None,
            page: lettuce_types::PageRequest::default(),
        });
        assert!(matches!(
            result,
            Err(ConversationServiceError::Repository(
                ConversationRepositoryError::Storage
            ))
        ));
    }

    #[test]
    fn manager_persists_a_provider_tool_call_set_with_stable_ordinals() {
        let manager = ConversationManager::new(ToolRepository::default());
        let conversation_id = ConversationId::new();
        let turn_id = lettuce_types::GenerationTurnId::new();
        let attempt_id = lettuce_types::GenerationAttemptId::new();
        let owner = ToolExecutionOwner {
            conversation_id,
            turn_id,
            attempt_id,
        };
        let request = ToolRequest {
            definitions: vec![
                crate::ToolDefinition {
                    name: "create_memory".to_owned(),
                    description: None,
                    parameters: json!({"type": "object"}),
                    version: 3,
                },
                crate::ToolDefinition {
                    name: "pin_memory".to_owned(),
                    description: None,
                    parameters: json!({"type": "object"}),
                    version: 2,
                },
            ],
            choice: ToolChoice::Auto,
        };
        let calls = vec![
            ProposedToolCall {
                provider_call_id: Some("call-1".to_owned()),
                name: "create_memory".to_owned(),
                arguments: json!({"content": "one"}),
                raw_arguments: None,
                provider_replay: None,
            },
            ProposedToolCall {
                provider_call_id: Some("call-2".to_owned()),
                name: "pin_memory".to_owned(),
                arguments: json!({"id": "one"}),
                raw_arguments: None,
                provider_replay: None,
            },
        ];

        let stored = manager
            .request_tool_executions(owner, &request, calls, TimestampMillis::new(10))
            .expect("tool executions");

        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].ordinal, 0);
        assert_eq!(stored[0].definition_version, 3);
        assert_eq!(stored[1].ordinal, 1);
        assert_eq!(stored[1].definition_version, 2);
        let next_round = manager
            .request_tool_executions(
                owner,
                &request,
                vec![ProposedToolCall {
                    provider_call_id: Some("call-3".to_owned()),
                    name: "create_memory".to_owned(),
                    arguments: json!({"content": "two"}),
                    raw_arguments: None,
                    provider_replay: None,
                }],
                TimestampMillis::new(20),
            )
            .expect("next tool round");
        assert_eq!(next_round[0].ordinal, 2);
        assert_eq!(
            manager
                .tool_executions(conversation_id, turn_id, attempt_id)
                .expect("stored calls"),
            [stored, next_round].concat()
        );
    }

    #[test]
    fn manager_rejects_undeclared_and_named_choice_mismatched_calls_before_storage() {
        let manager = ConversationManager::new(ToolRepository::default());
        let request = ToolRequest {
            definitions: vec![crate::ToolDefinition {
                name: "create_memory".to_owned(),
                description: None,
                parameters: json!({"type": "object"}),
                version: 1,
            }],
            choice: ToolChoice::Named {
                name: "create_memory".to_owned(),
            },
        };
        let result = manager.request_tool_executions(
            ToolExecutionOwner {
                conversation_id: ConversationId::new(),
                turn_id: lettuce_types::GenerationTurnId::new(),
                attempt_id: lettuce_types::GenerationAttemptId::new(),
            },
            &request,
            vec![ProposedToolCall {
                provider_call_id: None,
                name: "delete_memory".to_owned(),
                arguments: json!({"id": "one"}),
                raw_arguments: None,
                provider_replay: None,
            }],
            TimestampMillis::new(10),
        );

        assert!(matches!(
            result,
            Err(ConversationServiceError::Invalid(
                ValidationError::InvalidReference { .. }
            ))
        ));
        assert!(
            manager
                .repository()
                .executions
                .lock()
                .expect("tool repository")
                .is_empty()
        );
    }

    #[test]
    fn manager_requires_a_call_for_required_tool_choice() {
        let manager = ConversationManager::new(ToolRepository::default());
        let request = ToolRequest {
            definitions: vec![crate::ToolDefinition {
                name: "create_memory".to_owned(),
                description: None,
                parameters: json!({"type": "object"}),
                version: 1,
            }],
            choice: ToolChoice::Required,
        };

        assert!(matches!(
            manager.request_tool_executions(
                ToolExecutionOwner {
                    conversation_id: ConversationId::new(),
                    turn_id: lettuce_types::GenerationTurnId::new(),
                    attempt_id: lettuce_types::GenerationAttemptId::new(),
                },
                &request,
                Vec::new(),
                TimestampMillis::new(10),
            ),
            Err(ConversationServiceError::Invalid(
                ValidationError::InvalidValue { .. }
            ))
        ));
    }
}
