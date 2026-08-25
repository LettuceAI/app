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

    struct ReaderOnly;

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
}
