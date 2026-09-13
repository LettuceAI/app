use std::collections::BTreeMap;

use lettuce_conversations::{
    BranchStatus, Conversation, ConversationAggregate, ConversationBranch, ConversationKind,
    ConversationLifecycle, ConversationParticipant, GenerationAttempt, GenerationAttemptStatus,
    GenerationInput, GenerationOperation, GenerationTarget, GenerationTurn, GenerationTurnStatus,
    IdempotencyKey, InferenceUsage, InitialMessageOrigin, Message, MessageCandidate, MessagePart,
    MessageRenderSource, MessageRevision, MessageRole, MessageVisibility, ModelSelectionSnapshot,
    ParticipantRole, SnapshotSelection, UsageCounters, UsageOutcome, UsageRecord,
    UsageUnavailableReason,
};
use lettuce_transfer::{
    BackupConversation, BackupMessage, LEGACY_ID_NAMESPACE, LegacyBackupDirectMessage,
    LegacyBackupDirectSession, LegacyConversationRecord,
    LegacyDirectConversationMaterializationRequest, LegacyImportAdmission, LegacyImportAssignment,
    LegacyImportPlan, LegacyImportRepository, LegacyImportRepositoryError,
    LegacyImportStageReceipt,
};
use lettuce_types::{
    CharacterId, ConversationBranchId, ConversationId, ConversationParticipantId,
    GenerationAttemptId, GenerationTurnId, MessageCandidateId, MessageId, MessageRevisionId,
    ModelProfileId, PersonaId, ProviderAccountId, Revision, SceneId, TimestampMillis, UsageEventId,
};
use lettuce_usage::UsageEvent;
use uuid::Uuid;

use crate::{
    ConversationLaunchPlanner, DIRECT_LAUNCH_REQUEST_FORMAT_V1, DirectConversationLaunchRequest,
    DirectLaunchSources, DirectUserParticipant, LaunchSelection,
};

type Error = LegacyImportRepositoryError;

#[derive(Debug)]
pub struct LegacyDirectConversationImportCoordinator<'a, S> {
    sources: &'a S,
}

struct ImportContext {
    models: BTreeMap<ModelProfileId, (ModelProfileId, ProviderAccountId)>,
    personas: BTreeMap<PersonaId, PersonaId>,
}

