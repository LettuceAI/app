use lettuce_characters::{CharacterRepository, GroupRepository, RepositoryError};
use lettuce_companions::{
    CompanionGrowthRun, CompanionGrowthRunRepository, CompanionGrowthRunRepositoryError,
    GroupCompanionRepository, GrowthMemoryEvidence, MAX_GROWTH_MEMORIES, SoulOwner, SoulRepository,
    SoulRepositoryError,
};
use lettuce_conversations::{
    Conversation, ConversationKind, ConversationReader, ConversationRepositoryError,
};
use lettuce_jobs::{
    CancellationPolicy, IdempotencyKey, JobKind, JobPriority, JobSnapshot, JobSpec, JobStore,
    JobSubject, OutcomeRef, RecoveryPolicy, ResourceClass, StoreError, SubjectKind,
};
use lettuce_memory::{DynamicMemoryAttemptStatus, MemoryItem, MemoryRepository};
use lettuce_types::{CharacterId, OperationRecordId, TimestampMillis};
use uuid::Uuid;

use crate::CompanionMemoryJobRunResult;

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionGrowthJobAdmission {
    pub run: CompanionGrowthRun,
    pub job: JobSnapshot,
    pub created: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionGrowthJobAdmissionError {
    #[error("companion growth source is invalid: {reason}")]
    InvalidSource { reason: String },
    #[error("companion growth conversation read failed: {0}")]
    Conversation(#[from] ConversationRepositoryError),
    #[error("companion growth character read failed: {0}")]
    Character(#[from] RepositoryError),
    #[error("companion growth memory read failed: {0}")]
    Memory(#[from] lettuce_memory::MemoryRepositoryError),
    #[error("companion growth Soul read failed: {0:?}")]
    Soul(SoulRepositoryError),
    #[error("companion growth job admission failed: {0}")]
    Job(StoreError),
    #[error("companion growth run persistence failed: {0}")]
    Run(CompanionGrowthRunRepositoryError),
    #[error("companion growth model is unavailable")]
    Model,
}

#[derive(Debug)]
pub struct CompanionGrowthJobAdmissionCoordinator<'a, R: ?Sized, J: ?Sized> {
    sources: &'a R,
    jobs: &'a J,
}

impl<'a, R: ?Sized, J: ?Sized> CompanionGrowthJobAdmissionCoordinator<'a, R, J> {
    #[must_use]
    pub const fn new(sources: &'a R, jobs: &'a J) -> Self {
        Self { sources, jobs }
    }
}

impl<
    R: ConversationReader
        + CharacterRepository
        + GroupRepository
        + GroupCompanionRepository
        + MemoryRepository
        + lettuce_companions::CompanionStateRepository
        + SoulRepository
        + CompanionGrowthRunRepository
        + lettuce_models::ModelProfileRepository
        + lettuce_models::ProviderAccountRepository
        + lettuce_models::GlobalModelSettingsRepository
        + ?Sized,
    J: JobStore + ?Sized,
> CompanionGrowthJobAdmissionCoordinator<'_, R, J>
{
    pub fn admit_after_memory(
        &self,
        result: &CompanionMemoryJobRunResult,
    ) -> Result<Option<CompanionGrowthJobAdmission>, CompanionGrowthJobAdmissionError> {
        if result.fresh_memories.is_empty() {
            return Ok(None);
        }
        if result.fresh_memories.len() > MAX_GROWTH_MEMORIES
            || result
                .fresh_memories
                .iter()
                .any(|memory| memory.text.trim().is_empty())
            || result.dispatch.attempt.status != DynamicMemoryAttemptStatus::Succeeded
            || result.dispatch.attempt.run_id != result.dispatch.run.id
        {
            return Err(CompanionGrowthJobAdmissionError::InvalidSource {
                reason:
                    "growth result has empty memories or the attempt did not succeed for its run"
                        .to_owned(),
            });
        }
        let conversation =
            ConversationReader::get(self.sources, result.dispatch.run.conversation_id)?;
        let ConversationKind::Direct(details) = &conversation.conversation.kind else {
            return Ok(None);
        };
        let character_id = details.character.source_id;
        let character = CharacterRepository::get(self.sources, character_id)?.ok_or(
            CompanionGrowthJobAdmissionError::InvalidSource {
                reason: "the character of the conversation is missing".to_owned(),
            },
        )?;
        let clock = crate::companion::companion_clock::conversation_clock_context(
            self.sources,
            &conversation.conversation,
        )
        .map_err(|error| CompanionGrowthJobAdmissionError::InvalidSource {
            reason: format!("{error:?}"),
        })?;
        if !clock.companion {
            return Ok(None);
        }
        let config = character
            .character
            .defaults
            .companion_soul
            .unwrap_or_default();
        let created_at = clock.effective_now(result.dispatch.attempt.finished_at.ok_or(
            CompanionGrowthJobAdmissionError::InvalidSource {
                reason: "the memory attempt has no finish time".to_owned(),
            },
        )?);
        let idempotency_key =
            IdempotencyKey::new(format!("companion-growth-{}", result.dispatch.run.id)).map_err(
                |error| CompanionGrowthJobAdmissionError::InvalidSource {
                    reason: error.to_string(),
                },
            )?;
        let admission = self.admit_growth(
            result,
            &conversation.conversation,
            character_id,
            &config,
            character.character.profile.name,
            &result.fresh_memories,
            created_at,
            idempotency_key,
        )?;
        Ok(Some(admission))
    }

    pub fn admit_group_after_memory(
        &self,
        result: &CompanionMemoryJobRunResult,
    ) -> Result<Vec<CompanionGrowthJobAdmission>, CompanionGrowthJobAdmissionError> {
        if result.dispatch.attempt.status != DynamicMemoryAttemptStatus::Succeeded
            || result.dispatch.attempt.run_id != result.dispatch.run.id
        {
            return Err(CompanionGrowthJobAdmissionError::InvalidSource {
                reason: "growth result's attempt did not succeed for its run".to_owned(),
            });
        }
        let conversation =
            ConversationReader::get(self.sources, result.dispatch.run.conversation_id)?;
        if !conversation.conversation.kind.is_group() {
            return Ok(Vec::new());
        }
        let companion = crate::generation::live_sources::live_companion_mode(
            self.sources,
            &conversation.conversation,
        )
        .map_err(|error| CompanionGrowthJobAdmissionError::InvalidSource {
            reason: format!("{error:?}"),
        })?;
        if !companion {
            return Ok(Vec::new());
        }
        let clock = crate::companion::companion_clock::conversation_clock_context(
            self.sources,
            &conversation.conversation,
        )
        .map_err(|error| CompanionGrowthJobAdmissionError::InvalidSource {
            reason: format!("{error:?}"),
        })?;
        let finished_at = result.dispatch.attempt.finished_at.ok_or(
            CompanionGrowthJobAdmissionError::InvalidSource {
                reason: "the memory attempt has no finish time".to_owned(),
            },
        )?;
        let started_at = result.dispatch.attempt.started_at.ok_or(
            CompanionGrowthJobAdmissionError::InvalidSource {
                reason: "the memory attempt has no start time".to_owned(),
            },
        )?;
        let created_at = clock.effective_now(finished_at);
        let source_messages = result
            .dispatch
            .run
            .source_messages
            .iter()
            .map(|source| source.message_id)
            .collect::<std::collections::HashSet<_>>();
        let mut participants = conversation
            .conversation
            .participants
            .iter()
            .collect::<Vec<_>>();
        participants.sort_by_key(|participant| participant.ordinal);
        let mut admissions = Vec::new();
        for participant in participants {
            if !participant.enabled {
                continue;
            }
            let lettuce_conversations::ParticipantSource::Character(character_id) =
                participant.source
            else {
                continue;
            };
            let character = CharacterRepository::get(self.sources, character_id)?.ok_or(
                CompanionGrowthJobAdmissionError::InvalidSource {
                    reason: "a character of the group is missing".to_owned(),
                },
            )?;
            let Some(config) = character.character.defaults.companion_soul else {
                continue;
            };
            let space_id = match GroupCompanionRepository::perspective_space(
                self.sources,
                conversation.conversation.id,
                result.dispatch.run.branch_id,
                character_id,
            )
            .map_err(|error| CompanionGrowthJobAdmissionError::InvalidSource {
                reason: format!("{error:?}"),
            })? {
                Some(space_id) => space_id,
                None => continue,
            };
            let Some(snapshot) = MemoryRepository::get(self.sources, space_id)? else {
                continue;
            };
            let fresh_memories = snapshot
                .items
                .iter()
                .filter(|memory| {
                    memory
                        .source_message_id
                        .is_some_and(|id| source_messages.contains(&id))
                        && memory.created_at >= started_at
                        && memory.superseded_by.is_none()
                        && memory.superseded_at.is_none()
                        && !memory.text.trim().is_empty()
                })
                .take(MAX_GROWTH_MEMORIES)
                .cloned()
                .collect::<Vec<_>>();
            if fresh_memories.is_empty() {
                continue;
            }
            let idempotency_key = IdempotencyKey::new(format!(
                "companion-growth-{}-{character_id}",
                result.dispatch.run.id
            ))
            .map_err(|error| CompanionGrowthJobAdmissionError::InvalidSource {
                reason: error.to_string(),
            })?;
            admissions.push(self.admit_growth(
                result,
                &conversation.conversation,
                character_id,
                &config,
                character.character.profile.name,
                &fresh_memories,
                created_at,
                idempotency_key,
            )?);
        }
        Ok(admissions)
    }

    #[allow(clippy::too_many_arguments)]
    fn admit_growth(
        &self,
        result: &CompanionMemoryJobRunResult,
        conversation: &Conversation,
        character_id: CharacterId,
        config: &lettuce_companions::CompanionSoulConfig,
        companion_name: String,
        fresh_memories: &[MemoryItem],
        created_at: TimestampMillis,
        idempotency_key: IdempotencyKey,
    ) -> Result<CompanionGrowthJobAdmission, CompanionGrowthJobAdmissionError> {
        let subject = JobSubject::new(SubjectKind::Conversation, conversation.id.to_string())
            .map_err(|error| CompanionGrowthJobAdmissionError::InvalidSource {
                reason: error.to_string(),
            })?;
        let admitted = self
            .jobs
            .create_or_get(
                JobSpec::new(
                    JobKind::CompanionGrowth,
                    subject,
                    OutcomeRef::Character(character_id),
                )
                .with_idempotency_key(idempotency_key)
                .with_resources(vec![
                    ResourceClass::Network,
                    ResourceClass::ModelLoad,
                    ResourceClass::DiskRead,
                    ResourceClass::DiskWrite,
                    ResourceClass::Cpu,
                ])
                .with_priority(JobPriority::Background)
                .with_policies(RecoveryPolicy::Restart, CancellationPolicy::Cooperative),
            )
            .map_err(CompanionGrowthJobAdmissionError::Job)?;
        if !admitted.created {
            match self.sources.load_companion_growth_run(admitted.job.id) {
                Ok(run) => {
                    return Ok(CompanionGrowthJobAdmission {
                        run,
                        job: admitted.job,
                        created: false,
                    });
                }
                Err(CompanionGrowthRunRepositoryError::NotFound) => {}
                Err(error) => return Err(CompanionGrowthJobAdmissionError::Run(error)),
            }
        }
        let owner = SoulOwner::for_conversation(
            character_id,
            conversation.id,
            config.share_soul_growth_across_chats,
        );
        let soul = self.soul_or_default(owner, config, created_at)?;
        let run = self
            .sources
            .admit_companion_growth_run(CompanionGrowthRun {
                job_id: admitted.job.id,
                conversation_id: conversation.id,
                character_id,
                memory_run_id: result.dispatch.run.id,
                memory_attempt_id: result.dispatch.attempt.id,
                profile: self.growth_profile(&result.dispatch.run.profile)?,
                companion_name,
                authored_soul: config.soul.clone(),
                soul,
                fresh_memories: fresh_memories
                    .iter()
                    .map(|memory| GrowthMemoryEvidence {
                        id: memory.id.to_string(),
                        text: memory.text.clone(),
                    })
                    .collect(),
                operation_id: OperationRecordId::from_uuid(Uuid::new_v5(
                    &admitted.job.id.as_uuid(),
                    b"companion-growth-soul-apply",
                )),
                created_at,
                proposal_checkpoint: None,
                soul_conversation_id: owner.conversation_id(),
            })
            .map_err(CompanionGrowthJobAdmissionError::Run)?;
        Ok(CompanionGrowthJobAdmission {
            run,
            job: admitted.job,
            created: admitted.created,
        })
    }

    /// The Soul the conversation grows, created from the companion settings
    /// when it has none yet; a conversation's own Soul starts from the shared
    /// one when that exists.
    fn soul_or_default(
        &self,
        owner: SoulOwner,
        config: &lettuce_companions::CompanionSoulConfig,
        now: lettuce_types::TimestampMillis,
    ) -> Result<lettuce_companions::SoulState, CompanionGrowthJobAdmissionError> {
        if let Some(soul) = SoulRepository::get(self.sources, owner)
            .map_err(CompanionGrowthJobAdmissionError::Soul)?
        {
            return Ok(soul);
        }
        let shared = match owner {
            SoulOwner::Character(_) => None,
            SoulOwner::Conversation { character_id, .. } => {
                SoulRepository::get(self.sources, SoulOwner::Character(character_id))
                    .map_err(CompanionGrowthJobAdmissionError::Soul)?
            }
        };
        let initial = match shared {
            Some(shared) => lettuce_companions::SoulState {
                revision: lettuce_types::Revision::INITIAL,
                facts: shared.facts,
            },
            None => lettuce_companions::initial_soul_state(Some(config), now).map_err(|error| {
                CompanionGrowthJobAdmissionError::Soul(SoulRepositoryError::Invalid(error))
            })?,
        };
        match SoulRepository::create(self.sources, owner, initial, now) {
            Ok(soul) => Ok(soul),
            Err(SoulRepositoryError::AlreadyExists) => SoulRepository::get(self.sources, owner)
                .map_err(CompanionGrowthJobAdmissionError::Soul)?
                .ok_or(CompanionGrowthJobAdmissionError::Soul(
                    SoulRepositoryError::NotFound,
                )),
            Err(error) => Err(CompanionGrowthJobAdmissionError::Soul(error)),
        }
    }

    /// Growth runs on the summarisation model with its companion memory slot
    /// and defaults, not the memory cycle's sampling.
    fn growth_profile(
        &self,
        memory: &lettuce_conversations::ResolvedInferenceProfile,
    ) -> Result<lettuce_conversations::ResolvedInferenceProfile, CompanionGrowthJobAdmissionError>
    {
        let model_id = memory.chat_profile.model_profile_id;
        let model = lettuce_models::ModelProfileRepository::get(self.sources, model_id)
            .map_err(|_| CompanionGrowthJobAdmissionError::Model)?
            .ok_or(CompanionGrowthJobAdmissionError::Model)?;
        let account =
            lettuce_models::ProviderAccountRepository::get(self.sources, model.provider_account_id)
                .map_err(|_| CompanionGrowthJobAdmissionError::Model)?
                .ok_or(CompanionGrowthJobAdmissionError::Model)?;
        let global =
            lettuce_models::GlobalModelSettingsRepository::global_model_settings(self.sources)
                .map_err(|_| CompanionGrowthJobAdmissionError::Model)?
                .0;
        let chat_profile = lettuce_models::resolve_chat_profile(
            &lettuce_models::ExpectedModelIdentity {
                model_profile_id: model.id,
                model_revision: model.revision,
                provider_account_id: account.id,
                provider_account_revision: account.revision,
                external_model_id: model.external_model_id.clone(),
                display_name: model.display_name.clone(),
                provider_protocol: account.protocol,
                model_kind: model.kind,
            },
            &model,
            &account,
            &crate::feature_parameter_input(
                &model.config.feature_parameters.companion_memory,
                crate::COMPANION_MEMORY_DEFAULTS,
                crate::FeatureRequestFields::Sampling,
                account.protocol,
                &global,
            ),
            &lettuce_models::ChatRequirements::default(),
        )
        .map_err(|_| CompanionGrowthJobAdmissionError::Model)?;
        Ok(lettuce_conversations::ResolvedInferenceProfile {
            chat_profile,
            ..memory.clone()
        })
    }
}
