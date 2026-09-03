use std::collections::{HashMap, HashSet};

use lettuce_companions::CompanionTurnEffectStatus;
use lettuce_conversations::{
    ConversationReader, ConversationRepositoryError, MessageRenderSource, MessageRole,
    MessageVisibility, ResolvedInferenceProfile,
};
use lettuce_jobs::{JobKind, SubjectKind, handle::JobHandle};
use lettuce_memory::{
    DynamicMemoryAttempt, DynamicMemoryAttemptStatus, DynamicMemoryRun,
    DynamicMemoryRunAttemptAdmission, DynamicMemoryRunRepository, DynamicMemoryRunRepositoryError,
    DynamicMemorySourceMessage, DynamicMemoryStructuredFallbackFormat, DynamicMemorySummaryWindow,
    MemoryRepository, MemoryRepositoryError, MemorySummaryRepository,
    NewDynamicMemoryAttemptRecovery, NewDynamicMemoryRunAttempt,
};
use lettuce_types::{
    DynamicMemoryAttemptId, DynamicMemoryRunId, MessageId, PageLimit, PageRequest, TimestampMillis,
};
use uuid::Uuid;

use crate::CompanionPostTurnMemoryAdmission;

const BACKGROUND_MEMORY_NAMESPACE: Uuid =
    Uuid::from_u128(0xb139_3c5f_69a7_55b8_91d6_2a0d_7098_6e31u128);

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionPostTurnMemoryRunDispatch {
    pub run: DynamicMemoryRun,
    pub attempt: DynamicMemoryAttempt,
    pub recovered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompanionPostTurnMemoryRunError {
    #[error("post-turn memory admission is invalid")]
    InvalidAdmission,
    #[error("post-turn memory source timeline failed: {0}")]
    Conversation(ConversationRepositoryError),
    #[error("post-turn memory space failed: {0}")]
    Memory(MemoryRepositoryError),
    #[error("post-turn memory run failed: {0}")]
    Run(DynamicMemoryRunRepositoryError),
}

#[derive(Debug)]
pub struct CompanionPostTurnMemoryRunCoordinator<'a, R: ?Sized, C: ?Sized> {
    repository: &'a R,
    conversations: &'a C,
}

impl<
    'a,
    R: DynamicMemoryRunRepository + MemoryRepository + MemorySummaryRepository + ?Sized,
    C: ConversationReader + ?Sized,