impl<'a, S> LegacyDirectConversationImportCoordinator<'a, S>
where
    S: DirectLaunchSources + LegacyImportRepository,
{
    #[must_use]
    pub const fn new(sources: &'a S) -> Self {
        Self { sources }
    }

    /// Converts each legacy direct session into a finished conversation: launch
    /// snapshots come from the imported character, persona and models, and each
    /// assistant variant becomes a candidate of its own historical turn.
    pub fn execute(
        &self,
        admission: &LegacyImportAdmission,
        plan: &LegacyImportPlan,
        sessions: &[LegacyBackupDirectSession],
        completed_at: TimestampMillis,
    ) -> Result<LegacyImportStageReceipt, Error> {
        let plan_fingerprint = crate::legacy_import::plan_fingerprint(plan);
        if plan_fingerprint != admission.plan_fingerprint {
            return Err(Error::Conflict);
        }
        let source_fingerprint = plan.source_fingerprint.clone().ok_or(Error::InvalidInput)?;
        let context = import_context(admission, plan);
        let conversations = sessions
            .iter()
            .map(|session| self.map_session(session, &context))
            .collect::<Result<Vec<_>, _>>()?;
        self.sources.materialize_direct_conversations(
            LegacyDirectConversationMaterializationRequest {
                run_id: admission.run_id,
                plan_fingerprint,
                source_fingerprint,
                conversations,
                completed_at,
            },
        )
    }

    pub fn execute_database_import(
        &self,
        admission: &LegacyImportAdmission,
        import: &crate::LegacyDatabaseImportPlan,
        completed_at: TimestampMillis,
    ) -> Result<LegacyImportStageReceipt, Error> {
        self.execute(
            admission,
            &import.plan,
            &import.compatibility.direct_sessions().sessions,
            completed_at,
        )
    }

    fn map_session(
        &self,
        session: &LegacyBackupDirectSession,
        context: &ImportContext,
    ) -> Result<LegacyConversationRecord, Error> {
        let mut messages = session.messages.iter().collect::<Vec<_>>();
        messages.sort_by_key(|message| message.ordinal);
        let opens_with_scene = messages
            .first()
            .is_some_and(|message| message.role == "scene" && !message.content.trim().is_empty());
        let persona = if session.persona_disabled {
            LaunchSelection::Disabled
        } else {
            match &session.persona_source_id {
                Some(id) => LaunchSelection::Explicit(
                    context
                        .personas
                        .get(&parse(id)?)
                        .copied()
                        .ok_or(Error::InvalidInput)?,
                ),
                None => LaunchSelection::Inherit,
            }
        };
        let scene = match (&session.selected_scene_source_id, opens_with_scene) {
            (Some(id), true) => LaunchSelection::Explicit(parse::<SceneId>(id)?),
            _ => LaunchSelection::Disabled,
        };
        let request = DirectConversationLaunchRequest {
            format_version: DIRECT_LAUNCH_REQUEST_FORMAT_V1,
            title: session.title.clone(),
            user: DirectUserParticipant {
                display_name: "User".to_owned(),
                authored_description: None,
            },
            character_id: parse::<CharacterId>(&session.character_source_id)?,
            scene,
            starter: LaunchSelection::Disabled,
            persona,
            operation_key: IdempotencyKey::new(format!("legacy-import.{}", session.source_id))
                .map_err(|_| Error::InvalidInput)?,
        };
        let (plan, snapshots) = ConversationLaunchPlanner::new(self.sources)
            .prepare_direct(&request)
            .map_err(|_| Error::Conflict)?
            .into_parts();
        let conversation_id = ConversationId::from_uuid(legacy_uuid(&session.source_id));
        let branch_id = ConversationBranchId::from_uuid(derived(&session.source_id, "branch"));
        let created_at = timestamp(session.created_at)?;
        let updated_at = timestamp(session.updated_at)?.max(created_at);
        let participant = |role| {
            plan.participants
                .iter()
                .find(|participant| participant.role == role)
                .map(|participant| participant.id)
                .ok_or(Error::InvalidInput)
        };
        let user = participant(ParticipantRole::User)?;
        let character = participant(ParticipantRole::Character)?;
        let scene_origin =
            plan.initial_timeline
                .entries
                .iter()
                .find_map(|entry| match &entry.origin {
                    origin @ InitialMessageOrigin::SelectedScene { .. } => Some(origin.clone()),
                    InitialMessageOrigin::StarterMessage { .. } => None,
                });
        let ConversationKind::Direct(details) = &plan.kind else {
            return Err(Error::InvalidInput);
        };
        let model = match &details.model {
            SnapshotSelection::Inherited(model) | SnapshotSelection::Explicit(model) => {
                Some(model.clone())
            }
            _ => None,
        };
        let mut writer = SessionWriter {
            conversation_id,
            branch_id,
            user,
            character,
            model,
            context,
            messages: Vec::new(),
            turns: Vec::new(),
            usage: Vec::new(),
        };
        let mut parent: Option<(MessageId, MessageRole)> = None;
        for (index, legacy) in messages.iter().enumerate() {
            let origin = (index == 0 && opens_with_scene)
                .then(|| scene_origin.clone())
                .flatten();
            let next = writer.push(legacy, parent, origin)?;
            parent = Some(next);
        }
        let head_message_id = parent.map(|(id, _)| id);
        let conversation = Conversation {
            id: conversation_id,
            lifecycle: if session.archived {
                ConversationLifecycle::Archived
            } else {
                ConversationLifecycle::Active
            },
            title: plan.title.clone(),
            kind: plan.kind.clone(),
            active_branch_id: branch_id,
            participants: plan
                .participants
                .iter()
                .map(|draft| ConversationParticipant {
                    id: draft.id,
                    role: draft.role,
                    ordinal: draft.ordinal,
                    enabled: draft.enabled,
                    muted: draft.muted,
                    source: draft.source,
                    display_name: draft.display_name.clone(),
                    authored_description: draft.authored_description.clone(),
                    model_selection: draft.model_selection.clone(),
                    revision: Revision::INITIAL,
                    created_at,
                    updated_at: created_at,
                })
                .collect(),
            current_settings: None,
            revision: Revision::INITIAL,
            created_at,
            updated_at,
        };
        let branch = ConversationBranch {
            id: branch_id,
            conversation_id,
            parent_branch_id: None,
            fork_message_id: None,
            head_message_id,
            status: BranchStatus::Active,
            revision: Revision::INITIAL,
            created_at,
            updated_at,
        };
        Ok(LegacyConversationRecord {
            history: BackupConversation {
                aggregate: ConversationAggregate {
                    conversation,
                    branches: vec![branch],
                },
                messages: writer.messages,
            },
            turns: writer.turns,
            usage: writer.usage,
            snapshots,
        })
    }
}

