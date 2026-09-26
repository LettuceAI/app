use lettuce_characters::{CharacterRepository, RepositoryError};
use lettuce_companions::{
    CompanionGrowthRun, CompanionGrowthRunRepository, CompanionGrowthRunRepositoryError,
    GrowthMemoryEvidence, MAX_GROWTH_MEMORIES, SoulOwner, SoulRepository, SoulRepositoryError,
};
use lettuce_conversations::{ConversationKind, ConversationReader, ConversationRepositoryError};
use lettuce_jobs::{
    CancellationPolicy, IdempotencyKey, JobKind, JobPriority, JobSnapshot, JobSpec, JobStore,
    JobSubject, OutcomeRef, RecoveryPolicy, ResourceClass, StoreError, SubjectKind,
};
use lettuce_memory::DynamicMemoryAttemptStatus;
use lettuce_types::OperationRecordId;
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
    #[error("companion growth source is invalid")]
    InvalidSource,
    #[error("companion growth conversation read failed: {0}")]
    Conversation(#[from] ConversationRepositoryError),
    #[error("companion growth character read failed: {0}")]
    Character(#[from] RepositoryError),
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
            return Err(CompanionGrowthJobAdmissionError::InvalidSource);
        }
        let conversation =
            ConversationReader::get(self.sources, result.dispatch.run.conversation_id)?;
        let ConversationKind::Direct(details) = &conversation.conversation.kind else {
            return Ok(None);
        };
        let character_id = details.character.source_id;
        let character = CharacterRepository::get(self.sources, character_id)?
            .ok_or(CompanionGrowthJobAdmissionError::InvalidSource)?;
        let clock = crate::companion::companion_clock::companion_clock_context(
            self.sources,
            &conversation.conversation,
        )
        .map_err(|_| CompanionGrowthJobAdmissionError::InvalidSource)?;
        if !clock.companion {
            return Ok(None);
        }
        let config = character
            .character
            .defaults
            .companion_soul
            .unwrap_or_default();
        let idempotency_key =
            IdempotencyKey::new(format!("companion-growth-{}", result.dispatch.run.id))
                .map_err(|_| CompanionGrowthJobAdmissionError::InvalidSource)?;
        let subject = JobSubject::new(
            SubjectKind::Conversation,
            conversation.conversation.id.to_string(),
        )
        .map_err(|_| CompanionGrowthJobAdmissionError::InvalidSource)?;
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
                    return Ok(Some(CompanionGrowthJobAdmission {
                        run,
                        job: admitted.job,
                        created: false,
                    }));
                }
                Err(CompanionGrowthRunRepositoryError::NotFound) => {}
                Err(error) => return Err(CompanionGrowthJobAdmissionError::Run(error)),
            }
        }
        let owner = SoulOwner::for_conversation(
            character_id,
            conversation.conversation.id,
            config.share_soul_growth_across_chats,
        );
        let created_at = clock.effective_now(
            result
                .dispatch
                .attempt
                .finished_at
                .ok_or(CompanionGrowthJobAdmissionError::InvalidSource)?,
        );
        let soul = self.soul_or_default(owner, &config, created_at)?;
        let run = self
            .sources
            .admit_companion_growth_run(CompanionGrowthRun {
                job_id: admitted.job.id,
                conversation_id: conversation.conversation.id,
                character_id,
                memory_run_id: result.dispatch.run.id,
                memory_attempt_id: result.dispatch.attempt.id,
                profile: self.growth_profile(&result.dispatch.run.profile)?,
                companion_name: character.character.profile.name,
                authored_soul: config.soul,
                soul,
                fresh_memories: result
                    .fresh_memories
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
        Ok(Some(CompanionGrowthJobAdmission {
            run,
            job: admitted.job,
            created: admitted.created,
        }))
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