> CompanionPostTurnMemoryRunCoordinator<'a, R, C>
{
    #[must_use]
    pub const fn new(repository: &'a R, conversations: &'a C) -> Self {
        Self {
            repository,
            conversations,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn admit_or_recover(
        &self,
        admission: &CompanionPostTurnMemoryAdmission,
        profile: ResolvedInferenceProfile,
        time_awareness_enabled: bool,
        supersession_enabled: bool,
        structured_fallback_format: DynamicMemoryStructuredFallbackFormat,
        handle: &JobHandle,
        now: TimestampMillis,
    ) -> Result<CompanionPostTurnMemoryRunDispatch, CompanionPostTurnMemoryRunError> {
        validate_job(admission, handle)?;
        let conversation_id = admission.batch.conversation_id;
        let snapshot = self
            .repository
            .get_for_conversation(conversation_id)
            .map_err(CompanionPostTurnMemoryRunError::Memory)?
            .ok_or(CompanionPostTurnMemoryRunError::InvalidAdmission)?;
        let expected_messages = expected_effect_messages(admission)?;
        let run_id = stable_run_id(admission);

        match self.repository.load_dynamic_memory_run(run_id) {
            Ok(run) => {
                if run.conversation_id != conversation_id
                    || run.space_id != snapshot.id
                    || run
                        .source_messages
                        .iter()
                        .map(|source| (source.message_id, source.role))
                        .ne(expected_messages.iter().copied())
                    || run.profile != profile
                    || run.time_awareness_enabled != time_awareness_enabled
                    || run.supersession_enabled != supersession_enabled
                    || run.structured_fallback_format != structured_fallback_format
                    || run.summary_window.message_interval
                        != admission.batch.summary_message_interval
                {
                    return Err(CompanionPostTurnMemoryRunError::InvalidAdmission);
                }
                let latest = self
                    .repository
                    .load_latest_dynamic_memory_attempt(run_id)
                    .map_err(CompanionPostTurnMemoryRunError::Run)?;
                if latest.job_id == handle.id() {
                    if latest.status != DynamicMemoryAttemptStatus::Processing {
                        return Err(CompanionPostTurnMemoryRunError::InvalidAdmission);
                    }
                    return Ok(CompanionPostTurnMemoryRunDispatch {
                        run,
                        attempt: latest,
                        recovered: false,
                    });
                }
                if latest.status != DynamicMemoryAttemptStatus::Processing {
                    return Err(CompanionPostTurnMemoryRunError::InvalidAdmission);
                }
                let child_ordinal = latest
                    .ordinal
                    .checked_add(1)
                    .ok_or(CompanionPostTurnMemoryRunError::InvalidAdmission)?;
                let child_id = stable_attempt_id(run_id, child_ordinal, handle.id());
                let recovered = self
                    .repository
                    .recover_dynamic_memory_attempt(NewDynamicMemoryAttemptRecovery {
                        run_id,
                        parent_attempt_id: latest.id,
                        child_attempt_id: child_id,
                        job_id: handle.id(),
                        now,
                    })
                    .map_err(CompanionPostTurnMemoryRunError::Run)?;
                Ok(CompanionPostTurnMemoryRunDispatch {
                    run: recovered.run,
                    attempt: recovered.child,
                    recovered: true,
                })
            }
            Err(DynamicMemoryRunRepositoryError::NotFound) => {
                let summary_window = summary_window(
                    self.repository
                        .get_summary(snapshot.id)
                        .map_err(CompanionPostTurnMemoryRunError::Memory)?,
                    admission.batch.summary_message_interval,
                    admission.batch.window_selection,
                    admission.batch.unsummarized_message_count,
                    expected_messages.len(),
                )?;
                let source_messages = resolve_source_messages(
                    self.conversations,
                    conversation_id,
                    expected_messages,
                )?;
                let attempt_id = stable_attempt_id(run_id, 0, handle.id());
                let DynamicMemoryRunAttemptAdmission { run, attempt } = self
                    .repository
                    .admit_dynamic_memory_run_attempt(NewDynamicMemoryRunAttempt {
                        run_id,
                        attempt_id,
                        conversation_id,
                        space_id: snapshot.id,
                        starting_memory: snapshot,
                        source_messages,
                        profile,
                        time_awareness_enabled,
                        supersession_enabled,
                        structured_fallback_format,
                        summary_window,
                        job_id: handle.id(),
                        now,
                    })
                    .map_err(CompanionPostTurnMemoryRunError::Run)?;
                let attempt = self
                    .repository
                    .transition_dynamic_memory_attempt(
                        attempt.id,
                        attempt.revision,
                        DynamicMemoryAttemptStatus::Processing,
                        None,
                        now,
                    )
                    .map_err(CompanionPostTurnMemoryRunError::Run)?;
                Ok(CompanionPostTurnMemoryRunDispatch {
                    run,
                    attempt,
                    recovered: false,
                })
            }
            Err(error) => Err(CompanionPostTurnMemoryRunError::Run(error)),
        }
    }
}

fn summary_window(
    previous: Option<lettuce_memory::MemorySummary>,
    message_interval: u32,
    selection: crate::CompanionMemoryWindowSelection,
    unsummarized_message_count: u64,
    source_message_count: usize,
) -> Result<DynamicMemorySummaryWindow, CompanionPostTurnMemoryRunError> {
    if message_interval == 0 || source_message_count == 0 || unsummarized_message_count == 0 {
        return Err(CompanionPostTurnMemoryRunError::InvalidAdmission);
    }
    let cursor = previous.map_or(0, |summary| summary.window_end);
    let source_message_count = u64::try_from(source_message_count)
        .map_err(|_| CompanionPostTurnMemoryRunError::InvalidAdmission)?;
    if source_message_count > unsummarized_message_count {
        return Err(CompanionPostTurnMemoryRunError::InvalidAdmission);
    }
    let (start, end) = match selection {
        crate::CompanionMemoryWindowSelection::Automatic => (
            cursor,
            cursor
                .checked_add(source_message_count)
                .ok_or(CompanionPostTurnMemoryRunError::InvalidAdmission)?,
        ),
        crate::CompanionMemoryWindowSelection::Recent => {
            let end = cursor
                .checked_add(unsummarized_message_count)
                .ok_or(CompanionPostTurnMemoryRunError::InvalidAdmission)?;
            (end - source_message_count, end)
        }
    };
    Ok(DynamicMemorySummaryWindow {
        message_interval,
        start,
        end,
    })
}

fn validate_job(
    admission: &CompanionPostTurnMemoryAdmission,
    handle: &JobHandle,
) -> Result<(), CompanionPostTurnMemoryRunError> {
    let conversation_id = admission.batch.conversation_id;
    if admission.batch.effects.is_empty()
        || admission.job.id != handle.id()
        || admission.job.kind != JobKind::MemoryExtraction
        || admission.job.subject.kind != SubjectKind::Conversation
        || admission.job.subject.id.as_str() != conversation_id.to_string()
        || admission.job.idempotency_key.as_ref() != Some(&admission.batch.idempotency_key)
        || admission.job.state.is_terminal()
        || admission.batch.effects.iter().any(|effect| {
            effect.conversation_id != conversation_id
                || effect.status == CompanionTurnEffectStatus::Invalidated
                || (admission.batch.settle_effects
                    && (effect.status != CompanionTurnEffectStatus::Processing
                        || effect.source_window.is_some()
                        || effect.summary.is_some()))
        })
    {
        return Err(CompanionPostTurnMemoryRunError::InvalidAdmission);
    }
    Ok(())
}

fn expected_effect_messages(
    admission: &CompanionPostTurnMemoryAdmission,
) -> Result<Vec<(MessageId, MessageRole)>, CompanionPostTurnMemoryRunError> {
    let source_effects = admission
        .batch
        .effects
        .get(admission.batch.source_effect_offset..)
        .ok_or(CompanionPostTurnMemoryRunError::InvalidAdmission)?;
    if source_effects.is_empty() {
        return Err(CompanionPostTurnMemoryRunError::InvalidAdmission);
    }
    let mut expected = Vec::with_capacity(source_effects.len() * 2);
    let mut unique = HashSet::new();
    for effect in source_effects {
        if let Some(id) = effect.user_message_id {
            if !unique.insert(id) {
                return Err(CompanionPostTurnMemoryRunError::InvalidAdmission);
            }
            expected.push((id, MessageRole::User));
        }
        if !unique.insert(effect.assistant_message_id) {
            return Err(CompanionPostTurnMemoryRunError::InvalidAdmission);
        }
        expected.push((effect.assistant_message_id, MessageRole::Assistant));
    }
    if expected.len() > lettuce_memory::MAX_DYNAMIC_MEMORY_SOURCE_MESSAGES {
        return Err(CompanionPostTurnMemoryRunError::InvalidAdmission);
    }
    Ok(expected)
}

fn resolve_source_messages<C: ConversationReader + ?Sized>(
    conversations: &C,
    conversation_id: lettuce_types::ConversationId,
    expected: Vec<(MessageId, MessageRole)>,
) -> Result<Vec<DynamicMemorySourceMessage>, CompanionPostTurnMemoryRunError> {
    let aggregate = conversations
        .get(conversation_id)
        .map_err(CompanionPostTurnMemoryRunError::Conversation)?;

    let expected_ids = expected.iter().map(|(id, _)| *id).collect::<HashSet<_>>();
    let mut found = HashMap::<
        MessageId,
        (
            MessageRole,
            MessageVisibility,
            MessageRenderSource,
            TimestampMillis,
        ),
    >::new();
    let mut cursor = None;
    let mut seen_cursors = HashSet::new();
    loop {
        let page = conversations
            .timeline_page(
                conversation_id,
                aggregate.conversation.active_branch_id,
                &PageRequest {
                    cursor: cursor.clone(),
                    limit: PageLimit::new(200),
                },
            )
            .map_err(CompanionPostTurnMemoryRunError::Conversation)?;
        for item in page.items {
            if expected_ids.contains(&item.message.id) {
                found.insert(
                    item.message.id,
                    (
                        item.message.role,
                        item.message.visibility,
                        item.message.active_render_source,
                        item.message.effective_time,
                    ),
                );
            }
        }
        if found.len() == expected.len() {
            break;
        }
        let Some(next) = page.next_cursor else {
            return Err(CompanionPostTurnMemoryRunError::InvalidAdmission);
        };
        if !seen_cursors.insert(next.clone()) {
            return Err(CompanionPostTurnMemoryRunError::InvalidAdmission);
        }
        cursor = Some(next);
    }

    expected
        .into_iter()
        .map(|(message_id, expected_role)| {
            let (role, visibility, render_source, effective_time) = found
                .get(&message_id)
                .copied()
                .ok_or(CompanionPostTurnMemoryRunError::InvalidAdmission)?;
            if role != expected_role || visibility != MessageVisibility::Visible {
                return Err(CompanionPostTurnMemoryRunError::InvalidAdmission);
            }
            Ok(DynamicMemorySourceMessage {
                message_id,
                role,
                render_source,
                effective_time,
            })
        })
        .collect()
}

fn stable_run_id(admission: &CompanionPostTurnMemoryAdmission) -> DynamicMemoryRunId {
    DynamicMemoryRunId::from_uuid(Uuid::new_v5(
        &BACKGROUND_MEMORY_NAMESPACE,
        admission.batch.idempotency_key.as_str().as_bytes(),
    ))
}

fn stable_attempt_id(
    run_id: DynamicMemoryRunId,
    ordinal: u16,
    job_id: lettuce_types::JobId,
) -> DynamicMemoryAttemptId {
    DynamicMemoryAttemptId::from_uuid(Uuid::new_v5(
        &run_id.as_uuid(),
        format!("attempt.{ordinal}.job.{job_id}").as_bytes(),
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use lettuce_companions::{
        CompanionMemoryChanges, CompanionTurnEffect, CompanionTurnEffectSeed,
    };
    use lettuce_conversations::{
        CharacterLaunchSnapshot, Conversation, ConversationAggregate, ConversationKind,
        ConversationLifecycle, ConversationQuery, ConversationSummary, DirectConversationDetails,
        GenerationTurn, KeysetPage, Message, MessageCandidate, MessageRevision, OperationKind,
        OperationRecord, ProtectedSnapshotRef, SnapshotSelection, SnapshotSource, TimelineItem,
        TimelinePage,
    };
    use lettuce_jobs::{
        CancellationPolicy, IdempotencyKey, InMemoryJobStore, JobPriority, JobSpec, OutcomeRef,
        RecoveryPolicy, ResourceClass,
    };
    use lettuce_memory::{
        DynamicMemoryAttemptFailureCode, DynamicMemoryAttemptRecovery, DynamicMemoryInferenceRound,
        DynamicMemoryRunAttemptAdmission, DynamicMemoryToolCallEvidence, MemoryChangeSet,
        MemorySpaceSnapshot, NewDynamicMemoryInferenceRound,
    };
    use lettuce_models::{
        CapabilityStatus, ChatParameterResolutionInput, ChatRequirements, ExpectedModelIdentity,
        ModelCapabilities, ModelKind, ModelProfile, ModelProfileConfig, ProviderAccount,
        ProviderConfig, ProviderProtocol,
    };
    use lettuce_settings::SecretOwnerId;
    use lettuce_types::{
        CharacterId, ContentHash, ConversationBranchId, ConversationId, GenerationTurnId,
        MemorySpaceId, MessageCandidateId, MessageId, MessageRevisionId, ModelProfileId,
        ProviderAccountId, Revision,
    };

    use super::*;

    #[derive(Debug)]
    struct Reader {
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        items: Vec<TimelineItem>,
    }

    impl ConversationReader for Reader {
        fn get(
            &self,
            id: ConversationId,
        ) -> Result<ConversationAggregate, ConversationRepositoryError> {
            if id != self.conversation_id {
                return Err(ConversationRepositoryError::NotFound);
            }
            let character_id = CharacterId::new();
            let now = TimestampMillis::new(1);
            Ok(ConversationAggregate {
                conversation: Conversation {
                    id,
                    lifecycle: ConversationLifecycle::Active,
                    title: "Memory test".into(),
                    kind: ConversationKind::Direct(DirectConversationDetails {
                        format_version: 1,
                        character: CharacterLaunchSnapshot {
                            snapshot_ref: ProtectedSnapshotRef {
                                source: SnapshotSource::Character(character_id),
                                source_revision: Revision::INITIAL,
                                artifact_id: lettuce_types::SnapshotArtifactId::new(),
                                digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
                                schema_version: 1,
                                byte_size: 1,
                            },
                            source_id: character_id,
                            source_revision: Revision::INITIAL,
                            name: "Character".into(),
                            nickname: None,
                        },
                        persona: SnapshotSelection::Disabled,
                        scene: SnapshotSelection::Disabled,
                        starter: SnapshotSelection::Disabled,
                        prompt: SnapshotSelection::Disabled,
                        lorebooks: SnapshotSelection::Explicit(Vec::new()),
                        model: SnapshotSelection::Disabled,
                        memory: SnapshotSelection::Disabled,
                        voice: SnapshotSelection::Disabled,
                    }),
                    active_branch_id: self.branch_id,
                    participants: Vec::new(),
                    current_settings: None,
                    revision: Revision::INITIAL,
                    created_at: now,
                    updated_at: now,
                },
                branches: Vec::new(),
            })
        }

        fn timeline_page(
            &self,
            conversation_id: ConversationId,
            branch_id: ConversationBranchId,
            _page: &PageRequest,
        ) -> Result<TimelinePage, ConversationRepositoryError> {
            if conversation_id != self.conversation_id || branch_id != self.branch_id {
                return Err(ConversationRepositoryError::NotFound);
            }
            Ok(TimelinePage {
                conversation_id,
                selected_branch_id: branch_id,
                branch_path: Vec::new(),
                items: self.items.clone(),
                boundary_parent_id: None,
                next_cursor: None,
            })
        }

        fn page(
            &self,
            _query: &ConversationQuery,
        ) -> Result<KeysetPage<ConversationSummary>, ConversationRepositoryError> {
            unimplemented!()
        }
        fn get_message_revision(
            &self,
            _id: MessageRevisionId,
        ) -> Result<MessageRevision, ConversationRepositoryError> {
            unimplemented!()
        }
        fn page_message_revisions(
            &self,
            _message_id: MessageId,
            _page: &PageRequest,
        ) -> Result<KeysetPage<MessageRevision>, ConversationRepositoryError> {
            unimplemented!()
        }
        fn get_candidate(
            &self,
            _id: MessageCandidateId,
        ) -> Result<MessageCandidate, ConversationRepositoryError> {
            unimplemented!()
        }
        fn page_candidates(
            &self,
            _message_id: MessageId,
            _page: &PageRequest,
        ) -> Result<KeysetPage<MessageCandidate>, ConversationRepositoryError> {
            unimplemented!()
        }
        fn get_turn(
            &self,
            _id: GenerationTurnId,
        ) -> Result<GenerationTurn, ConversationRepositoryError> {
            unimplemented!()
        }
        fn page_turns(
            &self,
            _conversation_id: ConversationId,
            _page: &PageRequest,
        ) -> Result<KeysetPage<GenerationTurn>, ConversationRepositoryError> {
            unimplemented!()
        }
        fn operation_record(
            &self,
            _conversation_id: ConversationId,
            _kind: OperationKind,
            _token: &lettuce_conversations::OperationToken,
        ) -> Result<Option<OperationRecord>, ConversationRepositoryError> {
            unimplemented!()
        }
        fn page_outbox(
            &self,
            _conversation_id: ConversationId,
            _page: &PageRequest,
        ) -> Result<
            KeysetPage<lettuce_conversations::ConversationOutboxRecord>,
            ConversationRepositoryError,
        > {
            unimplemented!()
        }
    }

    #[derive(Debug)]
    struct Repository {
        conversation_id: ConversationId,
        snapshot: MemorySpaceSnapshot,
        summary: Mutex<Option<lettuce_memory::MemorySummary>>,
        run: Mutex<Option<DynamicMemoryRun>>,
        attempts: Mutex<Vec<DynamicMemoryAttempt>>,
    }

    impl MemoryRepository for Repository {
        fn create(
            &self,
            _snapshot: MemorySpaceSnapshot,
        ) -> Result<MemorySpaceSnapshot, MemoryRepositoryError> {
            unimplemented!()
        }
        fn get(
            &self,
            _id: MemorySpaceId,
        ) -> Result<Option<MemorySpaceSnapshot>, MemoryRepositoryError> {
            unimplemented!()
        }
        fn get_for_conversation(
            &self,
            id: ConversationId,
        ) -> Result<Option<MemorySpaceSnapshot>, MemoryRepositoryError> {
            Ok((id == self.conversation_id).then(|| self.snapshot.clone()))
        }
        fn compare_and_apply(
            &self,
            _change: MemoryChangeSet,
        ) -> Result<MemorySpaceSnapshot, MemoryRepositoryError> {
            unimplemented!()
        }
    }

    impl lettuce_memory::MemorySummaryRepository for Repository {
        fn get_summary(
            &self,
            _space_id: MemorySpaceId,
        ) -> Result<Option<lettuce_memory::MemorySummary>, MemoryRepositoryError> {
            Ok(self.summary.lock().expect("summary").clone())
        }

        fn compare_and_apply_summary(
            &self,
            _change: lettuce_memory::MemorySummaryChange,
        ) -> Result<lettuce_memory::MemorySummaryCommit, MemoryRepositoryError> {
            unimplemented!()
        }
    }

    impl DynamicMemoryRunRepository for Repository {
        fn admit_dynamic_memory_run_attempt(
            &self,
            input: NewDynamicMemoryRunAttempt,
        ) -> Result<DynamicMemoryRunAttemptAdmission, DynamicMemoryRunRepositoryError> {
            if self.run.lock().expect("run").is_some() {
                return Err(DynamicMemoryRunRepositoryError::Conflict);
            }
            let run = DynamicMemoryRun {
                id: input.run_id,
                conversation_id: input.conversation_id,
                space_id: input.space_id,
                starting_memory: input.starting_memory,
                source_messages: input.source_messages,
                profile: input.profile,
                time_awareness_enabled: input.time_awareness_enabled,
                supersession_enabled: input.supersession_enabled,
                structured_fallback_format: input.structured_fallback_format,
                summary_window: input.summary_window,
                tool_request: lettuce_memory::dynamic_memory_tool_request_for_run(
                    input.supersession_enabled,
                    input.time_awareness_enabled,
                ),
                created_at: input.now,
            };
            let attempt = DynamicMemoryAttempt {
                id: input.attempt_id,
                run_id: input.run_id,
                ordinal: 0,
                retry_parent_id: None,
                job_id: input.job_id,
                status: DynamicMemoryAttemptStatus::Created,
                failure: None,
                revision: Revision::INITIAL,
                created_at: input.now,
                started_at: None,
                finished_at: None,
                updated_at: input.now,
            };
            *self.run.lock().expect("run") = Some(run.clone());
            self.attempts
                .lock()
                .expect("attempts")
                .push(attempt.clone());
            Ok(DynamicMemoryRunAttemptAdmission { run, attempt })
        }

        fn load_dynamic_memory_run(
            &self,
            id: DynamicMemoryRunId,
        ) -> Result<DynamicMemoryRun, DynamicMemoryRunRepositoryError> {
            self.run
                .lock()
                .expect("run")
                .clone()
                .filter(|run| run.id == id)
                .ok_or(DynamicMemoryRunRepositoryError::NotFound)
        }

        fn load_dynamic_memory_attempt(
            &self,
            id: DynamicMemoryAttemptId,
        ) -> Result<DynamicMemoryAttempt, DynamicMemoryRunRepositoryError> {
            self.attempts
                .lock()
                .expect("attempts")
                .iter()
                .find(|attempt| attempt.id == id)
                .cloned()
                .ok_or(DynamicMemoryRunRepositoryError::NotFound)
        }

        fn load_latest_dynamic_memory_attempt(
            &self,
            run_id: DynamicMemoryRunId,
        ) -> Result<DynamicMemoryAttempt, DynamicMemoryRunRepositoryError> {
            self.attempts
                .lock()
                .expect("attempts")
                .iter()
                .filter(|attempt| attempt.run_id == run_id)
                .max_by_key(|attempt| attempt.ordinal)
                .cloned()
                .ok_or(DynamicMemoryRunRepositoryError::NotFound)
        }

        fn transition_dynamic_memory_attempt(
            &self,
            id: DynamicMemoryAttemptId,
            expected_revision: Revision,
            next: DynamicMemoryAttemptStatus,
            failure: Option<DynamicMemoryAttemptFailureCode>,
            at: TimestampMillis,
        ) -> Result<DynamicMemoryAttempt, DynamicMemoryRunRepositoryError> {
            let mut attempts = self.attempts.lock().expect("attempts");
            let attempt = attempts
                .iter_mut()
                .find(|attempt| attempt.id == id && attempt.revision == expected_revision)
                .ok_or(DynamicMemoryRunRepositoryError::Conflict)?;
            *attempt = attempt
                .transition(next, failure, at)
                .map_err(|_| DynamicMemoryRunRepositoryError::Conflict)?;
            Ok(attempt.clone())
        }

        fn recover_dynamic_memory_attempt(
            &self,
            input: NewDynamicMemoryAttemptRecovery,
        ) -> Result<DynamicMemoryAttemptRecovery, DynamicMemoryRunRepositoryError> {
            let run = self.load_dynamic_memory_run(input.run_id)?;
            let mut attempts = self.attempts.lock().expect("attempts");
            let parent_index = attempts
                .iter()
                .position(|attempt| attempt.id == input.parent_attempt_id)
                .ok_or(DynamicMemoryRunRepositoryError::NotFound)?;
            let parent = attempts[parent_index]
                .transition(DynamicMemoryAttemptStatus::Interrupted, None, input.now)
                .map_err(|_| DynamicMemoryRunRepositoryError::Conflict)?;
            attempts[parent_index] = parent.clone();
            let child = DynamicMemoryAttempt {
                id: input.child_attempt_id,
                run_id: input.run_id,
                ordinal: parent.ordinal + 1,
                retry_parent_id: Some(parent.id),
                job_id: input.job_id,
                status: DynamicMemoryAttemptStatus::Processing,
                failure: None,
                revision: Revision::INITIAL,
                created_at: input.now,
                started_at: Some(input.now),
                finished_at: None,
                updated_at: input.now,
            };
            attempts.push(child.clone());
            Ok(DynamicMemoryAttemptRecovery { run, parent, child })
        }

        fn admit_dynamic_memory_inference_round(
            &self,
            _run_id: DynamicMemoryRunId,
            _attempt_id: DynamicMemoryAttemptId,
            _expected_round_ordinal: u8,
            _expected_next_call_ordinal: u16,
            _round: NewDynamicMemoryInferenceRound,
        ) -> Result<DynamicMemoryInferenceRound, DynamicMemoryRunRepositoryError> {
            unimplemented!()
        }
        fn list_dynamic_memory_inference_rounds(
            &self,
            _run_id: DynamicMemoryRunId,
            _attempt_id: DynamicMemoryAttemptId,
        ) -> Result<Vec<DynamicMemoryInferenceRound>, DynamicMemoryRunRepositoryError> {
            Ok(Vec::new())
        }
        fn list_dynamic_memory_tool_calls(
            &self,
            _run_id: DynamicMemoryRunId,
            _attempt_id: DynamicMemoryAttemptId,
        ) -> Result<Vec<DynamicMemoryToolCallEvidence>, DynamicMemoryRunRepositoryError> {
            Ok(Vec::new())
        }
    }

    fn profile() -> ResolvedInferenceProfile {
        let account_id = ProviderAccountId::new();
        let profile_id = ModelProfileId::new();
        let account = ProviderAccount {
            id: account_id,
            secret_owner_id: SecretOwnerId::new(),
            provider_kind: "ollama".into(),
            protocol: ProviderProtocol::Ollama,
            label: "Ollama".into(),
            endpoint: Some("http://127.0.0.1:11434".into()),
            enabled: true,
            streaming_enabled: false,
            allow_invalid_tls: false,
            api_key_ref: None,
            secret_headers: Vec::new(),
            config: ProviderConfig::Standard,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        let model = ModelProfile {
            id: profile_id,
            provider_account_id: account_id,
            external_model_id: "memory-model".into(),
            display_name: "Memory model".into(),
            kind: ModelKind::Chat,
            config: ModelProfileConfig {
                chat_parameters: Default::default(),
                capabilities: ModelCapabilities {
                    input_modalities: lettuce_models::ModalityCapabilities {
                        text: CapabilityStatus::Supported,
                        ..Default::default()
                    },
                    output_modalities: lettuce_models::ModalityCapabilities {
                        text: CapabilityStatus::Supported,
                        ..Default::default()
                    },
                    tools: CapabilityStatus::Supported,
                    ..Default::default()
                },
            },
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        let expected = ExpectedModelIdentity {
            model_profile_id: profile_id,
            model_revision: model.revision,
            provider_account_id: account_id,
            provider_account_revision: account.revision,
            external_model_id: model.external_model_id.clone(),
            display_name: model.display_name.clone(),
            provider_protocol: account.protocol,
            model_kind: ModelKind::Chat,
        };
        ResolvedInferenceProfile {
            chat_profile: lettuce_models::resolve_chat_profile(
                &expected,
                &model,
                &account,
                &ChatParameterResolutionInput::default(),
                &ChatRequirements {
                    require_tools: true,
                    ..Default::default()
                },
            )
            .expect("profile"),
            tool_policy: lettuce_conversations::ToolPolicy::Required,
            output_policy: lettuce_conversations::OutputPolicy::Plain,
            safety_policy: lettuce_conversations::SafetyContext::Standard,
            correlation_id: None,
        }
    }

    fn timeline_item(
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        id: MessageId,
        role: MessageRole,
        render_source: MessageRenderSource,
    ) -> TimelineItem {
        TimelineItem {
            message: Message {
                id,
                conversation_id,
                branch_id,
                parent_message_id: None,
                author_participant_id: Some(lettuce_types::ConversationParticipantId::new()),
                role,
                logical_time: TimestampMillis::new(1),
                effective_time: TimestampMillis::new(1),
                visibility: MessageVisibility::Visible,
                pinned: false,
                scene_edited: false,
                active_render_source: render_source,
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(1),
                updated_at: TimestampMillis::new(1),
            },
            active_revision: None,
            active_candidate: None,
            initial_origin: None,
        }
    }

    fn admission(
        store: &InMemoryJobStore,
        conversation_id: ConversationId,
        effects: Vec<CompanionTurnEffect>,
    ) -> CompanionPostTurnMemoryAdmission {
        let unsummarized_message_count = effects
            .iter()
            .map(|effect| {
                if effect.user_message_id.is_some() {
                    2
                } else {
                    1
                }
            })
            .sum();
        let key = IdempotencyKey::new("companion-memory-fixed-batch").expect("key");
        let spec = JobSpec::new(
            JobKind::MemoryExtraction,
            lettuce_jobs::JobSubject::new(SubjectKind::Conversation, conversation_id.to_string())
                .expect("subject"),
            OutcomeRef::Conversation(conversation_id),
        )
        .with_idempotency_key(key.clone())
        .with_resources(vec![ResourceClass::Network])
        .with_priority(JobPriority::Background)
        .with_policies(RecoveryPolicy::Restart, CancellationPolicy::Cooperative);
        let admitted = store.create_or_get(spec).expect("job");
        CompanionPostTurnMemoryAdmission {
            batch: crate::CompanionPostTurnMemoryBatch {
                conversation_id,
                idempotency_key: key,
                summary_message_interval: 20,
                window_selection: crate::CompanionMemoryWindowSelection::Automatic,
                unsummarized_message_count,
                source_effect_offset: 0,
                effects,
                settle_effects: true,
            },
            job: admitted.job,
            created: admitted.created,
        }
    }

    #[test]
    fn new_summary_window_starts_after_the_durable_cursor() {
        let space_id = MemorySpaceId::new();
        let previous = lettuce_memory::MemorySummary {
            space_id,
            text: "previous".to_owned(),
            token_count: 1,
            window_start: 6,
            window_end: 10,
            source_message_ids: (0..4).map(|_| MessageId::new()).collect(),
            updated_at: TimestampMillis::new(1),
        };
        assert_eq!(
            summary_window(
                Some(previous.clone()),
                4,
                crate::CompanionMemoryWindowSelection::Automatic,
                4,
                4,
            )
            .expect("window"),
            lettuce_memory::DynamicMemorySummaryWindow {
                message_interval: 4,
                start: 10,
                end: 14,
            }
        );
        assert_eq!(
            summary_window(
                Some(previous),
                4,
                crate::CompanionMemoryWindowSelection::Recent,
                10,
                4,
            )
            .expect("recent window"),
            lettuce_memory::DynamicMemorySummaryWindow {
                message_interval: 4,
                start: 16,
                end: 20,
            }
        );
    }

    #[test]
    fn bridge_freezes_ordered_render_sources_and_recovers_the_same_run() {
        let conversation_id = ConversationId::new();
        let branch_id = ConversationBranchId::new();
        let user_one = MessageId::new();
        let assistant_one = MessageId::new();
        let user_two = MessageId::new();
        let assistant_two = MessageId::new();
        let sources = [
            MessageRenderSource::Revision(MessageRevisionId::new()),
            MessageRenderSource::Candidate(MessageCandidateId::new()),
            MessageRenderSource::Revision(MessageRevisionId::new()),
            MessageRenderSource::Candidate(MessageCandidateId::new()),
        ];
        let reader = Reader {
            conversation_id,
            branch_id,
            items: vec![
                timeline_item(
                    conversation_id,
                    branch_id,
                    assistant_two,
                    MessageRole::Assistant,
                    sources[3],
                ),
                timeline_item(
                    conversation_id,
                    branch_id,
                    user_two,
                    MessageRole::User,
                    sources[2],
                ),
                timeline_item(
                    conversation_id,
                    branch_id,
                    assistant_one,
                    MessageRole::Assistant,
                    sources[1],
                ),
                timeline_item(
                    conversation_id,
                    branch_id,
                    user_one,
                    MessageRole::User,
                    sources[0],
                ),
            ],
        };
        let effects = vec![
            CompanionTurnEffect {
                id: lettuce_types::CompanionEffectId::new(),
                conversation_id,
                turn_id: GenerationTurnId::new(),
                user_message_id: Some(user_one),
                assistant_message_id: assistant_one,
                status: CompanionTurnEffectStatus::Processing,
                summary: None,
                seed: CompanionTurnEffectSeed::default(),
                memory_changes: CompanionMemoryChanges::default(),
                source_window: None,
                created_at: TimestampMillis::new(2),
                updated_at: TimestampMillis::new(2),
            },
            CompanionTurnEffect {
                id: lettuce_types::CompanionEffectId::new(),
                conversation_id,
                turn_id: GenerationTurnId::new(),
                user_message_id: Some(user_two),
                assistant_message_id: assistant_two,
                status: CompanionTurnEffectStatus::Processing,
                summary: None,
                seed: CompanionTurnEffectSeed::default(),
                memory_changes: CompanionMemoryChanges::default(),
                source_window: None,
                created_at: TimestampMillis::new(3),
                updated_at: TimestampMillis::new(3),
            },
        ];
        let repository = Repository {
            conversation_id,
            snapshot: MemorySpaceSnapshot {
                id: MemorySpaceId::new(),
                revision: Revision::INITIAL,
                items: Vec::new(),
            },
            summary: Mutex::new(None),
            run: Mutex::new(None),
            attempts: Mutex::new(Vec::new()),
        };
        let coordinator = CompanionPostTurnMemoryRunCoordinator::new(&repository, &reader);
        let first_store = InMemoryJobStore::new();
        let first_admission = admission(&first_store, conversation_id, effects.clone());
        let first_handle = JobHandle::new(first_admission.job.id);
        let resolved_profile = profile();
        let first = coordinator
            .admit_or_recover(
                &first_admission,
                resolved_profile.clone(),
                true,
                true,
                DynamicMemoryStructuredFallbackFormat::Xml,
                &first_handle,
                TimestampMillis::new(10),
            )
            .expect("first admission");
        assert_eq!(
            first
                .run
                .source_messages
                .iter()
                .map(|source| {
                    (
                        source.message_id,
                        source.role,
                        source.render_source,
                        source.effective_time,
                    )
                })
                .collect::<Vec<_>>(),
            vec![
                (
                    user_one,
                    MessageRole::User,
                    sources[0],
                    TimestampMillis::new(1)
                ),
                (
                    assistant_one,
                    MessageRole::Assistant,
                    sources[1],
                    TimestampMillis::new(1),
                ),
                (
                    user_two,
                    MessageRole::User,
                    sources[2],
                    TimestampMillis::new(1)
                ),
                (
                    assistant_two,
                    MessageRole::Assistant,
                    sources[3],
                    TimestampMillis::new(1),
                ),
            ]
        );
        assert_eq!(first.attempt.status, DynamicMemoryAttemptStatus::Processing);
        assert!(first.run.time_awareness_enabled);
        assert!(first.run.supersession_enabled);
        assert_eq!(
            first.run.summary_window,
            lettuce_memory::DynamicMemorySummaryWindow {
                message_interval: 20,
                start: 0,
                end: 4,
            }
        );
        assert_eq!(
            first.run.tool_request,
            lettuce_memory::dynamic_memory_tool_request_for_run(true, true)
        );
        *repository.summary.lock().expect("summary") = Some(lettuce_memory::MemorySummary {
            space_id: repository.snapshot.id,
            text: "persisted by the run".to_owned(),
            token_count: 4,
            window_start: first.run.summary_window.start,
            window_end: first.run.summary_window.end,
            source_message_ids: first
                .run
                .source_messages
                .iter()
                .map(|source| source.message_id)
                .collect(),
            updated_at: TimestampMillis::new(10),
        });
        assert!(!first.recovered);
        assert_eq!(
            coordinator.admit_or_recover(
                &first_admission,
                profile(),
                true,
                true,
                DynamicMemoryStructuredFallbackFormat::Json,
                &first_handle,
                TimestampMillis::new(11),
            ),
            Err(CompanionPostTurnMemoryRunError::InvalidAdmission)
        );
        let mut changed_interval = first_admission.clone();
        changed_interval.batch.summary_message_interval = 21;
        assert_eq!(
            coordinator.admit_or_recover(
                &changed_interval,
                resolved_profile.clone(),
                true,
                true,
                DynamicMemoryStructuredFallbackFormat::Xml,
                &first_handle,
                TimestampMillis::new(11),
            ),
            Err(CompanionPostTurnMemoryRunError::InvalidAdmission)
        );
        assert_eq!(
            coordinator.admit_or_recover(
                &first_admission,
                resolved_profile.clone(),
                true,
                false,
                DynamicMemoryStructuredFallbackFormat::Xml,
                &first_handle,
                TimestampMillis::new(11),
            ),
            Err(CompanionPostTurnMemoryRunError::InvalidAdmission)
        );
        assert_eq!(
            coordinator
                .admit_or_recover(
                    &first_admission,
                    resolved_profile.clone(),
                    true,
                    true,
                    DynamicMemoryStructuredFallbackFormat::Xml,
                    &first_handle,
                    TimestampMillis::new(11),
                )
                .expect("exact replay"),
            first
        );

        let mut mismatched_effects = effects.clone();
        mismatched_effects[1].assistant_message_id = MessageId::new();
        let mismatched_admission = admission(&first_store, conversation_id, mismatched_effects);
        assert_eq!(
            coordinator.admit_or_recover(
                &mismatched_admission,
                resolved_profile.clone(),
                true,
                true,
                DynamicMemoryStructuredFallbackFormat::Xml,
                &first_handle,
                TimestampMillis::new(11),
            ),
            Err(CompanionPostTurnMemoryRunError::InvalidAdmission)
        );

        let restarted_store = InMemoryJobStore::new();
        let restarted_admission = admission(&restarted_store, conversation_id, effects);
        let restarted_handle = JobHandle::new(restarted_admission.job.id);
        let recovered = coordinator
            .admit_or_recover(
                &restarted_admission,
                resolved_profile,
                true,
                true,
                DynamicMemoryStructuredFallbackFormat::Xml,
                &restarted_handle,
                TimestampMillis::new(12),
            )
            .expect("restart recovery");
        assert_eq!(recovered.run.id, first.run.id);
        assert_eq!(recovered.attempt.retry_parent_id, Some(first.attempt.id));
        assert_eq!(recovered.attempt.job_id, restarted_handle.id());
        assert!(recovered.recovered);
    }
}