struct SessionWriter<'a> {
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
    user: ConversationParticipantId,
    character: ConversationParticipantId,
    model: Option<ModelSelectionSnapshot>,
    context: &'a ImportContext,
    messages: Vec<BackupMessage>,
    turns: Vec<GenerationTurn>,
    usage: Vec<UsageEvent>,
}

impl SessionWriter<'_> {
    fn push(
        &mut self,
        legacy: &LegacyBackupDirectMessage,
        parent: Option<(MessageId, MessageRole)>,
        origin: Option<InitialMessageOrigin>,
    ) -> Result<(MessageId, MessageRole), Error> {
        let message_id = MessageId::from_uuid(legacy_uuid(&legacy.source_id));
        let created_at = timestamp(legacy.created_at)?;
        let effective_time = legacy
            .effective_at
            .map(timestamp)
            .transpose()?
            .unwrap_or(created_at);
        let (role, author, visibility) = match (legacy.role.as_str(), origin.is_some()) {
            ("scene", true) => (MessageRole::Scene, None, MessageVisibility::Visible),
            ("scene", false) => (MessageRole::System, None, MessageVisibility::Visible),
            ("user", _) => (
                MessageRole::User,
                Some(self.user),
                MessageVisibility::Visible,
            ),
            ("assistant", _) => (
                MessageRole::Assistant,
                Some(self.character),
                MessageVisibility::Visible,
            ),
            ("system", _) => (
                MessageRole::System,
                None,
                if legacy.visible_in_chat {
                    MessageVisibility::Visible
                } else {
                    MessageVisibility::Hidden
                },
            ),
            _ => return Err(Error::InvalidInput),
        };
        let mut revisions = Vec::new();
        let mut candidates = Vec::new();
        let variants_as_candidates = role == MessageRole::Assistant
            && !legacy.variants.is_empty()
            && parent.is_some()
            && self.model.is_some();
        let active_render_source = if variants_as_candidates {
            self.push_candidates(legacy, message_id, parent, &mut candidates)?
        } else if legacy.variants.is_empty() {
            let revision = MessageRevision {
                id: MessageRevisionId::from_uuid(derived(&legacy.source_id, "revision")),
                message_id,
                sequence: Revision::INITIAL,
                parts: parts(&legacy.content, legacy.reasoning.as_deref()),
                authored_at: created_at,
                source_turn_id: None,
                provider_replay: None,
            };
            let active = MessageRenderSource::Revision(revision.id);
            revisions.push(revision);
            active
        } else {
            let mut active = None;
            for (index, variant) in legacy.variants.iter().enumerate() {
                let revision = MessageRevision {
                    id: MessageRevisionId::from_uuid(derived(&variant.source_id, "revision")),
                    message_id,
                    sequence: Revision::new(
                        u64::try_from(index + 1).map_err(|_| Error::InvalidInput)?,
                    ),
                    parts: parts(&variant.content, variant.reasoning.as_deref()),
                    authored_at: timestamp(variant.created_at)?,
                    source_turn_id: None,
                    provider_replay: None,
                };
                if legacy.selected_variant_source_id.as_deref() == Some(variant.source_id.as_str())
                    || (active.is_none() && index + 1 == legacy.variants.len())
                {
                    active = Some(revision.id);
                }
                revisions.push(revision);
            }
            MessageRenderSource::Revision(active.ok_or(Error::InvalidInput)?)
        };
        let timeline_ordinal =
            u64::try_from(self.messages.len() + 1).map_err(|_| Error::InvalidInput)?;
        self.messages.push(BackupMessage {
            message: Message {
                id: message_id,
                conversation_id: self.conversation_id,
                branch_id: self.branch_id,
                parent_message_id: parent.map(|(id, _)| id),
                author_participant_id: author,
                role,
                logical_time: created_at,
                effective_time,
                visibility,
                pinned: legacy.pinned,
                scene_edited: legacy.scene_edited && role == MessageRole::Scene,
                active_render_source,
                revision: Revision::INITIAL,
                created_at,
                updated_at: created_at,
            },
            timeline_ordinal,
            initial_origin: origin,
            revisions,
            candidates,
        });
        Ok((message_id, role))
    }

    fn push_candidates(
        &mut self,
        legacy: &LegacyBackupDirectMessage,
        message_id: MessageId,
        parent: Option<(MessageId, MessageRole)>,
        candidates: &mut Vec<MessageCandidate>,
    ) -> Result<MessageRenderSource, Error> {
        let (parent_id, parent_role) = parent.ok_or(Error::InvalidInput)?;
        let model = self.model.clone().ok_or(Error::InvalidInput)?;
        let (usage_model, usage_provider, usage_model_revision, usage_provider_revision) = legacy
            .model_source_id
            .as_deref()
            .and_then(|id| id.parse::<ModelProfileId>().ok())
            .and_then(|id| self.context.models.get(&id))
            .map_or(
                (
                    model.source_id,
                    model.provider_account_id,
                    model.source_revision,
                    model.provider_account_revision,
                ),
                |(model_id, provider_id)| {
                    (
                        *model_id,
                        *provider_id,
                        Revision::INITIAL,
                        Revision::INITIAL,
                    )
                },
            );
        let mut previous: Option<MessageCandidateId> = None;
        let mut active = None;
        for (index, variant) in legacy.variants.iter().enumerate() {
            let candidate_id = MessageCandidateId::from_uuid(legacy_uuid(&variant.source_id));
            let turn_id = GenerationTurnId::from_uuid(derived(&variant.source_id, "turn"));
            let attempt_id = GenerationAttemptId::from_uuid(derived(&variant.source_id, "attempt"));
            let usage_event_id = UsageEventId::from_uuid(derived(&variant.source_id, "usage"));
            let at = timestamp(variant.created_at)?;
            let (operation, input, target) = match previous {
                None if parent_role == MessageRole::User => (
                    GenerationOperation::Send,
                    GenerationInput::UserMessage {
                        message_id: parent_id,
                    },
                    GenerationTarget::NewAssistant {
                        message_id,
                        parent_message_id: Some(parent_id),
                    },
                ),
                None => (
                    GenerationOperation::Continue,
                    GenerationInput::ExistingHead {
                        head_message_id: parent_id,
                    },
                    GenerationTarget::NewAssistant {
                        message_id,
                        parent_message_id: Some(parent_id),
                    },
                ),
                Some(prior) => (
                    GenerationOperation::Regenerate,
                    GenerationInput::ExistingCandidate {
                        message_id,
                        candidate_id: prior,
                    },
                    GenerationTarget::ExistingCandidate {
                        message_id,
                        prior_candidate_id: prior,
                    },
                ),
            };
            self.turns.push(GenerationTurn {
                id: turn_id,
                conversation_id: self.conversation_id,
                branch_id: self.branch_id,
                operation,
                input,
                target,
                swap_roles: false,
                retry_of_turn_id: None,
                idempotency_key: IdempotencyKey::new(format!("legacy-import.turn.{turn_id}"))
                    .map_err(|_| Error::InvalidInput)?,
                correlation_id: None,
                status: GenerationTurnStatus::Succeeded,
                selected_speaker: None,
                guidance: None,
                requested_model_override: None,
                forced_speaker: None,
                resolved_model: Some(model.clone()),
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
                    .map_err(|_| Error::InvalidInput)?,
                    job_id: None,
                    started_at: Some(at),
                    finished_at: Some(at),
                    candidate_ids: vec![candidate_id],
                    usage_event_id: Some(usage_event_id),
                    failure: None,
                }],
                failure: None,
                revision: Revision::INITIAL,
                created_at: at,
                updated_at: at,
            });
            let counters = match (variant.usage.prompt_tokens, variant.usage.completion_tokens) {
                (Some(input_tokens), Some(output_tokens)) => UsageCounters::Known(InferenceUsage {
                    provider_reported_cost: None,
                    cache_write_tokens: None,
                    web_search_requests: None,
                    cached_input_tokens: None,
                    reasoning_tokens: None,
                    input_tokens,
                    output_tokens,
                }),
                _ => UsageCounters::Unavailable(UsageUnavailableReason::ProviderOmitted),
            };
            self.usage.push(UsageEvent {
                id: usage_event_id,
                record: UsageRecord {
                    turn_id,
                    attempt_id,
                    outcome: UsageOutcome::Succeeded,
                    usage: counters,
                    model_profile_id: Some(usage_model),
                    model_revision: Some(usage_model_revision),
                    provider_account_id: Some(usage_provider),
                    provider_account_revision: Some(usage_provider_revision),
                    recorded_at: at,
                },
            });
            candidates.push(MessageCandidate {
                id: candidate_id,
                message_id,
                turn_id,
                attempt_id,
                author_participant_id: self.character,
                ordinal: u16::try_from(index).map_err(|_| Error::InvalidInput)?,
                parts: parts(&variant.content, variant.reasoning.as_deref()),
                model: model.clone(),
                created_at: at,
                provider_replay: None,
            });
            if legacy.selected_variant_source_id.as_deref() == Some(variant.source_id.as_str()) {
                active = Some(candidate_id);
            }
            previous = Some(candidate_id);
        }
        Ok(MessageRenderSource::Candidate(
            active.or(previous).ok_or(Error::InvalidInput)?,
        ))
    }
}

fn import_context(admission: &LegacyImportAdmission, plan: &LegacyImportPlan) -> ImportContext {
    let mut providers = BTreeMap::new();
    let mut models = BTreeMap::new();
    let mut personas = BTreeMap::new();
    for assignment in &admission.assignments {
        match assignment {
            LegacyImportAssignment::ProviderAccount {
                legacy_id,
                destination_id,
                ..
            } => {
                providers.insert(*legacy_id, *destination_id);
            }
            LegacyImportAssignment::ModelProfile {
                legacy_id,
                destination_id,
            } => {
                models.insert(*legacy_id, *destination_id);
            }
            LegacyImportAssignment::Persona {
                legacy_id,
                destination_id,
            } => {
                personas.insert(*legacy_id, *destination_id);
            }
            _ => {}
        }
    }
    let models = plan
        .provider_models
        .model_profiles
        .iter()
        .filter_map(|model| {
            Some((
                model.id,
                (
                    *models.get(&model.id)?,
                    *providers.get(&model.provider_account_id)?,
                ),
            ))
        })
        .collect();
    ImportContext { models, personas }
}

fn legacy_uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).unwrap_or_else(|_| Uuid::new_v5(&LEGACY_ID_NAMESPACE, value.as_bytes()))
}

fn derived(value: &str, suffix: &str) -> Uuid {
    Uuid::new_v5(&LEGACY_ID_NAMESPACE, format!("{value}:{suffix}").as_bytes())
}

fn parse<T: std::str::FromStr>(value: &str) -> Result<T, Error> {
    value.parse().map_err(|_| Error::InvalidInput)
}

fn timestamp(value: u64) -> Result<TimestampMillis, Error> {
    i64::try_from(value)
        .map(TimestampMillis::new)
        .map_err(|_| Error::InvalidInput)
}

fn parts(content: &str, reasoning: Option<&str>) -> Vec<MessagePart> {
    let mut parts = Vec::new();
    if let Some(reasoning) = reasoning.filter(|value| !value.trim().is_empty()) {
        parts.push(MessagePart::ReasoningSummary {
            text: reasoning.to_owned(),
        });
    }
    if !content.is_empty() {
        parts.push(MessagePart::Text {
            text: content.to_owned(),
        });
    }
    parts
}
