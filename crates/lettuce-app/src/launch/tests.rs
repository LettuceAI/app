use std::time::Duration;

use lettuce_characters::{
    Character, CharacterDefaults, CharacterMedia, CharacterPresentationV1, CharacterProfile,
    CharacterProvenance, CharacterRepository, ChatMode, ConversationStarter, CreateCharacterPlan,
    CreateGroupPlan, GroupMember, GroupProfile, GroupRepository, GroupStartingScene,
    InteractionMode, LifecycleStatus, MemoryPolicy, Persona, PersonaRepository, Scene,
    SceneDocumentV1, SceneOwner, ScenePart, SceneVariant, Selection, StarterMessage, StarterRole,
};
use lettuce_companions::{
    CompanionConsolidationRunRepository, CompanionConversationSender, CompanionGrowthRunRepository,
    CompanionScheduledNoteRepository, CompanionSoulWriterRunRepository, CompanionStateOwner,
    CompanionStateReplacement, CompanionStateRepository, CompanionTurnEffectRepository,
    CompanionTurnEffectStatus, CompanionTurnInput, EmotionClassification, EmotionLabelScore,
    PreparedCompanionSend, SoulRepository, apply_turn,
};
use lettuce_context::{
    BindingInsertionTarget, CharacterLorebookBindingRepository, DetectionPolicy,
    GroupLorebookBindingRepository, LorebookBehaviorVersion, LorebookBindingCreate,
    LorebookMetadataDraft, LorebookRepository, PersonaLorebookBindingRepository,
    PromptBehaviorVersion, PromptMetadataDraft, PromptPurpose, PromptRepository,
};
use lettuce_conversations::{
    AttachAttemptJob, ContextAssembler, ContextRequest, ContinueConversation, ConversationCreator,
    ConversationKind, ConversationReader, ConversationRepository, CreateConversationPlan,
    DirectConversationDetails, FinalizationDraft, GenerationCheckpointEnvelope,
    GenerationCheckpointEvent, GenerationInput, GenerationTurnStatus, GroupChatModeSnapshot,
    GroupConversationDetails, IdempotencyKey, InferenceCandidate, InferenceOutcome, InferencePort,
    InferenceRequest, InferenceUsage, InitialMessageOrigin, MemoryModeSnapshot, MessageDraft,
    MessagePart, MessageRole, MessageVisibility, OperationToken, OutputPolicy, ParticipantRole,
    PortError, PromptPurposeSnapshot, ProposedToolCall, ProviderContextPart,
    ResolvedInferenceProfile, SafetyContext, SendConversation, SnapshotSelection, SnapshotSource,
    ToolExecutionRepository, ToolExecutionStatus, ToolExecutionTransition, ToolPolicy,
};
use lettuce_database::Database;
use lettuce_embeddings::{EmbeddingRequest, EmbeddingVector};
use lettuce_jobs::{
    AttemptNo, CancellationPolicy, CancellationReason, Claim, ClaimRef, InMemoryJobStore, JobKind,
    JobOutcome, JobState, LeaseId, OutcomeRef, RecoveryPolicy, ResourceAvailability, ResourceClass,
    WorkerId, handle::CancellationToken, handle::JobHandle,
};
use lettuce_memory::{
    DynamicMemoryAttemptStatus, DynamicMemoryPreparationRepository, DynamicMemoryRoundFinishReason,
    DynamicMemoryRunRepository, DynamicMemorySourceMessage, MemoryCategory, MemoryChangeSet,
    MemoryItem, MemoryPolicy as DynamicMemoryPolicy, MemoryRepository, MemorySpaceSnapshot,
    MemorySummary, MemorySummaryChange, MemorySummaryRepository, NewDynamicMemoryInferenceRound,
    NewDynamicMemoryRunAttempt, NewDynamicMemoryToolCall, Score, dynamic_memory_tool_request,
};
use lettuce_models::{
    ModelKind, ModelProfile, ModelProfileConfig, ModelProfileRepository, ProviderAccount,
    ProviderAccountRepository, ProviderConfig, ProviderProtocol,
};
use lettuce_settings::{GlobalSettingsStore, SecretOwnerId};
use lettuce_types::{
    CharacterId, ContentHash, ConversationStarterId, DynamicMemoryAttemptId, DynamicMemoryRunId,
    GroupId, JobId, LorebookId, MemoryId, MemorySpaceId, MessageId, ModelProfileId,
    OperationRecordId, PageLimit, PageRequest, PersonaId, ProviderAccountId, RequestId, Revision,
    SceneId, SceneVariantId, StarterMessageId, TimestampMillis, ToolExecutionId, UsageEventId,
};

use super::planner::ConversationLaunchPlanner;
use super::policy;
use super::request::{
    DirectConversationLaunchRequest, DirectUserParticipant, GroupConversationLaunchRequest,
    LaunchSelection,
};
use crate::{
    AppBackend, BuiltInPromptId, BuiltInPromptService, CompanionEmotionEngine,
    CompanionEmotionGenerationError, CompanionTurnCoordinator, ConversationLaunchError,
};

const NOW: TimestampMillis = TimestampMillis::new(1_000);

struct ScenarioEmbeddingEngine;

impl crate::MemoryEmbeddingEngine for ScenarioEmbeddingEngine {
    fn source_revision(&self) -> &str {
        "scenario-v1"
    }

    fn count_tokens(&self, text: &str) -> Result<u32, crate::EmbeddingGenerationError> {
        u32::try_from(text.split_whitespace().count())
            .map_err(|_| crate::EmbeddingGenerationError::Unavailable)
    }

    fn embed_memory(
        &self,
        request: &EmbeddingRequest,
        cancellation: &CancellationToken,
    ) -> Result<EmbeddingVector, crate::EmbeddingGenerationError> {
        if cancellation.is_cancelled() {
            return Err(crate::EmbeddingGenerationError::Cancelled);
        }
        let mut values = vec![0.0; request.dimensions.get()];
        values[0] = 1.0;
        Ok(EmbeddingVector {
            source_revision: self.source_revision().into(),
            values,
        })
    }
}

struct ScriptedInference {
    outcomes: Mutex<VecDeque<InferenceOutcome>>,
    requests: Mutex<Vec<InferenceRequest>>,
}

struct FallibleScriptedInference {
    outcomes: Mutex<VecDeque<Result<InferenceOutcome, PortError>>>,
    requests: Mutex<Vec<InferenceRequest>>,
}

struct UnavailableInference;

#[async_trait::async_trait]
impl InferencePort for UnavailableInference {
    async fn run(&self, _request: InferenceRequest) -> Result<InferenceOutcome, PortError> {
        Err(PortError::Unavailable)
    }
}

#[async_trait::async_trait]
impl InferencePort for ScriptedInference {
    async fn run(&self, request: InferenceRequest) -> Result<InferenceOutcome, PortError> {
        self.requests.lock().expect("requests").push(request);
        self.outcomes
            .lock()
            .expect("outcomes")
            .pop_front()
            .ok_or(PortError::Empty)
    }
}

#[async_trait::async_trait]
impl InferencePort for FallibleScriptedInference {
    async fn run(&self, request: InferenceRequest) -> Result<InferenceOutcome, PortError> {
        self.requests.lock().expect("requests").push(request);
        self.outcomes
            .lock()
            .expect("outcomes")
            .pop_front()
            .unwrap_or(Err(PortError::Empty))
    }
}

fn database() -> Database {
    Database::open_in_memory().expect("open database")
}

fn database_with_builtins() -> Database {
    let database = database();
    BuiltInPromptService::new(&database)
        .expect("prompt service")
        .bootstrap(TimestampMillis::new(1))
        .expect("bootstrap prompts");
    database
}

fn key(value: &str) -> IdempotencyKey {
    IdempotencyKey::new(value).expect("idempotency key")
}

fn request(character_id: CharacterId, operation_key: &str) -> DirectConversationLaunchRequest {
    DirectConversationLaunchRequest {
        format_version: 1,
        title: "Session".into(),
        user: DirectUserParticipant {
            display_name: "Traveller".into(),
            authored_description: None,
        },
        character_id,
        scene: LaunchSelection::Inherit,
        starter: LaunchSelection::Inherit,
        persona: LaunchSelection::Inherit,
        operation_key: key(operation_key),
    }
}

fn direct_send_command(
    conversation: &lettuce_conversations::Conversation,
    operation_key: &str,
    text: &str,
) -> SendConversation {
    let user_id = conversation
        .participants
        .iter()
        .find(|participant| participant.role == ParticipantRole::User)
        .expect("user")
        .id;
    SendConversation {
        conversation_id: conversation.id,
        branch_id: conversation.active_branch_id,
        expected_revision: conversation.revision,
        operation: OperationToken {
            key: key(operation_key),
            request_digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
        },
        message: MessageDraft {
            role: MessageRole::User,
            author_participant_id: Some(user_id),
            parts: vec![MessagePart::Text { text: text.into() }],
            visibility: MessageVisibility::Visible,
            pinned: false,
            scene_edited: false,
        },
        swap_roles: false,
    }
}

fn context_request_for(
    database: &Database,
    conversation_id: lettuce_types::ConversationId,
    source_message_id: lettuce_types::MessageId,
) -> ContextRequest {
    let aggregate = ConversationReader::get(database, conversation_id).expect("conversation");
    let branch_id = aggregate.conversation.active_branch_id;
    ContextRequest {
        conversation_id,
        branch_id,
        branch_path: vec![branch_id],
        source_message_id,
        operation: lettuce_conversations::GenerationOperation::Send,
        swap_roles: false,
        guidance: None,
        window: lettuce_conversations::ContextWindowPolicy::default(),
        selected_speaker: None,
        capabilities: lettuce_models::ModelCapabilities::default(),
        safety: SafetyContext::Standard,
        prompt_runtime: lettuce_conversations::PromptRuntimeFacts::default(),
        prompt_values: lettuce_conversations::PromptRuntimeValues::default(),
        memory: None,
        timeline: ConversationReader::timeline_page(
            database,
            conversation_id,
            branch_id,
            &lettuce_types::PageRequest::default(),
        )
        .expect("timeline")
        .items,
    }
}

fn finalize_started_turn(
    database: &Database,
    started: &lettuce_conversations::BeginGeneration,
    model: lettuce_conversations::ModelSelectionSnapshot,
    key_prefix: &str,
    now: i64,
) -> lettuce_conversations::GenerationFinalizationResult {
    let mut turn = started.turn.clone();
    for (sequence, status) in [
        GenerationTurnStatus::Preparing,
        GenerationTurnStatus::ContextPrepared,
        GenerationTurnStatus::Running,
    ]
    .into_iter()
    .enumerate()
    {
        turn = database
            .append_event(
                turn.id,
                turn.revision,
                &OperationToken {
                    key: key(&format!("{key_prefix}-stage-{sequence}")),
                    request_digest: ContentHash::parse("ed".repeat(32)).expect("digest"),
                },
                GenerationCheckpointEnvelope {
                    turn_id: turn.id,
                    attempt_id: started.attempt.id,
                    job_id: None,
                    correlation_id: None,
                    sequence: u64::try_from(sequence + 1).expect("sequence"),
                    event: GenerationCheckpointEvent::Stage { status },
                },
                TimestampMillis::new(now + i64::try_from(sequence).expect("time")),
            )
            .expect("advance turn")
            .value;
    }
    let conversation = ConversationReader::get(database, started.conversation.id)
        .expect("current conversation")
        .conversation;
    database
        .finalize_generation(
            turn.id,
            started.attempt.id,
            conversation.revision,
            turn.revision,
            &OperationToken {
                key: key(&format!("{key_prefix}-finalize")),
                request_digest: ContentHash::parse("ed".repeat(32)).expect("digest"),
            },
            FinalizationDraft {
                parts: vec![MessagePart::Text {
                    text: "Continued.".into(),
                }],
                ordinal: 0,
                model,
                replay: None,
                outcome: GenerationCheckpointEvent::Completed,
            },
            UsageEventId::new(),
            TimestampMillis::new(now + 10),
        )
        .expect("finalize turn")
}

fn request_with_starter(
    character_id: CharacterId,
    operation_key: &str,
    starter_id: ConversationStarterId,
) -> DirectConversationLaunchRequest {
    DirectConversationLaunchRequest {
        starter: LaunchSelection::Explicit(starter_id),
        ..request(character_id, operation_key)
    }
}

fn scene_with(character_id: CharacterId, ordinal: u32, parts: Vec<ScenePart>) -> Scene {
    Scene::new(
        SceneId::new(),
        SceneOwner::Character(character_id),
        ordinal,
        SceneDocumentV1::new(parts).expect("scene document"),
        TimestampMillis::new(1),
    )
    .expect("scene")
}

fn text_scene(character_id: CharacterId, ordinal: u32, text: &str) -> Scene {
    scene_with(
        character_id,
        ordinal,
        vec![ScenePart::Text { text: text.into() }],
    )
}

fn starter_with(
    character_id: CharacterId,
    ordinal: u32,
    name: &str,
    messages: Vec<StarterMessage>,
) -> ConversationStarter {
    ConversationStarter::new(
        ConversationStarterId::new(),
        character_id,
        name.into(),
        ordinal,
        messages,
        TimestampMillis::new(1),
    )
    .expect("starter")
}

fn message(role: StarterRole, content: &str) -> StarterMessage {
    StarterMessage {
        id: StarterMessageId::new(),
        role,
        content: content.into(),
    }
}

fn character_with(
    id: CharacterId,
    nickname: Option<&str>,
    defaults: CharacterDefaults,
) -> Character {
    Character::new(
        id,
        CharacterProfile {
            name: "Ada".into(),
            nickname: nickname.map(str::to_owned),
            description: Some("A meticulous engineer".into()),
            definition: None,
            design_description: None,
        },
        CharacterProvenance::default(),
        defaults,
        CharacterPresentationV1::default(),
        None,
        CharacterMedia::default(),
        TimestampMillis::new(1),
    )
    .expect("character")
}

fn seed_character(
    database: &Database,
    scenes: Vec<Scene>,
    variants: Vec<SceneVariant>,
    starters: Vec<ConversationStarter>,
    mutate: impl FnOnce(&mut CharacterDefaults),
) -> CharacterId {
    let id = CharacterId::new();
    let scenes: Vec<Scene> = scenes
        .into_iter()
        .map(|mut scene| {
            scene.owner = SceneOwner::Character(id);
            scene
        })
        .collect();
    let starters: Vec<ConversationStarter> = starters
        .into_iter()
        .map(|mut starter| {
            starter.character_id = id;
            starter
        })
        .collect();
    let mut defaults = CharacterDefaults::default();
    mutate(&mut defaults);
    let character = character_with(id, Some("Addy"), defaults);
    CharacterRepository::create(
        database,
        CreateCharacterPlan {
            character,
            scenes,
            variants,
            starters,
        },
    )
    .expect("create character");
    id
}

fn plain_character(database: &Database) -> CharacterId {
    seed_character(database, Vec::new(), Vec::new(), Vec::new(), |_| {})
}

fn seed_lorebook(database: &Database, name: &str) -> LorebookId {
    LorebookRepository::create(
        database,
        LorebookMetadataDraft {
            name: name.into(),
            detection_policy: DetectionPolicy::RecentMessageWindow,
            icon_asset_id: None,
            behavior_version: LorebookBehaviorVersion::LegacyV1,
        },
        Vec::new(),
        TimestampMillis::new(1),
    )
    .expect("lorebook")
    .book
    .id
}

fn seed_prompt(
    database: &Database,
    name: &str,
    purpose: PromptPurpose,
) -> lettuce_types::PromptDocumentId {
    PromptRepository::create_user_draft(
        database,
        PromptMetadataDraft {
            name: name.into(),
            purpose,
            condense: false,
            behavior_version: PromptBehaviorVersion::LegacyV1,
        },
        Vec::new(),
        TimestampMillis::new(1),
    )
    .expect("prompt")
    .id
}

fn seed_model(database: &Database, protocol: ProviderProtocol, kind_label: &str) -> ModelProfileId {
    seed_model_with(database, protocol, kind_label, ModelKind::Chat, true)
}

fn seed_model_with(
    database: &Database,
    protocol: ProviderProtocol,
    kind_label: &str,
    kind: ModelKind,
    enabled: bool,
) -> ModelProfileId {
    let account = ProviderAccountRepository::upsert(
        database,
        ProviderAccount {
            id: ProviderAccountId::new(),
            secret_owner_id: SecretOwnerId::new(),
            provider_kind: kind_label.into(),
            protocol,
            label: "Account".into(),
            endpoint: None,
            enabled,
            streaming_enabled: true,
            allow_invalid_tls: false,
            api_key_ref: None,
            secret_headers: Vec::new(),
            config: ProviderConfig::Standard,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        },
        None,
    )
    .expect("account");
    ModelProfileRepository::upsert(
        database,
        ModelProfile {
            id: ModelProfileId::new(),
            provider_account_id: account.id,
            external_model_id: "vendor/model".into(),
            display_name: "Vendor Model".into(),
            kind,
            config: ModelProfileConfig {
                chat_parameters: lettuce_models::ChatParameterProfile {
                    temperature: Some(0.7),
                    context_length: Some(8192),
                    max_output_tokens: Some(1024),
                    ..Default::default()
                },
                capabilities: lettuce_models::ModelCapabilities {
                    input_modalities: lettuce_models::ModalityCapabilities {
                        text: lettuce_models::CapabilityStatus::Supported,
                        ..Default::default()
                    },
                    output_modalities: lettuce_models::ModalityCapabilities {
                        text: lettuce_models::CapabilityStatus::Supported,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            },
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        },
        None,
    )
    .expect("profile")
    .id
}

fn set_application_default_model(database: &Database, id: ModelProfileId) {
    let stored = GlobalSettingsStore::load(database).expect("settings");
    GlobalSettingsStore::save(database, stored.settings, Some(id), stored.revision)
        .expect("save settings");
}

fn seed_persona(database: &Database, title: &str) -> PersonaId {
    let persona = Persona::new(
        PersonaId::new(),
        title.into(),
        "A wandering cartographer".into(),
        TimestampMillis::new(1),
    )
    .expect("persona");
    PersonaRepository::create(database, persona)
        .expect("create persona")
        .id
}

fn plan_for(
    database: &Database,
    request: &DirectConversationLaunchRequest,
) -> CreateConversationPlan {
    ConversationLaunchPlanner::new(database)
        .prepare_direct(request)
        .expect("prepare launch")
        .into_parts()
        .0
}

fn direct_details(plan: &CreateConversationPlan) -> &DirectConversationDetails {
    match &plan.kind {
        ConversationKind::Direct(details) => details,
        ConversationKind::Group(_) => panic!("direct launch produced a group conversation"),
    }
}

fn message_text(part: &MessagePart) -> &str {
    match part {
        MessagePart::Text { text } => text,
        _ => panic!("expected a text part"),
    }
}

fn lorebook_names(
    selection: &SnapshotSelection<Vec<lettuce_conversations::LorebookLaunchSnapshot>>,
) -> Option<Vec<String>> {
    match selection {
        SnapshotSelection::Inherited(books) | SnapshotSelection::Explicit(books) => {
            Some(books.iter().map(|book| book.name.clone()).collect())
        }
        SnapshotSelection::Disabled => None,
    }
}

#[test]
fn happy_path_direct_launch_has_two_participants_and_an_empty_timeline() {
    let database = database();
    let character_id = plain_character(&database);
    let result = ConversationLaunchPlanner::new(&database)
        .launch_direct(&request(character_id, "happy-path"), NOW)
        .expect("launch");
    result.value.validate().expect("aggregate validates");
    assert_eq!(result.value.conversation.participants.len(), 2);
    assert!(
        ConversationReader::timeline_page(
            &database,
            result.value.conversation.id,
            result.value.conversation.active_branch_id,
            &lettuce_types::PageRequest::default(),
        )
        .expect("timeline")
        .items
        .is_empty()
    );
    let plan = plan_for(&database, &request(character_id, "happy-path"));
    let details = direct_details(&plan);
    assert_eq!(details.scene, SnapshotSelection::Disabled);
    assert_eq!(details.starter, SnapshotSelection::Disabled);
    assert_eq!(details.voice, SnapshotSelection::Disabled);
}

#[test]
fn character_participant_uses_nickname_and_mirrors_the_model_selection() {
    let database = database();
    let model_id = seed_model(&database, ProviderProtocol::Anthropic, "anthropic");
    let character_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.model_profile_id = Some(model_id);
    });
    let plan = plan_for(&database, &request(character_id, "participant-shape"));
    let details = direct_details(&plan);
    let participant = plan
        .participants
        .iter()
        .find(|participant| participant.role == ParticipantRole::Character)
        .expect("character participant");
    assert_eq!(participant.display_name, "Addy");
    assert_eq!(participant.model_selection, details.model);
}

#[test]
fn archived_character_is_rejected() {
    let database = database();
    let character_id = plain_character(&database);
    CharacterRepository::archive(&database, character_id, Revision::INITIAL, NOW).expect("archive");
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&request(character_id, "archived"))
            .expect_err("archived character"),
        ConversationLaunchError::CharacterArchived { character_id }
    );
}

#[test]
fn companion_character_launch_seeds_normalized_runtime_state() {
    let database = database_with_builtins();
    let persona_id = seed_persona(&database, "Mira");
    let character_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.interaction_mode = InteractionMode::Companion;
        let mut config = lettuce_companions::CompanionSoulConfig::default();
        config.soul.baseline_affect.warmth = 0.8;
        config.relationship_defaults.trust = 0.6;
        defaults.companion_soul = Some(config);
    });
    let mut initial_request = request(character_id, "companion");
    initial_request.persona = LaunchSelection::Explicit(persona_id);
    let launched = ConversationLaunchPlanner::new(&database)
        .launch_direct(&initial_request, NOW)
        .expect("launch companion");
    let retried = ConversationLaunchPlanner::new(&database)
        .launch_direct(&initial_request, NOW)
        .expect("retry companion launch");
    assert_eq!(
        retried.value.conversation.id,
        launched.value.conversation.id
    );
    let state = CompanionStateRepository::get(
        &database,
        CompanionStateOwner {
            conversation_id: launched.value.conversation.id,
            character_id,
            persona_id: Some(persona_id),
        },
    )
    .expect("load state")
    .expect("companion state");
    assert_eq!(state.state.relationship_state.trust, 0.6);
    assert_eq!(state.state.emotional_state.felt.warmth, 0.8);

    let mut evolved = state.state.clone();
    evolved.relationship_state.trust = 0.9;
    evolved.emotional_state.felt.warmth = 0.1;
    CompanionStateRepository::replace(
        &database,
        state.owner,
        OperationRecordId::new(),
        CompanionStateReplacement {
            expected_session_revision: state.session_revision,
            expected_relationship_revision: state.relationship_revision,
            state: evolved,
            applied_at: TimestampMillis::new(NOW.get() + 1),
        },
    )
    .expect("evolve companion state");

    let mut next_request = request(character_id, "companion-next");
    next_request.persona = LaunchSelection::Explicit(persona_id);
    let next = ConversationLaunchPlanner::new(&database)
        .launch_direct(&next_request, TimestampMillis::new(NOW.get() + 2))
        .expect("launch next companion session");
    let next_state = CompanionStateRepository::get(
        &database,
        CompanionStateOwner {
            conversation_id: next.value.conversation.id,
            character_id,
            persona_id: Some(persona_id),
        },
    )
    .expect("load next state")
    .expect("next companion state");
    assert_eq!(next_state.state.relationship_state.trust, 0.9);
    assert_eq!(next_state.state.emotional_state.felt.warmth, 0.8);
}

#[test]
fn companion_send_commits_user_turn_and_state_once() {
    let database = database_with_builtins();
    let character_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.interaction_mode = InteractionMode::Companion;
        defaults.companion_soul = Some(lettuce_companions::CompanionSoulConfig::default());
    });
    let launched = ConversationLaunchPlanner::new(&database)
        .launch_direct(&request(character_id, "companion-send-launch"), NOW)
        .expect("launch companion");
    let conversation = &launched.value.conversation;
    let owner = CompanionStateOwner {
        conversation_id: conversation.id,
        character_id,
        persona_id: None,
    };
    let state = CompanionStateRepository::get(&database, owner)
        .expect("load state")
        .expect("state");
    let config = lettuce_companions::CompanionSoulConfig::default();
    let transition = apply_turn(
        &state.state,
        &config.soul.baseline_affect,
        &config.soul.regulation_style,
        &config.relationship_defaults,
        &CompanionTurnInput {
            signals: vec!["emotion:love".into()],
            emotion_delta: lettuce_companions::EmotionVector {
                warmth: 0.1,
                ..Default::default()
            },
            relationship_delta: lettuce_companions::RelationshipDelta {
                affection: 0.05,
                ..Default::default()
            },
            confidence: 0.8,
            now: TimestampMillis::new(NOW.get() + 10),
        },
    );
    let user_id = conversation
        .participants
        .iter()
        .find(|participant| participant.role == ParticipantRole::User)
        .expect("user")
        .id;
    let command = SendConversation {
        conversation_id: conversation.id,
        branch_id: conversation.active_branch_id,
        expected_revision: conversation.revision,
        operation: OperationToken {
            key: key("companion-send"),
            request_digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
        },
        message: MessageDraft {
            role: MessageRole::User,
            author_participant_id: Some(user_id),
            parts: vec![MessagePart::Text {
                text: "I love spending time with you.".into(),
            }],
            visibility: MessageVisibility::Visible,
            pinned: false,
            scene_edited: false,
        },
        swap_roles: false,
    };
    let replacement = CompanionStateReplacement {
        expected_session_revision: state.session_revision,
        expected_relationship_revision: state.relationship_revision,
        state: transition.current,
        applied_at: TimestampMillis::new(NOW.get() + 10),
    };
    let prepare = || {
        PreparedCompanionSend::new(
            command.clone(),
            owner,
            replacement.clone(),
            Some(lettuce_companions::CompanionTurnEffectSeed::default()),
        )
        .expect("prepared send")
    };
    let sent = CompanionConversationSender::begin_companion_send(
        &database,
        prepare(),
        TimestampMillis::new(NOW.get() + 10),
    )
    .expect("send");
    let replay = CompanionConversationSender::begin_companion_send(
        &database,
        prepare(),
        TimestampMillis::new(NOW.get() + 99),
    )
    .expect("replay");
    assert_eq!(replay.operation, sent.operation);
    assert_eq!(replay.value.turn.id, sent.value.turn.id);

    let stored = CompanionStateRepository::get(&database, owner)
        .expect("reload state")
        .expect("stored state");
    assert_eq!(
        stored.session_revision,
        state
            .session_revision
            .next()
            .expect("next session revision")
    );
    assert_eq!(
        stored.relationship_revision,
        state
            .relationship_revision
            .next()
            .expect("next relationship revision")
    );
    assert_eq!(stored.state.active_signals, ["emotion:love"]);
    assert_eq!(stored.state.relationship_state.interaction_count, 1);
}

#[tokio::test]
async fn companion_effect_appears_once_with_the_finalized_assistant_message() {
    let database = database_with_builtins();
    let model_id = seed_model(&database, ProviderProtocol::Ollama, "companion-effect");
    set_application_default_model(&database, model_id);
    let character_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.interaction_mode = InteractionMode::Companion;
        defaults.memory_policy = MemoryPolicy::Dynamic;
        defaults.companion_soul = Some(lettuce_companions::CompanionSoulConfig::default());
    });
    let launched = ConversationLaunchPlanner::new(&database)
        .launch_direct(&request(character_id, "companion-effect-launch"), NOW)
        .expect("launch companion");
    let memory_space =
        MemoryRepository::get_for_conversation(&database, launched.value.conversation.id)
            .expect("conversation memory space")
            .expect("dynamic memory space");
    assert_eq!(memory_space.revision, Revision::INITIAL);
    assert!(memory_space.items.is_empty());
    let sent = CompanionTurnCoordinator::<_, ScenarioEmotionEngine>::new(&database, None)
        .begin_send(
            &direct_send_command(
                &launched.value.conversation,
                "companion-effect-send",
                "I missed you.",
            ),
            TimestampMillis::new(NOW.get() + 1),
            &CancellationToken::new(),
        )
        .expect("send companion message");
    let turn_id = sent.value.turn.id;
    let attempt_id = sent.value.attempt.id;
    let user_message_id = match &sent.value.turn.input {
        GenerationInput::UserMessage { message_id } => *message_id,
        _ => panic!("expected user input"),
    };
    let mut turn = sent.value.turn;
    let operation = |value: &str| OperationToken {
        key: key(value),
        request_digest: ContentHash::parse("ef".repeat(32)).expect("digest"),
    };
    for (sequence, status) in [
        GenerationTurnStatus::Preparing,
        GenerationTurnStatus::ContextPrepared,
        GenerationTurnStatus::Running,
    ]
    .into_iter()
    .enumerate()
    {
        turn = database
            .append_event(
                turn_id,
                turn.revision,
                &operation(&format!("companion-effect-stage-{sequence}")),
                GenerationCheckpointEnvelope {
                    turn_id,
                    attempt_id,
                    job_id: None,
                    correlation_id: None,
                    sequence: u64::try_from(sequence + 1).expect("sequence"),
                    event: GenerationCheckpointEvent::Stage { status },
                },
                TimestampMillis::new(NOW.get() + 2 + i64::try_from(sequence).expect("time")),
            )
            .expect("advance companion turn")
            .value;
    }
    let conversation = ConversationReader::get(&database, launched.value.conversation.id)
        .expect("current conversation")
        .conversation;
    let ConversationKind::Direct(details) = &conversation.kind else {
        panic!("expected direct conversation");
    };
    let model = match &details.model {
        SnapshotSelection::Inherited(model) | SnapshotSelection::Explicit(model) => model.clone(),
        SnapshotSelection::Disabled => panic!("expected resolved model"),
    };
    let finalize_operation = operation("companion-effect-finalize");
    let finalization_draft = FinalizationDraft {
        parts: vec![MessagePart::Text {
            text: "I missed you too.".into(),
        }],
        ordinal: 0,
        model: model.clone(),
        replay: None,
        outcome: GenerationCheckpointEvent::Completed,
    };
    let usage_event_id = UsageEventId::new();
    let finalized = database
        .finalize_generation(
            turn_id,
            attempt_id,
            conversation.revision,
            turn.revision,
            &finalize_operation,
            finalization_draft.clone(),
            usage_event_id,
            TimestampMillis::new(NOW.get() + 10),
        )
        .expect("finalize assistant");
    let effect = CompanionTurnEffectRepository::get_for_message(
        &database,
        conversation.id,
        finalized.value.assistant_message.id,
    )
    .expect("load effect")
    .expect("processing effect");
    assert_eq!(effect.status, CompanionTurnEffectStatus::Processing);
    assert_eq!(effect.user_message_id, Some(user_message_id));
    assert_eq!(
        CompanionTurnEffectRepository::list_processing(&database, 512)
            .expect("list processing effects")
            .iter()
            .map(|effect| effect.id)
            .collect::<Vec<_>>(),
        [effect.id]
    );
    let replay = database
        .finalize_generation(
            turn_id,
            attempt_id,
            conversation.revision,
            turn.revision,
            &finalize_operation,
            finalization_draft,
            usage_event_id,
            TimestampMillis::new(NOW.get() + 99),
        )
        .expect("replay finalization");
    assert_eq!(replay.operation, finalized.operation);
    assert_eq!(
        CompanionTurnEffectRepository::get_for_message(
            &database,
            conversation.id,
            finalized.value.assistant_message.id,
        )
        .expect("reload effect")
        .expect("same effect")
        .id,
        effect.id
    );

    let mut stored_profile = ModelProfileRepository::get(&database, model.source_id)
        .expect("profile")
        .expect("profile exists");
    stored_profile.config.chat_parameters.temperature = None;
    let account = ProviderAccountRepository::get(&database, model.provider_account_id)
        .expect("account")
        .expect("account exists");
    let profile = ResolvedInferenceProfile {
        chat_profile: lettuce_models::resolve_chat_profile(
            &model.expected_chat_identity(),
            &stored_profile,
            &account,
            &lettuce_models::ChatParameterResolutionInput::default(),
            &lettuce_models::ChatRequirements::default(),
        )
        .expect("resolve profile"),
        tool_policy: ToolPolicy::Required,
        output_policy: OutputPolicy::Plain,
        safety_policy: SafetyContext::Standard,
        correlation_id: None,
    };
    let prompt_ids = BuiltInPromptService::new(&database)
        .expect("prompt service")
        .bootstrap(TimestampMillis::new(NOW.get() + 11))
        .expect("prompt ids");
    let prompt = PromptRepository::get(&database, prompt_ids.get(BuiltInPromptId::DynamicMemory))
        .expect("prompt")
        .expect("dynamic memory prompt");
    let summary_prompt =
        PromptRepository::get(&database, prompt_ids.get(BuiltInPromptId::DynamicSummary))
            .expect("prompt")
            .expect("dynamic summary prompt");
    let soul_writer_prompt = PromptRepository::get(
        &database,
        prompt_ids.get(BuiltInPromptId::CompanionSoulWriter),
    )
    .expect("prompt")
    .expect("Soul-writer prompt");
    let soul_writer_request_id = RequestId::new();
    let current_soul = serde_json::json!({
        "soul": { "traits": "Careful", "baselineAffect": { "warmth": 2.0 } }
    });
    let soul_writer = crate::CompanionSoulWriterAdmissionCoordinator::new(&database, &database)
        .admit(crate::CompanionSoulWriterAdmissionRequest {
            request_id: soul_writer_request_id,
            primary_profile: profile.clone(),
            fallback_profile: Some(profile.clone()),
            prompt: &soul_writer_prompt,
            character_name: "Mira",
            character_definition: Some("A careful traveller"),
            character_description: None,
            opening_context: Some("At the station"),
            current_soul: Some(&current_soul),
            user_notes: None,
            fallback_format: lettuce_companions::SoulWriterFallbackFormat::Json,
            now: TimestampMillis::new(NOW.get() + 12),
        })
        .expect("admit Soul-writer preview");
    assert!(soul_writer.created);
    assert_eq!(soul_writer.job.kind, JobKind::CompanionSoulWriter);
    assert_eq!(soul_writer.run.job_id, soul_writer.job.id);
    assert_eq!(soul_writer.run.prompt_id, soul_writer_prompt.id);
    assert_eq!(soul_writer.run.prompt_revision, soul_writer_prompt.revision);
    assert_eq!(soul_writer.run.primary_profile, profile);
    assert_eq!(soul_writer.run.fallback_profile, Some(profile.clone()));
    assert_eq!(soul_writer.run.starting_draft["soul"]["traits"], "Careful");
    assert_eq!(
        soul_writer.run.starting_draft["soul"]["baselineAffect"]["warmth"],
        1.0
    );
    assert!(soul_writer.run.rounds.is_empty());
    let soul_writer_replay =
        crate::CompanionSoulWriterAdmissionCoordinator::new(&database, &database)
            .admit(crate::CompanionSoulWriterAdmissionRequest {
                request_id: soul_writer_request_id,
                primary_profile: profile.clone(),
                fallback_profile: None,
                prompt: &summary_prompt,
                character_name: "changed",
                character_definition: None,
                character_description: None,
                opening_context: None,
                current_soul: None,
                user_notes: None,
                fallback_format: lettuce_companions::SoulWriterFallbackFormat::Xml,
                now: TimestampMillis::new(NOW.get() + 13),
            })
            .expect("replay Soul-writer preview");
    assert!(!soul_writer_replay.created);
    assert_eq!(soul_writer_replay.run, soul_writer.run);
    let soul_inference = ScriptedInference {
        outcomes: Mutex::new(VecDeque::from([
            InferenceOutcome {
                candidates: vec![InferenceCandidate {
                    ordinal: 0,
                    parts: Vec::new(),
                    tool_calls: vec![ProposedToolCall {
                        provider_call_id: Some("soul-identity".into()),
                        name: "set_identity".into(),
                        arguments: serde_json::json!({"traits": "Patient and observant"}),
                        raw_arguments: None,
                        provider_replay: None,
                    }],
                    provider_replay: None,
                }],
                usage: None,
                finish_reason: lettuce_conversations::FinishReason::Stop,
                provider_finish_reason: None,
                provider_request_id: Some("soul-round-one".into()),
                warning_codes: Vec::new(),
            },
            InferenceOutcome {
                candidates: vec![InferenceCandidate {
                    ordinal: 0,
                    parts: Vec::new(),
                    tool_calls: vec![
                        ProposedToolCall {
                            provider_call_id: Some("soul-done".into()),
                            name: "done".into(),
                            arguments: serde_json::json!({}),
                            raw_arguments: None,
                            provider_replay: None,
                        },
                        ProposedToolCall {
                            provider_call_id: Some("soul-after-done".into()),
                            name: "set_identity".into(),
                            arguments: serde_json::json!({"traits": "ignored"}),
                            raw_arguments: None,
                            provider_replay: None,
                        },
                    ],
                    provider_replay: None,
                }],
                usage: None,
                finish_reason: lettuce_conversations::FinishReason::Stop,
                provider_finish_reason: None,
                provider_request_id: Some("soul-round-two".into()),
                warning_codes: Vec::new(),
            },
        ])),
        requests: Mutex::new(Vec::new()),
    };
    let soul_work = crate::CompanionSoulWriterDispatchCoordinator::new(&database, &database)
        .claim(
            soul_writer_request_id,
            WorkerId::new(),
            TimestampMillis::new(NOW.get() + 14),
            Duration::from_secs(60),
            &ResourceAvailability::all(),
        )
        .expect("claim Soul-writer job")
        .expect("Soul-writer work");
    let soul_result =
        crate::CompanionSoulWriterExecutionCoordinator::new(&database, &soul_inference)
            .run(
                soul_writer_request_id,
                &soul_writer_prompt,
                &soul_work.handle,
                None,
                TimestampMillis::new(NOW.get() + 14),
            )
            .await
            .expect("execute Soul-writer preview");
    assert_eq!(soul_result.rounds, 2);
    assert_eq!(soul_result.draft["soul"]["traits"], "Patient and observant");
    {
        let soul_requests = soul_inference.requests.lock().expect("Soul requests");
        assert_eq!(soul_requests.len(), 2);
        assert_eq!(soul_requests[0].profile.tool_policy, ToolPolicy::Required);
        assert_eq!(
            soul_requests[1].context.messages.len(),
            soul_requests[0].context.messages.len() + 2
        );
    }
    let settled_soul = crate::CompanionSoulWriterDispatchCoordinator::new(&database, &database)
        .settle(
            soul_work,
            Ok(soul_result.clone()),
            CancellationReason::User,
            TimestampMillis::new(NOW.get() + 15),
        )
        .expect("settle Soul-writer job");
    assert!(matches!(
        settled_soul,
        crate::CompanionSoulWriterSettledWork::Succeeded { ref job, .. }
            if job.state == JobState::Succeeded
    ));
    let soul_handle = JobHandle::new(soul_writer.job.id);
    let completed_replay =
        crate::CompanionSoulWriterExecutionCoordinator::new(&database, &UnavailableInference)
            .run(
                soul_writer_request_id,
                &soul_writer_prompt,
                &soul_handle,
                None,
                TimestampMillis::new(NOW.get() + 15),
            )
            .await
            .expect("replay completed Soul-writer preview");
    assert!(completed_replay.replayed);
    assert_eq!(completed_replay.draft, soul_result.draft);
    let fallback_request_id = RequestId::new();
    let fallback_writer = crate::CompanionSoulWriterAdmissionCoordinator::new(&database, &database)
        .admit(crate::CompanionSoulWriterAdmissionRequest {
            request_id: fallback_request_id,
            primary_profile: profile.clone(),
            fallback_profile: None,
            prompt: &soul_writer_prompt,
            character_name: "Mira",
            character_definition: Some("A careful traveller"),
            character_description: None,
            opening_context: None,
            current_soul: None,
            user_notes: None,
            fallback_format: lettuce_companions::SoulWriterFallbackFormat::Json,
            now: TimestampMillis::new(NOW.get() + 16),
        })
        .expect("admit structured fallback preview");
    let fallback_inference = ScriptedInference {
        outcomes: Mutex::new(VecDeque::from([
            InferenceOutcome {
                candidates: vec![InferenceCandidate {
                    ordinal: 0,
                    parts: vec![MessagePart::Text {
                        text: "I cannot call tools".into(),
                    }],
                    tool_calls: Vec::new(),
                    provider_replay: None,
                }],
                usage: None,
                finish_reason: lettuce_conversations::FinishReason::Stop,
                provider_finish_reason: None,
                provider_request_id: Some("soul-no-tools".into()),
                warning_codes: Vec::new(),
            },
            InferenceOutcome {
                candidates: vec![InferenceCandidate {
                    ordinal: 0,
                    parts: vec![MessagePart::Text {
                        text: r#"{"operations":[{"name":"set_identity","arguments":{"traits":"Structured"}},{"name":"done","arguments":{}}]}"#.into(),
                    }],
                    tool_calls: Vec::new(),
                    provider_replay: None,
                }],
                usage: None,
                finish_reason: lettuce_conversations::FinishReason::Stop,
                provider_finish_reason: None,
                provider_request_id: Some("soul-structured".into()),
                warning_codes: Vec::new(),
            },
        ])),
        requests: Mutex::new(Vec::new()),
    };
    let fallback_result =
        crate::CompanionSoulWriterExecutionCoordinator::new(&database, &fallback_inference)
            .run(
                fallback_request_id,
                &soul_writer_prompt,
                &JobHandle::new(fallback_writer.job.id),
                None,
                TimestampMillis::new(NOW.get() + 17),
            )
            .await
            .expect("execute structured fallback preview");
    assert_eq!(fallback_result.draft["soul"]["traits"], "Structured");
    {
        let fallback_requests = fallback_inference
            .requests
            .lock()
            .expect("fallback requests");
        assert_eq!(fallback_requests.len(), 2);
        assert_eq!(
            fallback_requests[0].profile.tool_policy,
            ToolPolicy::Required
        );
        assert_eq!(
            fallback_requests[1].profile.tool_policy,
            ToolPolicy::Disabled
        );
        assert!(fallback_requests[1].tools.is_none());
    }
    let mut alternate_profile = profile.clone();
    alternate_profile.chat_profile.model_profile_id = ModelProfileId::new();
    let alternate_request_id = RequestId::new();
    let alternate_writer =
        crate::CompanionSoulWriterAdmissionCoordinator::new(&database, &database)
            .admit(crate::CompanionSoulWriterAdmissionRequest {
                request_id: alternate_request_id,
                primary_profile: profile.clone(),
                fallback_profile: Some(alternate_profile.clone()),
                prompt: &soul_writer_prompt,
                character_name: "Mira",
                character_definition: Some("A careful traveller"),
                character_description: None,
                opening_context: None,
                current_soul: None,
                user_notes: None,
                fallback_format: lettuce_companions::SoulWriterFallbackFormat::Json,
                now: TimestampMillis::new(NOW.get() + 18),
            })
            .expect("admit alternate profile preview");
    let alternate_inference = FallibleScriptedInference {
        outcomes: Mutex::new(VecDeque::from([
            Err(PortError::Unavailable),
            Ok(InferenceOutcome {
                candidates: vec![InferenceCandidate {
                    ordinal: 0,
                    parts: Vec::new(),
                    tool_calls: vec![ProposedToolCall {
                        provider_call_id: Some("alternate-done".into()),
                        name: "done".into(),
                        arguments: serde_json::json!({}),
                        raw_arguments: None,
                        provider_replay: None,
                    }],
                    provider_replay: None,
                }],
                usage: None,
                finish_reason: lettuce_conversations::FinishReason::Stop,
                provider_finish_reason: None,
                provider_request_id: Some("alternate-profile".into()),
                warning_codes: Vec::new(),
            }),
        ])),
        requests: Mutex::new(Vec::new()),
    };
    crate::CompanionSoulWriterExecutionCoordinator::new(&database, &alternate_inference)
        .run(
            alternate_request_id,
            &soul_writer_prompt,
            &JobHandle::new(alternate_writer.job.id),
            None,
            TimestampMillis::new(NOW.get() + 19),
        )
        .await
        .expect("execute alternate profile preview");
    {
        let alternate_requests = alternate_inference
            .requests
            .lock()
            .expect("alternate requests");
        assert_eq!(alternate_requests.len(), 2);
        assert_eq!(
            alternate_requests[1].profile.chat_profile.model_profile_id,
            alternate_profile.chat_profile.model_profile_id
        );
    }
    let interrupted_request_id = RequestId::new();
    let interrupted_writer =
        crate::CompanionSoulWriterAdmissionCoordinator::new(&database, &database)
            .admit(crate::CompanionSoulWriterAdmissionRequest {
                request_id: interrupted_request_id,
                primary_profile: profile.clone(),
                fallback_profile: None,
                prompt: &soul_writer_prompt,
                character_name: "Mira",
                character_definition: None,
                character_description: None,
                opening_context: None,
                current_soul: None,
                user_notes: None,
                fallback_format: lettuce_companions::SoulWriterFallbackFormat::Json,
                now: TimestampMillis::new(NOW.get() + 20),
            })
            .expect("admit interrupted preview");
    let interrupted_inference = ScriptedInference {
        outcomes: Mutex::new(VecDeque::from([InferenceOutcome {
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts: Vec::new(),
                tool_calls: vec![ProposedToolCall {
                    provider_call_id: Some("interrupted-identity".into()),
                    name: "set_identity".into(),
                    arguments: serde_json::json!({"traits": "Durable partial"}),
                    raw_arguments: None,
                    provider_replay: None,
                }],
                provider_replay: None,
            }],
            usage: None,
            finish_reason: lettuce_conversations::FinishReason::Stop,
            provider_finish_reason: None,
            provider_request_id: Some("interrupted-first".into()),
            warning_codes: Vec::new(),
        }])),
        requests: Mutex::new(Vec::new()),
    };
    assert!(matches!(
        crate::CompanionSoulWriterExecutionCoordinator::new(&database, &interrupted_inference)
            .run(
                interrupted_request_id,
                &soul_writer_prompt,
                &JobHandle::new(interrupted_writer.job.id),
                None,
                TimestampMillis::new(NOW.get() + 21),
            )
            .await,
        Err(crate::CompanionSoulWriterExecutionError::Inference(
            PortError::Empty
        ))
    ));
    let interrupted_run = database
        .load_companion_soul_writer_run(interrupted_request_id)
        .expect("load interrupted preview");
    assert_eq!(interrupted_run.rounds.len(), 1);
    assert_eq!(
        interrupted_run.rounds[0].resulting_draft["soul"]["traits"],
        "Durable partial"
    );

    let cancelled_request_id = RequestId::new();
    let cancelled_writer =
        crate::CompanionSoulWriterAdmissionCoordinator::new(&database, &database)
            .admit(crate::CompanionSoulWriterAdmissionRequest {
                request_id: cancelled_request_id,
                primary_profile: profile.clone(),
                fallback_profile: None,
                prompt: &soul_writer_prompt,
                character_name: "Mira",
                character_definition: None,
                character_description: None,
                opening_context: None,
                current_soul: None,
                user_notes: None,
                fallback_format: lettuce_companions::SoulWriterFallbackFormat::Json,
                now: TimestampMillis::new(NOW.get() + 22),
            })
            .expect("admit cancelled preview");
    let cancelled_handle = JobHandle::new(cancelled_writer.job.id);
    cancelled_handle.request_cancel();
    assert!(matches!(
        crate::CompanionSoulWriterExecutionCoordinator::new(&database, &UnavailableInference)
            .run(
                cancelled_request_id,
                &soul_writer_prompt,
                &cancelled_handle,
                None,
                TimestampMillis::new(NOW.get() + 23),
            )
            .await,
        Err(crate::CompanionSoulWriterExecutionError::Cancelled)
    ));
    let capped_request_id = RequestId::new();
    let capped_writer = crate::CompanionSoulWriterAdmissionCoordinator::new(&database, &database)
        .admit(crate::CompanionSoulWriterAdmissionRequest {
            request_id: capped_request_id,
            primary_profile: profile.clone(),
            fallback_profile: None,
            prompt: &soul_writer_prompt,
            character_name: "Mira",
            character_definition: None,
            character_description: None,
            opening_context: None,
            current_soul: None,
            user_notes: None,
            fallback_format: lettuce_companions::SoulWriterFallbackFormat::Json,
            now: TimestampMillis::new(NOW.get() + 24),
        })
        .expect("admit capped preview");
    let capped_outcomes = (0..8)
        .map(|ordinal| InferenceOutcome {
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts: Vec::new(),
                tool_calls: vec![ProposedToolCall {
                    provider_call_id: Some(format!("capped-{ordinal}")),
                    name: "set_identity".into(),
                    arguments: serde_json::json!({"traits": format!("draft-{ordinal}")}),
                    raw_arguments: None,
                    provider_replay: None,
                }],
                provider_replay: None,
            }],
            usage: None,
            finish_reason: lettuce_conversations::FinishReason::Stop,
            provider_finish_reason: None,
            provider_request_id: Some(format!("capped-{ordinal}")),
            warning_codes: Vec::new(),
        })
        .collect::<VecDeque<_>>();
    let capped_inference = ScriptedInference {
        outcomes: Mutex::new(capped_outcomes),
        requests: Mutex::new(Vec::new()),
    };
    assert!(matches!(
        crate::CompanionSoulWriterExecutionCoordinator::new(&database, &capped_inference)
            .run(
                capped_request_id,
                &soul_writer_prompt,
                &JobHandle::new(capped_writer.job.id),
                None,
                TimestampMillis::new(NOW.get() + 25),
            )
            .await,
        Err(crate::CompanionSoulWriterExecutionError::RoundLimit)
    ));
    assert_eq!(
        database
            .load_companion_soul_writer_run(capped_request_id)
            .expect("load capped preview")
            .rounds
            .len(),
        8
    );
    let jobs = InMemoryJobStore::new();
    assert!(
        crate::CompanionMemoryDispatchCoordinator::new(&database, &jobs)
            .discover_and_claim(
                512,
                1,
                lettuce_memory::DynamicMemoryRunMode::AskFirst,
                WorkerId::new(),
                TimestampMillis::new(NOW.get() + 11),
                Duration::from_secs(60),
                &ResourceAvailability::all(),
            )
            .expect("record pending approval")
            .is_empty()
    );
    assert_eq!(
        lettuce_memory::DynamicMemoryApprovalRepository::get_dynamic_memory_pending_approval(
            &database,
            conversation.id,
        )
        .expect("pending approval")
        .expect("approval")
        .prompted_message_count,
        2
    );
    let work = crate::CompanionMemoryDispatchCoordinator::new(&database, &jobs)
        .retry_direct_with_model_and_claim(
            conversation.id,
            512,
            1,
            model.source_id,
            true,
            WorkerId::new(),
            TimestampMillis::new(NOW.get() + 11),
            Duration::from_secs(60),
            &ResourceAvailability::all(),
        )
        .expect("discover and claim job")
        .into_iter()
        .next()
        .expect("memory job");
    assert_eq!(
        work.admission.batch.selected_model_profile_id,
        Some(model.source_id)
    );
    assert!(work.admission.batch.update_dynamic_memory_model_on_success);
    assert_eq!(
        work.admission.batch.window_selection,
        crate::CompanionMemoryWindowSelection::Recent
    );
    let retry = crate::CompanionPostTurnMemoryAdmissionCoordinator::new(&database, &jobs)
        .retry_direct_with_model_and_admit(conversation.id, 512, 1, model.source_id, true)
        .expect("exact model retry")
        .expect("same admission");
    assert!(!retry.created);
    assert_eq!(
        retry.batch.idempotency_key,
        work.admission.batch.idempotency_key
    );
    assert!(
        lettuce_memory::DynamicMemoryApprovalRepository::get_dynamic_memory_pending_approval(
            &database,
            conversation.id,
        )
        .expect("cleared approval")
        .is_none()
    );
    let scripted = ScriptedInference {
        outcomes: Mutex::new(VecDeque::from([
            InferenceOutcome {
                candidates: vec![InferenceCandidate {
                    ordinal: 0,
                    parts: vec![MessagePart::Text {
                        text: "write_summary({ summary: 'invalid' })".into(),
                    }],
                    tool_calls: Vec::new(),
                    provider_replay: None,
                }],
                usage: Some(InferenceUsage {
                    input_tokens: 20,
                    output_tokens: 3,
                }),
                finish_reason: lettuce_conversations::FinishReason::Stop,
                provider_finish_reason: None,
                provider_request_id: Some("runner-summary-invalid".into()),
                warning_codes: Vec::new(),
            },
            InferenceOutcome {
                candidates: vec![InferenceCandidate {
                    ordinal: 0,
                    parts: vec![MessagePart::Text {
                        text: "The user told Mira they missed her.".into(),
                    }],
                    tool_calls: Vec::new(),
                    provider_replay: None,
                }],
                usage: Some(InferenceUsage {
                    input_tokens: 24,
                    output_tokens: 7,
                }),
                finish_reason: lettuce_conversations::FinishReason::Stop,
                provider_finish_reason: None,
                provider_request_id: Some("runner-summary-fallback".into()),
                warning_codes: Vec::new(),
            },
            InferenceOutcome {
                candidates: vec![InferenceCandidate {
                    ordinal: 0,
                    parts: Vec::new(),
                    tool_calls: vec![ProposedToolCall {
                        provider_call_id: Some("runner-create".into()),
                        name: "create_memory".into(),
                        arguments: serde_json::json!({
                            "text": "The user missed Mira",
                            "category": "relationship",
                            "source_message_id": user_message_id.to_string()
                        }),
                        raw_arguments: None,
                        provider_replay: None,
                    }],
                    provider_replay: None,
                }],
                usage: None,
                finish_reason: lettuce_conversations::FinishReason::Stop,
                provider_finish_reason: None,
                provider_request_id: Some("runner-first".into()),
                warning_codes: Vec::new(),
            },
            InferenceOutcome {
                candidates: vec![InferenceCandidate {
                    ordinal: 0,
                    parts: Vec::new(),
                    tool_calls: vec![ProposedToolCall {
                        provider_call_id: Some("runner-done".into()),
                        name: "done".into(),
                        arguments: serde_json::json!({"summary": "stored relationship"}),
                        raw_arguments: None,
                        provider_replay: None,
                    }],
                    provider_replay: None,
                }],
                usage: None,
                finish_reason: lettuce_conversations::FinishReason::Stop,
                provider_finish_reason: None,
                provider_request_id: Some("runner-second".into()),
                warning_codes: Vec::new(),
            },
        ])),
        requests: Mutex::new(Vec::new()),
    };
    let policy = DynamicMemoryPolicy {
        max_entries: 10,
        hot_token_budget: 100,
        cold_threshold: Score::from_basis_points(2_000).expect("score"),
        delete_confidence_default: Score::from_basis_points(5_000).expect("score"),
        max_hard_delete_ratio_per_cycle: Score::from_basis_points(5_000).expect("score"),
    };
    let dispatch = crate::CompanionPostTurnMemoryRunCoordinator::new(&database, &database)
        .admit_or_recover(
            &work.admission,
            profile.clone(),
            true,
            true,
            lettuce_memory::DynamicMemoryStructuredFallbackFormat::Xml,
            &work.handle,
            TimestampMillis::new(NOW.get() + 12),
        )
        .expect("admit memory run");
    let summary = crate::CompanionMemorySummaryCoordinator::new(
        &ScenarioEmbeddingEngine,
        &database,
        &database,
        &scripted,
    )
    .run(
        dispatch.run.id,
        dispatch.attempt.id,
        &summary_prompt,
        &work.handle,
        None,
        TimestampMillis::new(NOW.get() + 12),
    )
    .await
    .expect("checkpoint summary");
    assert!(!summary.replayed);
    assert!(matches!(
        crate::CompanionMemoryInferenceCoordinator::new(
            &database,
            &database,
            &UnavailableInference,
        )
        .run_first_round(
            dispatch.run.id,
            dispatch.attempt.id,
            &prompt,
            &summary.checkpoint.summary.text,
            &policy,
            &work.handle,
            None,
            TimestampMillis::new(NOW.get() + 12),
        )
        .await,
        Err(crate::CompanionMemoryInferenceError::Inference(
            PortError::Unavailable
        ))
    ));
    assert_eq!(
        MemorySummaryRepository::get_summary(&database, memory_space.id)
            .expect("summary after memory failure")
            .expect("stored summary after memory failure"),
        summary.checkpoint.summary
    );
    let memory_id = MemoryId::new();
    let runner = crate::CompanionMemoryJobRunner::new(
        &ScenarioEmbeddingEngine,
        &database,
        &database,
        &scripted,
    );
    let result = runner
        .run(
            &work.admission,
            profile.clone(),
            true,
            true,
            lettuce_memory::DynamicMemoryStructuredFallbackFormat::Xml,
            &summary_prompt,
            &prompt,
            &policy,
            Score::from_basis_points(9_000).expect("score"),
            &work.claim,
            &work.handle,
            None,
            TimestampMillis::new(NOW.get() + 12),
            |round| {
                if round.ordinal == 0 {
                    vec![crate::MemoryCreateSeed {
                        execution_id: round.calls[0].id,
                        id: memory_id,
                        token_count: 5,
                        created_at: TimestampMillis::new(NOW.get() + 12),
                    }]
                } else {
                    Vec::new()
                }
            },
        )
        .await
        .expect("run background job");
    assert_eq!(
        GlobalSettingsStore::load(&database)
            .expect("updated settings")
            .dynamic_memory_model_profile_id,
        Some(model.source_id)
    );
    assert!(!result.first_round_replayed);
    assert!(result.summary_replayed);
    assert_eq!(
        result.dispatch.attempt.status,
        DynamicMemoryAttemptStatus::Succeeded
    );
    assert_eq!(result.loop_result.completed_rounds, 2);
    assert_eq!(
        result.loop_result.summary.as_deref(),
        Some("stored relationship")
    );
    {
        let requests = scripted.requests.lock().expect("requests");
        assert_eq!(requests.len(), 4);
        assert!(requests[0].tools.is_some());
        assert_eq!(requests[0].profile.tool_policy, ToolPolicy::Required);
        assert!(requests[1].tools.is_none());
        assert_eq!(requests[1].profile.tool_policy, ToolPolicy::Disabled);
        assert!(matches!(
            requests[1].context.messages.last(),
            Some(lettuce_conversations::ProviderNeutralMessage {
                role: MessageRole::User,
                parts,
            }) if matches!(parts.as_slice(), [ProviderContextPart::Text { text }] if text == "Return only the final merged summary as plain text. No tools, no JSON, no markdown, no commentary.")
        ));
    }
    let stored_summary = MemorySummaryRepository::get_summary(&database, memory_space.id)
        .expect("summary")
        .expect("stored summary");
    assert_eq!(stored_summary.text, "The user told Mira they missed her.");
    assert_eq!(stored_summary.token_count, 7);
    assert_eq!(
        stored_summary.source_message_ids,
        [user_message_id, finalized.value.assistant_message.id]
    );
    let stored_memory = MemoryRepository::get(&database, memory_space.id)
        .expect("memory")
        .expect("memory space");
    assert_eq!(stored_memory.items[0].id, memory_id);
    assert_eq!(
        stored_memory.items[0].source_message_id,
        Some(user_message_id)
    );
    assert_eq!(stored_memory.items[0].source_role, Some(MessageRole::User));
    assert_eq!(
        stored_memory.items[0].observed_at,
        Some(TimestampMillis::new(NOW.get() + 1))
    );
    assert_eq!(
        stored_memory.items[0].observed_time_precision.as_deref(),
        Some("turn")
    );
    let terminal = crate::CompanionMemoryTerminalCoordinator::new(&database);
    let replayed = terminal
        .settle_success(
            result.dispatch.run.id,
            result.dispatch.attempt.id,
            &work.admission.batch,
            &work.handle,
            TimestampMillis::new(NOW.get() + 99),
        )
        .expect("replay terminal success");
    assert_eq!(
        replayed.attempt.status,
        DynamicMemoryAttemptStatus::Succeeded
    );
    assert_eq!(replayed.effects, result.effects);
    let mut empty_growth = result.clone();
    empty_growth.fresh_memories.clear();
    assert!(
        crate::CompanionGrowthJobAdmissionCoordinator::new(&database, &database)
            .admit_after_memory(&empty_growth)
            .expect("skip empty growth")
            .is_none()
    );
    let growth = crate::CompanionGrowthJobAdmissionCoordinator::new(&database, &database)
        .admit_after_memory(&result)
        .expect("admit growth")
        .expect("growth job");
    let growth_replay = crate::CompanionGrowthJobAdmissionCoordinator::new(&database, &database)
        .admit_after_memory(&result)
        .expect("replay growth")
        .expect("growth job replay");
    assert!(growth.created);
    assert!(!growth_replay.created);
    assert_eq!(growth.job.id, growth_replay.job.id);
    assert_eq!(growth.job.kind, JobKind::CompanionGrowth);
    assert_eq!(growth.run.memory_run_id, result.dispatch.run.id);
    assert_eq!(growth.run.memory_attempt_id, result.dispatch.attempt.id);
    assert_eq!(growth.run.profile, result.dispatch.run.profile);
    assert_eq!(growth.run.fresh_memories.len(), 1);
    assert_eq!(growth.run.fresh_memories[0].id, memory_id.to_string());
    let growth_dispatch = crate::CompanionGrowthDispatchCoordinator::new(&database, &database);
    let growth_work = growth_dispatch
        .claim(
            growth.job.id,
            WorkerId::new(),
            TimestampMillis::new(NOW.get() + 100),
            Duration::from_secs(30),
            &ResourceAvailability::all(),
        )
        .expect("claim growth job")
        .expect("growth work");
    assert_eq!(growth_work.job.state, JobState::Running);
    let mut growth_adjustments = vec![serde_json::json!({
        "category": "likes",
        "kind": "add",
        "policy": "adaptive",
        "slot": "drink",
        "value": "Prefers tea",
        "confidence": 0.8,
        "weight": 0.7,
        "sourceIndices": [0]
    })];
    growth_adjustments.extend((1..12).map(|index| {
        serde_json::json!({
            "category": "habits",
            "kind": "add",
            "policy": "adaptive",
            "slot": format!("habit-{index}"),
            "value": format!("Habit {index}"),
            "confidence": 0.8,
            "weight": 0.7,
            "sourceIndices": [0]
        })
    }));
    scripted
        .outcomes
        .lock()
        .expect("growth outcome")
        .push_back(InferenceOutcome {
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts: Vec::new(),
                tool_calls: vec![ProposedToolCall {
                    provider_call_id: Some("growth-record".into()),
                    name: "record_growth".into(),
                    arguments: serde_json::json!({"adjustments": growth_adjustments}),
                    raw_arguments: None,
                    provider_replay: None,
                }],
                provider_replay: None,
            }],
            usage: None,
            finish_reason: lettuce_conversations::FinishReason::Stop,
            provider_finish_reason: None,
            provider_request_id: Some("provider-growth".into()),
            warning_codes: Vec::new(),
        });
    let growth_prompt = PromptRepository::get(
        &database,
        prompt_ids.get(BuiltInPromptId::CompanionGrowthcycle),
    )
    .expect("growth prompt")
    .expect("growth prompt exists");
    let applied = crate::CompanionGrowthExecutionCoordinator::new(&database, &scripted)
        .run(
            growth.job.id,
            &growth_prompt,
            &growth_work.handle,
            None,
            TimestampMillis::new(NOW.get() + 101),
        )
        .await
        .expect("execute growth");
    assert_eq!(applied.applied_facts, 12);
    assert!(!applied.proposal_replayed);
    let checkpointed = database
        .load_companion_growth_run(growth.job.id)
        .expect("load checkpointed growth");
    assert_eq!(
        checkpointed
            .proposal_checkpoint
            .as_ref()
            .map(|checkpoint| checkpoint.proposals.len()),
        Some(12)
    );
    let replayed_growth =
        crate::CompanionGrowthExecutionCoordinator::new(&database, &UnavailableInference)
            .run(
                growth.job.id,
                &growth_prompt,
                &growth_work.handle,
                None,
                TimestampMillis::new(NOW.get() + 102),
            )
            .await
            .expect("replay growth without provider");
    assert_eq!(replayed_growth.receipt, applied.receipt);
    assert!(replayed_growth.proposal_replayed);
    let after_checkpoint = crate::CompanionGrowthJobAdmissionCoordinator::new(&database, &database)
        .admit_after_memory(&result)
        .expect("replay growth after checkpoint")
        .expect("growth job after checkpoint");
    assert_eq!(after_checkpoint.job.id, growth.job.id);
    assert_eq!(after_checkpoint.run, checkpointed);
    {
        let requests = scripted.requests.lock().expect("growth request");
        let growth_request = requests.last().expect("growth provider request");
        assert_eq!(growth_request.profile.tool_policy, ToolPolicy::Required);
        let growth_tools = growth_request.tools.as_ref().expect("growth tools");
        assert_eq!(growth_tools.definitions.len(), 1);
        assert_eq!(growth_tools.definitions[0].name, "record_growth");
        assert!(growth_request.context.messages.iter().any(|message| {
            message.parts.iter().any(|part| {
                matches!(part, ProviderContextPart::Text { text } if text.contains("The user missed Mira"))
            })
        }));
    }
    let stored_soul = SoulRepository::get(
        &database,
        lettuce_companions::SoulOwner::Character(growth.run.character_id),
    )
    .expect("load grown Soul")
    .expect("grown Soul");
    assert_eq!(
        stored_soul.revision,
        applied
            .receipt
            .as_ref()
            .expect("receipt")
            .resulting_revision
    );
    assert!(stored_soul.facts.iter().any(|fact| {
        fact.value == "Prefers tea" && fact.source_memory_ids.as_slice() == [memory_id.to_string()]
    }));
    let settled_growth = growth_dispatch
        .settle(
            growth_work,
            Ok(applied),
            CancellationReason::User,
            TimestampMillis::new(NOW.get() + 103),
        )
        .expect("settle growth job");
    let crate::CompanionGrowthSettledWork::Succeeded {
        result: applied_growth,
        job,
    } = settled_growth
    else {
        panic!("expected succeeded growth job");
    };
    assert_eq!(job.state, JobState::Succeeded);
    assert_eq!(
        job.outcome,
        Some(JobOutcome::Success {
            result_ref: OutcomeRef::Character(growth.run.character_id),
        })
    );
    assert!(
        crate::CompanionConsolidationJobAdmissionCoordinator::new(&database, &database)
            .admit_after_growth(
                growth.job.id,
                &crate::CompanionGrowthExecutionResult {
                    receipt: None,
                    applied_facts: 0,
                    proposal_replayed: false,
                },
            )
            .expect("skip empty consolidation")
            .is_none()
    );
    let consolidation =
        crate::CompanionConsolidationJobAdmissionCoordinator::new(&database, &database)
            .admit_after_growth(growth.job.id, &applied_growth)
            .expect("admit consolidation")
            .expect("consolidation job");
    assert!(consolidation.created);
    assert_eq!(consolidation.job.kind, JobKind::CompanionConsolidation);
    assert_eq!(consolidation.run.growth_job_id, growth.job.id);
    assert_eq!(consolidation.run.soul, stored_soul);
    let consolidation_replay =
        crate::CompanionConsolidationJobAdmissionCoordinator::new(&database, &database)
            .admit_after_growth(growth.job.id, &applied_growth)
            .expect("replay consolidation")
            .expect("consolidation replay");
    assert!(!consolidation_replay.created);
    assert_eq!(consolidation_replay.run, consolidation.run);
    let retired_fact_id = stored_soul.facts[0].id.clone();
    let consolidation_dispatch =
        crate::CompanionConsolidationDispatchCoordinator::new(&database, &database);
    let consolidation_work = consolidation_dispatch
        .claim(
            consolidation.job.id,
            WorkerId::new(),
            TimestampMillis::new(NOW.get() + 104),
            Duration::from_secs(30),
            &ResourceAvailability::all(),
        )
        .expect("claim consolidation job")
        .expect("consolidation work");
    assert_eq!(consolidation_work.job.state, JobState::Running);
    scripted
        .outcomes
        .lock()
        .expect("consolidation outcome")
        .push_back(InferenceOutcome {
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts: Vec::new(),
                tool_calls: vec![ProposedToolCall {
                    provider_call_id: Some("consolidate".into()),
                    name: "consolidate_soul".into(),
                    arguments: serde_json::json!({
                        "coreAdjustments": [{
                            "category": "traits",
                            "value": "Patient",
                            "confidence": 0.9,
                            "weight": 0.8
                        }],
                        "retire": [retired_fact_id.clone()]
                    }),
                    raw_arguments: None,
                    provider_replay: None,
                }],
                provider_replay: None,
            }],
            usage: None,
            finish_reason: lettuce_conversations::FinishReason::Stop,
            provider_finish_reason: None,
            provider_request_id: Some("provider-consolidation".into()),
            warning_codes: Vec::new(),
        });
    let consolidation_prompt = PromptRepository::get(
        &database,
        prompt_ids.get(BuiltInPromptId::CompanionConsolidation),
    )
    .expect("consolidation prompt")
    .expect("consolidation prompt exists");
    let consolidated = crate::CompanionConsolidationExecutionCoordinator::new(&database, &scripted)
        .run(
            consolidation.job.id,
            &consolidation_prompt,
            &consolidation_work.handle,
            None,
            TimestampMillis::new(NOW.get() + 105),
        )
        .await
        .expect("execute consolidation");
    assert_eq!(consolidated.applied_changes, 2);
    assert!(!consolidated.proposal_replayed);
    let checkpointed_consolidation = database
        .load_companion_consolidation_run(consolidation.job.id)
        .expect("load checkpointed consolidation");
    assert_eq!(
        checkpointed_consolidation
            .proposal_checkpoint
            .as_ref()
            .map(|checkpoint| (
                checkpoint.proposal.core_adjustments.len(),
                checkpoint.proposal.retire_ids.len()
            )),
        Some((1, 1))
    );
    let replayed_consolidation =
        crate::CompanionConsolidationExecutionCoordinator::new(&database, &UnavailableInference)
            .run(
                consolidation.job.id,
                &consolidation_prompt,
                &consolidation_work.handle,
                None,
                TimestampMillis::new(NOW.get() + 106),
            )
            .await
            .expect("replay consolidation without provider");
    assert_eq!(replayed_consolidation.receipt, consolidated.receipt);
    assert!(replayed_consolidation.proposal_replayed);
    {
        let requests = scripted.requests.lock().expect("consolidation request");
        let request = requests.last().expect("consolidation provider request");
        assert_eq!(request.profile.tool_policy, ToolPolicy::Required);
        let tools = request.tools.as_ref().expect("consolidation tools");
        assert_eq!(tools.definitions.len(), 1);
        assert_eq!(tools.definitions[0].name, "consolidate_soul");
        assert!(request.context.messages.iter().any(|message| {
            message.parts.iter().any(|part| {
                matches!(part, ProviderContextPart::Text { text } if text.contains("Habit 1"))
            })
        }));
    }
    let consolidated_soul = SoulRepository::get(
        &database,
        lettuce_companions::SoulOwner::Character(growth.run.character_id),
    )
    .expect("load consolidated Soul")
    .expect("consolidated Soul");
    assert!(consolidated_soul.facts.iter().any(|fact| {
        fact.value == "Patient" && fact.kind == lettuce_companions::SoulFactKind::Consolidated
    }));
    assert_eq!(
        consolidated_soul
            .facts
            .iter()
            .find(|fact| fact.id == retired_fact_id)
            .and_then(|fact| fact.superseded_by.as_deref()),
        Some("consolidation")
    );
    assert_eq!(
        consolidated_soul.revision,
        consolidated
            .receipt
            .as_ref()
            .expect("consolidation receipt")
            .resulting_revision
    );
    let after_consolidation =
        crate::CompanionConsolidationJobAdmissionCoordinator::new(&database, &database)
            .admit_after_growth(growth.job.id, &applied_growth)
            .expect("replay consolidation after Soul apply")
            .expect("consolidation after Soul apply");
    assert!(!after_consolidation.created);
    assert_eq!(after_consolidation.run, checkpointed_consolidation);
    assert_eq!(
        database
            .load_companion_consolidation_run_for_growth(growth.job.id)
            .expect("load consolidation by growth"),
        checkpointed_consolidation
    );
    let settled_consolidation = consolidation_dispatch
        .settle(
            consolidation_work,
            Ok(consolidated),
            CancellationReason::User,
            TimestampMillis::new(NOW.get() + 107),
        )
        .expect("settle consolidation job");
    let crate::CompanionConsolidationSettledWork::Succeeded { job, .. } = settled_consolidation
    else {
        panic!("expected succeeded consolidation job");
    };
    assert_eq!(job.state, JobState::Succeeded);
    assert_eq!(
        job.outcome,
        Some(JobOutcome::Success {
            result_ref: OutcomeRef::Character(growth.run.character_id),
        })
    );
    let settled = crate::CompanionMemoryDispatchCoordinator::new(&database, &jobs)
        .settle_run(
            work,
            Ok(result),
            CancellationReason::User,
            TimestampMillis::new(NOW.get() + 100),
        )
        .expect("settle runtime job");
    let crate::CompanionMemorySettledWork::Succeeded { result, job } = settled else {
        panic!("expected succeeded runtime job");
    };
    assert_eq!(job.state, JobState::Succeeded);
    assert_eq!(
        job.progress.fraction,
        Some(lettuce_jobs::FiniteFraction::new(1.0).expect("fraction"))
    );
    assert_eq!(
        job.outcome,
        Some(JobOutcome::Success {
            result_ref: OutcomeRef::Conversation(conversation.id),
        })
    );
    let ready = result.effects.into_iter().next().expect("settled effect");
    assert_eq!(ready.status, CompanionTurnEffectStatus::Ready);
    assert_eq!(ready.memory_changes.added, [memory_id]);
    assert_eq!(
        ready
            .source_window
            .expect("source window")
            .message_ids
            .len(),
        2
    );
    assert!(
        CompanionTurnEffectRepository::list_processing(&database, 512)
            .expect("ready effect is not pending")
            .is_empty()
    );

    let current = ConversationReader::get(&database, conversation.id)
        .expect("conversation after reply")
        .conversation;
    let ConversationKind::Direct(details) = &current.kind else {
        panic!("expected direct conversation");
    };
    let continue_model = match &details.model {
        SnapshotSelection::Inherited(model) | SnapshotSelection::Explicit(model) => model.clone(),
        SnapshotSelection::Disabled => panic!("expected resolved model"),
    };
    let continued = CompanionTurnCoordinator::<_, ScenarioEmotionEngine>::new(&database, None)
        .begin_continue(
            &ContinueConversation {
                conversation_id: current.id,
                branch_id: current.active_branch_id,
                expected_revision: current.revision,
                forced_speaker: None,
                swap_roles: false,
                operation: operation("companion-effect-continue"),
            },
            TimestampMillis::new(NOW.get() + 20),
        )
        .expect("begin companion continue");
    let continued_finalization = finalize_started_turn(
        &database,
        &continued.value,
        continue_model,
        "companion-effect-continue",
        NOW.get() + 21,
    );
    let continued_effect = CompanionTurnEffectRepository::get_for_message(
        &database,
        current.id,
        continued_finalization.value.assistant_message.id,
    )
    .expect("load continuation effect")
    .expect("continuation processing effect");
    assert_eq!(
        continued_effect.status,
        CompanionTurnEffectStatus::Processing
    );
    assert_eq!(continued_effect.user_message_id, None);
    assert_eq!(
        continued_effect.seed,
        lettuce_companions::CompanionTurnEffectSeed::default()
    );
    assert_eq!(
        CompanionTurnEffectRepository::list_processing(&database, 512)
            .expect("list continuation effect")
            .iter()
            .map(|effect| effect.id)
            .collect::<Vec<_>>(),
        [continued_effect.id]
    );
    let continued_source = ConversationReader::timeline_page(
        &database,
        current.id,
        current.active_branch_id,
        &PageRequest {
            cursor: None,
            limit: PageLimit::new(20),
        },
    )
    .expect("continuation timeline")
    .items
    .into_iter()
    .find(|item| item.message.id == continued_effect.assistant_message_id)
    .expect("continuation source");
    let failure_run_id = DynamicMemoryRunId::new();
    let failure_attempt_id = DynamicMemoryAttemptId::new();
    let failure_job_id = JobId::new();
    let failure_memory = MemoryRepository::get_for_conversation(&database, current.id)
        .expect("memory")
        .expect("memory space");
    let admitted_failure = database
        .admit_dynamic_memory_run_attempt(NewDynamicMemoryRunAttempt {
            run_id: failure_run_id,
            attempt_id: failure_attempt_id,
            conversation_id: current.id,
            space_id: failure_memory.id,
            starting_memory: failure_memory,
            source_messages: vec![DynamicMemorySourceMessage {
                message_id: continued_source.message.id,
                role: continued_source.message.role,
                render_source: continued_source.message.active_render_source,
                effective_time: continued_source.message.effective_time,
            }],
            profile,
            time_awareness_enabled: false,
            supersession_enabled: false,
            structured_fallback_format: lettuce_memory::DynamicMemoryStructuredFallbackFormat::Xml,
            summary_window: lettuce_memory::DynamicMemorySummaryWindow {
                message_interval: 1,
                start: stored_summary.window_end,
                end: stored_summary.window_end + 1,
            },
            job_id: failure_job_id,
            now: TimestampMillis::new(NOW.get() + 39),
        })
        .expect("failure run");
    database
        .transition_dynamic_memory_attempt(
            failure_attempt_id,
            admitted_failure.attempt.revision,
            DynamicMemoryAttemptStatus::Processing,
            None,
            TimestampMillis::new(NOW.get() + 39),
        )
        .expect("failure processing");
    let failed_model_id = ModelProfileId::new();
    let failure_batch = crate::CompanionPostTurnMemoryBatch {
        conversation_id: current.id,
        idempotency_key: lettuce_jobs::IdempotencyKey::new("terminal-failure-batch")
            .expect("batch key"),
        summary_message_interval: 1,
        window_selection: crate::CompanionMemoryWindowSelection::Automatic,
        unsummarized_message_count: if continued_effect.user_message_id.is_some() {
            2
        } else {
            1
        },
        source_effect_offset: 0,
        effects: vec![continued_effect],
        settle_effects: true,
        selected_model_profile_id: Some(failed_model_id),
        update_dynamic_memory_model_on_success: true,
    };
    let failure_handle = JobHandle::new(failure_job_id);
    let failed = terminal
        .settle_failure(
            failure_run_id,
            failure_attempt_id,
            &failure_batch,
            &failure_handle,
            crate::CompanionMemoryTerminalFailure::Cancelled,
            TimestampMillis::new(NOW.get() + 40),
        )
        .expect("cancel terminal effect");
    let failed_replay = terminal
        .settle_failure(
            failure_run_id,
            failure_attempt_id,
            &failure_batch,
            &failure_handle,
            crate::CompanionMemoryTerminalFailure::Cancelled,
            TimestampMillis::new(NOW.get() + 41),
        )
        .expect("replay cancelled terminal effect");
    assert_eq!(failed_replay, failed);
    assert_eq!(failed.attempt.status, DynamicMemoryAttemptStatus::Cancelled);
    let failed = failed.effects.into_iter().next().expect("failed effect");
    assert_eq!(failed.status, CompanionTurnEffectStatus::Failed);
    assert_eq!(
        failed.summary.as_deref(),
        Some("Dynamic memory was cancelled")
    );
    assert_eq!(
        GlobalSettingsStore::load(&database)
            .expect("settings after cancelled retry")
            .dynamic_memory_model_profile_id,
        Some(model.source_id)
    );
    assert!(
        CompanionTurnEffectRepository::list_processing(&database, 512)
            .expect("failed effect is not pending")
            .is_empty()
    );

    let before_delete = ConversationReader::get(&database, current.id)
        .expect("conversation before delete-after")
        .conversation;
    let delete_command = crate::DeleteAfterMessages {
        conversation_id: current.id,
        after_message_id: finalized.value.assistant_message.id,
        expected_revision: before_delete.revision,
        operation: operation("companion-effect-delete-after"),
        summary_message_interval: 1,
    };
    let deleted = crate::DynamicMemoryDeleteAfterCoordinator::new(&database, &jobs)
        .delete_after(&delete_command, TimestampMillis::new(NOW.get() + 50))
        .expect("delete after");
    assert_eq!(
        deleted
            .tombstone
            .as_ref()
            .expect("tombstone")
            .value
            .message
            .id,
        continued_finalization.value.assistant_message.id
    );
    assert_eq!(
        deleted
            .rewind
            .as_ref()
            .and_then(|rewind| rewind.invalid_run_id),
        Some(failure_run_id)
    );
    assert!(deleted.retained_effects.is_empty());
    assert_eq!(
        CompanionTurnEffectRepository::get_for_message(
            &database,
            current.id,
            continued_finalization.value.assistant_message.id,
        )
        .expect("invalidated effect")
        .expect("stored effect")
        .status,
        CompanionTurnEffectStatus::Invalidated
    );
    assert_eq!(
        MemorySummaryRepository::get_summary(&database, memory_space.id)
            .expect("restored summary")
            .expect("prior summary")
            .text,
        stored_summary.text
    );
    let replay = crate::DynamicMemoryDeleteAfterCoordinator::new(&database, &jobs)
        .delete_after(&delete_command, TimestampMillis::new(NOW.get() + 99))
        .expect("delete-after replay");
    assert_eq!(
        replay.tombstone.as_ref().map(|value| &value.operation),
        deleted.tombstone.as_ref().map(|value| &value.operation)
    );
    assert_eq!(replay.rewind, deleted.rewind);
}

struct ScenarioEmotionEngine {
    result: Result<Option<EmotionClassification>, CompanionEmotionGenerationError>,
    calls: std::sync::atomic::AtomicUsize,
}

impl ScenarioEmotionEngine {
    fn new(result: Result<Option<EmotionClassification>, CompanionEmotionGenerationError>) -> Self {
        Self {
            result,
            calls: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl CompanionEmotionEngine for ScenarioEmotionEngine {
    fn classify_emotion(
        &self,
        _: &str,
        _: &CancellationToken,
    ) -> Result<Option<EmotionClassification>, CompanionEmotionGenerationError> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.result.clone()
    }
}

#[test]
fn companion_turn_coordinator_classifies_once_and_replays_without_state_drift() {
    let database = database_with_builtins();
    let character_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.interaction_mode = InteractionMode::Companion;
        defaults.companion_soul = Some(lettuce_companions::CompanionSoulConfig::default());
    });
    let launched = ConversationLaunchPlanner::new(&database)
        .launch_direct(&request(character_id, "coordinator-launch"), NOW)
        .expect("launch");
    let command = direct_send_command(
        &launched.value.conversation,
        "coordinator-send",
        "I love spending time with you.",
    );
    let engine = ScenarioEmotionEngine::new(Ok(Some(EmotionClassification {
        labels: vec![EmotionLabelScore {
            label: "love".into(),
            score: 1.0,
        }],
        confidence: 1.0,
    })));
    let coordinator = CompanionTurnCoordinator::new(&database, Some(&engine));
    let first = coordinator
        .begin_send(
            &command,
            TimestampMillis::new(NOW.get() + 10),
            &CancellationToken::new(),
        )
        .expect("send");
    let replay = coordinator
        .begin_send(
            &command,
            TimestampMillis::new(NOW.get() + 99),
            &CancellationToken::new(),
        )
        .expect("replay");
    assert_eq!(engine.calls(), 1);
    assert_eq!(replay.operation, first.operation);
    let mut changed = command.clone();
    changed.operation.request_digest = ContentHash::parse("cd".repeat(32)).expect("digest");
    assert!(matches!(
        coordinator.begin_send(
            &changed,
            TimestampMillis::new(NOW.get() + 100),
            &CancellationToken::new(),
        ),
        Err(crate::CompanionTurnError::Conversation(
            lettuce_conversations::ConversationRepositoryError::Conflict
        ))
    ));
    assert_eq!(engine.calls(), 1);

    let state = CompanionStateRepository::get(
        &database,
        CompanionStateOwner {
            conversation_id: launched.value.conversation.id,
            character_id,
            persona_id: None,
        },
    )
    .expect("state")
    .expect("companion state");
    assert_eq!(state.state.active_signals, ["emotion:love"]);
    assert_eq!(state.state.relationship_state.interaction_count, 1);
}

#[test]
fn companion_turn_coordinator_uses_neutral_fallback_and_bypasses_roleplay() {
    let database = database_with_builtins();
    let companion_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.interaction_mode = InteractionMode::Companion;
        defaults.companion_soul = Some(lettuce_companions::CompanionSoulConfig::default());
    });
    let companion = ConversationLaunchPlanner::new(&database)
        .launch_direct(&request(companion_id, "fallback-launch"), NOW)
        .expect("companion launch");
    let unavailable = ScenarioEmotionEngine::new(Err(CompanionEmotionGenerationError::Unavailable));
    CompanionTurnCoordinator::new(&database, Some(&unavailable))
        .begin_send(
            &direct_send_command(
                &companion.value.conversation,
                "fallback-send",
                "ordinary message",
            ),
            TimestampMillis::new(NOW.get() + 10),
            &CancellationToken::new(),
        )
        .expect("fallback send");
    let state = CompanionStateRepository::get(
        &database,
        CompanionStateOwner {
            conversation_id: companion.value.conversation.id,
            character_id: companion_id,
            persona_id: None,
        },
    )
    .expect("state")
    .expect("companion state");
    assert_eq!(state.state.emotional_state.confidence, 0.2);
    assert!(state.state.active_signals.is_empty());

    let cancelled_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.interaction_mode = InteractionMode::Companion;
        defaults.companion_soul = Some(lettuce_companions::CompanionSoulConfig::default());
    });
    let cancelled = ConversationLaunchPlanner::new(&database)
        .launch_direct(&request(cancelled_id, "cancelled-launch"), NOW)
        .expect("cancelled launch");
    let cancelled_engine =
        ScenarioEmotionEngine::new(Err(CompanionEmotionGenerationError::Cancelled));
    assert!(matches!(
        CompanionTurnCoordinator::new(&database, Some(&cancelled_engine)).begin_send(
            &direct_send_command(&cancelled.value.conversation, "cancelled-send", "hello"),
            TimestampMillis::new(NOW.get() + 11),
            &CancellationToken::new(),
        ),
        Err(crate::CompanionTurnError::Cancelled)
    ));
    assert_eq!(
        ConversationReader::get(&database, cancelled.value.conversation.id)
            .expect("cancelled conversation")
            .conversation
            .revision,
        Revision::INITIAL
    );

    let roleplay_id = plain_character(&database);
    let roleplay = ConversationLaunchPlanner::new(&database)
        .launch_direct(&request(roleplay_id, "roleplay-bypass-launch"), NOW)
        .expect("roleplay launch");
    let engine = ScenarioEmotionEngine::new(Err(CompanionEmotionGenerationError::Cancelled));
    CompanionTurnCoordinator::new(&database, Some(&engine))
        .begin_send(
            &direct_send_command(
                &roleplay.value.conversation,
                "roleplay-bypass-send",
                "hello",
            ),
            TimestampMillis::new(NOW.get() + 11),
            &CancellationToken::new(),
        )
        .expect("roleplay send");
    assert_eq!(engine.calls(), 0);
}

#[tokio::test]
async fn companion_context_assembles_live_prompt_state_deterministically() {
    let database = database_with_builtins();
    let persona_id = seed_persona(&database, "Mira");
    let character_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.interaction_mode = InteractionMode::Companion;
        let mut config = lettuce_companions::CompanionSoulConfig::default();
        config.soul.essence = "Quietly steadfast".into();
        config.prompting.style_notes = "warm but reserved".into();
        defaults.companion_soul = Some(config);
    });
    let mut launch = request(character_id, "companion-context-launch");
    launch.persona = LaunchSelection::Explicit(persona_id);
    let launched = ConversationLaunchPlanner::new(&database)
        .launch_direct(&launch, NOW)
        .expect("launch companion");
    let sent = CompanionTurnCoordinator::<_, ScenarioEmotionEngine>::new(&database, None)
        .begin_send(
            &direct_send_command(
                &launched.value.conversation,
                "companion-context-send",
                "Stay with me.",
            ),
            TimestampMillis::new(NOW.get() + 10),
            &CancellationToken::new(),
        )
        .expect("send companion message");
    let note_id = uuid::Uuid::new_v4();
    let stored_note = database
        .upsert_scheduled_note(lettuce_companions::CompanionScheduledNote {
            id: note_id,
            character_id,
            label: " Reunion ".into(),
            content: " Remember the station reunion. ".into(),
            available_at: TimestampMillis::new(0),
            expires_at: None,
            recurrence: lettuce_companions::ScheduledNoteRecurrence::None,
            recurrence_window_ms: None,
            enabled: true,
            created_at: TimestampMillis::new(NOW.get() + 9),
            updated_at: TimestampMillis::new(NOW.get() + 9),
        })
        .expect("store scheduled note");
    assert_eq!(stored_note.label, "Reunion");
    assert_eq!(stored_note.content, "Remember the station reunion.");
    assert_eq!(
        database
            .list_scheduled_notes(character_id)
            .expect("list scheduled notes"),
        [stored_note]
    );
    let source_message_id = match sent.value.turn.input {
        GenerationInput::UserMessage { message_id } => message_id,
        ref other => panic!("expected user-message input, got {other:?}"),
    };
    let request = context_request_for(&database, launched.value.conversation.id, source_message_id);
    let assembler = crate::ConversationContextAssembler::new(&database);
    let first = assembler
        .assemble(request.clone())
        .await
        .expect("assemble companion context");
    let replay = assembler
        .assemble(request)
        .await
        .expect("reassemble companion context");
    assert_eq!(replay, first);
    let text = first
        .messages
        .iter()
        .flat_map(|message| &message.parts)
        .filter_map(|part| match part {
            ProviderContextPart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains(
        "The following relationship and emotional state describes Ada's live relationship with Mira"
    ));
    assert!(text.contains("Soul essence: Quietly steadfast."));
    assert!(text.contains("Companion style notes: warm but reserved."));
    assert!(text.contains("this chat is episode 1 of one continuous relationship"));
    assert!(text.contains(
        "[Background context you currently hold in mind]\n- Remember the station reunion."
    ));
    assert!(text.contains("Stay with me."));
    assert!(first.attributions.prompt.is_some());
    database
        .delete_scheduled_note(note_id)
        .expect("delete scheduled note");
    assert!(
        database
            .list_scheduled_notes(character_id)
            .expect("list deleted notes")
            .is_empty()
    );
}

#[tokio::test]
async fn companion_context_fails_closed_when_runtime_state_is_missing() {
    let database = database_with_builtins();
    let character_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.interaction_mode = InteractionMode::Companion;
        defaults.companion_soul = Some(lettuce_companions::CompanionSoulConfig::default());
    });
    let prepared = ConversationLaunchPlanner::new(&database)
        .prepare_direct(&request(character_id, "companion-context-missing-state"))
        .expect("prepare companion launch");
    let launched = ConversationCreator::create(&database, prepared, NOW)
        .expect("create without companion state");
    let sent = ConversationRepository::begin_send(
        &database,
        &direct_send_command(
            &launched.value.conversation,
            "companion-context-missing-state-send",
            "Hello.",
        ),
        TimestampMillis::new(NOW.get() + 1),
    )
    .expect("send");
    let source_message_id = match sent.value.turn.input {
        GenerationInput::UserMessage { message_id } => message_id,
        ref other => panic!("expected user-message input, got {other:?}"),
    };
    let request = context_request_for(&database, launched.value.conversation.id, source_message_id);
    assert_eq!(
        crate::ConversationContextAssembler::new(&database)
            .assemble(request)
            .await,
        Err(lettuce_conversations::ContextAssemblyError::ConversationUnavailable)
    );
}

#[test]
fn scene_only_launch_materializes_one_trimmed_scene_message() {
    let database = database();
    let scene = text_scene(CharacterId::new(), 0, "  A quiet harbour at dawn.  ");
    let scene_id = scene.id;
    let character_id = seed_character(&database, vec![scene], Vec::new(), Vec::new(), |defaults| {
        defaults.default_scene_id = Some(scene_id);
    });
    let plan = plan_for(&database, &request(character_id, "scene-only"));
    let details = direct_details(&plan);
    assert!(matches!(details.scene, SnapshotSelection::Inherited(_)));
    assert_eq!(plan.initial_timeline.entries.len(), 1);
    let entry = &plan.initial_timeline.entries[0];
    assert_eq!(entry.role, MessageRole::Scene);
    assert_eq!(entry.author_participant_id, None);
    assert_eq!(message_text(&entry.parts[0]), "A quiet harbour at dawn.");
    assert!(matches!(
        entry.origin,
        InitialMessageOrigin::SelectedScene { .. }
    ));
}

#[test]
fn starter_only_launch_maps_roles_authors_and_origins() {
    let database = database();
    let starter = starter_with(
        CharacterId::new(),
        0,
        "Greeting",
        vec![
            message(StarterRole::User, "Hello"),
            message(StarterRole::Assistant, "Welcome."),
        ],
    );
    let starter_id = starter.id;
    let user_message_id = starter.messages[0].id;
    let character_id = seed_character(&database, Vec::new(), Vec::new(), vec![starter], |_| {});
    let plan = plan_for(
        &database,
        &request_with_starter(character_id, "starter-only", starter_id),
    );
    let user = plan
        .participants
        .iter()
        .find(|participant| participant.role == ParticipantRole::User)
        .expect("user participant")
        .id;
    let character = plan
        .participants
        .iter()
        .find(|participant| participant.role == ParticipantRole::Character)
        .expect("character participant")
        .id;
    assert_eq!(plan.initial_timeline.entries.len(), 2);
    assert_eq!(plan.initial_timeline.entries[0].role, MessageRole::User);
    assert_eq!(
        plan.initial_timeline.entries[0].author_participant_id,
        Some(user)
    );
    assert!(matches!(
        plan.initial_timeline.entries[0].origin,
        InitialMessageOrigin::StarterMessage { starter_message_id, .. }
            if starter_message_id == user_message_id
    ));
    assert_eq!(
        plan.initial_timeline.entries[1].role,
        MessageRole::Assistant
    );
    assert_eq!(
        plan.initial_timeline.entries[1].author_participant_id,
        Some(character)
    );
}

#[test]
fn scene_and_starter_launch_puts_the_scene_first() {
    let database = database();
    let scene = text_scene(CharacterId::new(), 0, "The room is quiet.");
    let scene_id = scene.id;
    let mut starter = starter_with(
        CharacterId::new(),
        0,
        "Greeting",
        vec![message(StarterRole::Assistant, "Welcome.")],
    );
    starter.scene_id = Some(scene_id);
    let starter_id = starter.id;
    let character_id = seed_character(&database, vec![scene], Vec::new(), vec![starter], |_| {});
    let plan = plan_for(
        &database,
        &request_with_starter(character_id, "scene-and-starter", starter_id),
    );
    assert_eq!(plan.initial_timeline.entries.len(), 2);
    assert_eq!(plan.initial_timeline.entries[0].role, MessageRole::Scene);
    assert_eq!(
        plan.initial_timeline.entries[1].role,
        MessageRole::Assistant
    );
}

#[test]
fn a_starter_scene_replaces_the_requested_scene() {
    let database = database();
    let requested = text_scene(CharacterId::new(), 0, "Requested scene.");
    let starter_scene = text_scene(CharacterId::new(), 1, "Starter scene.");
    let requested_id = requested.id;
    let starter_scene_id = starter_scene.id;
    let mut starter = starter_with(CharacterId::new(), 0, "Greeting", Vec::new());
    starter.scene_id = Some(starter_scene_id);
    let starter_id = starter.id;
    let character_id = seed_character(
        &database,
        vec![requested, starter_scene],
        Vec::new(),
        vec![starter],
        |_| {},
    );
    let mut launch = request(character_id, "starter-scene-override");
    launch.scene = LaunchSelection::Explicit(requested_id);
    launch.starter = LaunchSelection::Explicit(starter_id);
    let plan = plan_for(&database, &launch);
    let details = direct_details(&plan);
    match &details.scene {
        SnapshotSelection::Explicit(scene) => assert_eq!(scene.source_id, starter_scene_id),
        other => panic!("expected the starter scene, got {other:?}"),
    }
}

#[test]
fn a_starter_without_a_scene_clears_the_requested_scene() {
    let database = database();
    let requested = text_scene(CharacterId::new(), 0, "Requested scene.");
    let requested_id = requested.id;
    let starter = starter_with(CharacterId::new(), 0, "Greeting", Vec::new());
    let starter_id = starter.id;
    let character_id = seed_character(
        &database,
        vec![requested],
        Vec::new(),
        vec![starter],
        |defaults| {
            defaults.default_scene_id = Some(requested_id);
        },
    );
    let mut launch = request(character_id, "starter-clears-scene");
    launch.scene = LaunchSelection::Explicit(requested_id);
    launch.starter = LaunchSelection::Explicit(starter_id);
    let plan = plan_for(&database, &launch);
    assert_eq!(direct_details(&plan).scene, SnapshotSelection::Disabled);
    assert!(plan.initial_timeline.entries.is_empty());
}

#[test]
fn scene_text_prefers_variant_then_base_then_direction() {
    let character_id = CharacterId::new();
    let mut scene = text_scene(character_id, 0, "  Base body.  ");
    let variant_id = SceneVariantId::new();
    let variant = SceneVariant {
        id: variant_id,
        scene_id: scene.id,
        ordinal: 0,
        content: SceneDocumentV1::new(vec![ScenePart::Text {
            text: " Variant body. ".into(),
        }])
        .expect("variant document"),
        direction: None,
        revision: Revision::INITIAL,
        created_at: TimestampMillis::new(1),
        updated_at: TimestampMillis::new(1),
    };
    scene.selected_variant_id = Some(variant_id);
    assert_eq!(
        policy::resolve_scene_text(&scene, std::slice::from_ref(&variant)),
        Some("Variant body.".to_owned())
    );

    let blank_variant = SceneVariant {
        content: SceneDocumentV1::new(vec![ScenePart::Text { text: "   ".into() }])
            .expect("variant document"),
        ..variant.clone()
    };
    assert_eq!(
        policy::resolve_scene_text(&scene, &[blank_variant]),
        Some("Base body.".to_owned())
    );

    assert_eq!(
        policy::resolve_scene_text(&scene, &[]),
        Some("Base body.".to_owned())
    );

    let mut blank_scene = text_scene(character_id, 0, "   ");
    blank_scene.direction = Some("  Keep it slow.  ".into());
    assert_eq!(
        policy::resolve_scene_text(&blank_scene, &[]),
        Some("Keep it slow.".to_owned())
    );

    let empty_scene = text_scene(character_id, 0, "  ");
    assert_eq!(policy::resolve_scene_text(&empty_scene, &[]), None);
}

#[test]
fn a_blank_scene_is_selected_without_a_timeline_entry() {
    let database = database();
    let scene = text_scene(CharacterId::new(), 0, "   ");
    let scene_id = scene.id;
    let character_id = seed_character(&database, vec![scene], Vec::new(), Vec::new(), |defaults| {
        defaults.default_scene_id = Some(scene_id);
    });
    let plan = plan_for(&database, &request(character_id, "blank-scene"));
    let details = direct_details(&plan);
    match &details.scene {
        SnapshotSelection::Inherited(scene) => assert_eq!(scene.title, "Scene 1"),
        other => panic!("expected an inherited scene, got {other:?}"),
    }
    assert!(plan.initial_timeline.entries.is_empty());
}

#[test]
fn scene_titles_are_bounded_and_fall_back_to_the_ordinal() {
    assert_eq!(policy::scene_title(None, 0), "Scene 1");
    assert_eq!(policy::scene_title(Some("   "), 3), "Scene 4");
    assert_eq!(policy::scene_title(Some(" Harbour "), 0), "Harbour");
    let long = "x".repeat(400);
    assert!(policy::scene_title(Some(&long), 0).chars().count() < 256);
}

#[test]
fn starter_message_content_is_copied_verbatim() {
    let database = database();
    let starter = starter_with(
        CharacterId::new(),
        0,
        "Greeting",
        vec![message(StarterRole::Assistant, "  spaced out  ")],
    );
    let starter_id = starter.id;
    let character_id = seed_character(&database, Vec::new(), Vec::new(), vec![starter], |_| {});
    let plan = plan_for(
        &database,
        &request_with_starter(character_id, "verbatim-starter-message", starter_id),
    );
    assert_eq!(plan.initial_timeline.entries.len(), 1);
    assert_eq!(
        message_text(&plan.initial_timeline.entries[0].parts[0]),
        "  spaced out  "
    );
}

#[test]
fn a_blank_starter_message_is_preserved() {
    let database = database();
    let starter = starter_with(
        CharacterId::new(),
        0,
        "Greeting",
        vec![message(StarterRole::Assistant, "")],
    );
    let starter_id = starter.id;
    let character_id = seed_character(&database, Vec::new(), Vec::new(), vec![starter], |_| {});
    let plan = plan_for(
        &database,
        &request_with_starter(character_id, "blank-starter-message", starter_id),
    );
    assert_eq!(plan.initial_timeline.entries.len(), 1);
    assert_eq!(message_text(&plan.initial_timeline.entries[0].parts[0]), "");
}

#[test]
fn starter_lorebooks_distinguish_explicit_empty_inherit_and_disabled() {
    let database = database();
    let book = seed_lorebook(&database, "Atlas");
    let starter = starter_with(CharacterId::new(), 0, "Greeting", Vec::new());
    let starter_id = starter.id;
    let character_id = seed_character(&database, Vec::new(), Vec::new(), vec![starter], |_| {});
    let bound = CharacterLorebookBindingRepository::bind_character_lorebook(
        &database,
        character_id,
        Revision::INITIAL,
        LorebookBindingCreate {
            lorebook_id: book,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind");

    let inherit = plan_for(
        &database,
        &request_with_starter(character_id, "starter-books-inherit", starter_id),
    );
    assert!(matches!(
        direct_details(&inherit).lorebooks,
        SnapshotSelection::Inherited(_)
    ));
    assert_eq!(
        lorebook_names(&direct_details(&inherit).lorebooks),
        Some(vec!["Atlas".to_owned()])
    );

    let details = CharacterRepository::get(&database, character_id)
        .expect("character")
        .expect("character exists");
    let mut starter = details.starters[0].clone();
    starter.lorebooks = Selection::Explicit(Vec::new());
    let update = lettuce_characters::ConversationStarterDraftUpdate {
        name: starter.name.clone(),
        scene_id: starter.scene_id,
        prompt_id: starter.prompt_id,
        lorebooks: Selection::Explicit(Vec::new()),
    };
    lettuce_characters::StarterRepository::update_starter(
        &database,
        character_id,
        bound.owner_revision,
        starter_id,
        update,
        NOW,
    )
    .expect("explicit empty");
    let explicit_empty = plan_for(
        &database,
        &request_with_starter(character_id, "starter-books-empty", starter_id),
    );
    assert_eq!(
        direct_details(&explicit_empty).lorebooks,
        SnapshotSelection::Explicit(Vec::new())
    );

    let revision = CharacterRepository::get(&database, character_id)
        .expect("character")
        .expect("character exists")
        .character
        .revision;
    lettuce_characters::StarterRepository::update_starter(
        &database,
        character_id,
        revision,
        starter_id,
        lettuce_characters::ConversationStarterDraftUpdate {
            name: "Greeting".into(),
            scene_id: None,
            prompt_id: None,
            lorebooks: Selection::Disabled,
        },
        NOW,
    )
    .expect("disabled");
    let disabled = plan_for(
        &database,
        &request_with_starter(character_id, "starter-books-disabled", starter_id),
    );
    assert_eq!(
        direct_details(&disabled).lorebooks,
        SnapshotSelection::Disabled
    );
}

#[test]
fn inherited_lorebooks_keep_binding_order_and_deduplicate_persona_books() {
    let database = database();
    let shared = seed_lorebook(&database, "Shared");
    let character_book = seed_lorebook(&database, "Character");
    let persona_book = seed_lorebook(&database, "Persona");
    let character_id = plain_character(&database);
    let first = CharacterLorebookBindingRepository::bind_character_lorebook(
        &database,
        character_id,
        Revision::INITIAL,
        LorebookBindingCreate {
            lorebook_id: character_book,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind character book");
    CharacterLorebookBindingRepository::bind_character_lorebook(
        &database,
        character_id,
        first.owner_revision,
        LorebookBindingCreate {
            lorebook_id: shared,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind shared book");
    let persona_id = seed_persona(&database, "Traveller");
    let bound = PersonaLorebookBindingRepository::bind_persona_lorebook(
        &database,
        persona_id,
        Revision::INITIAL,
        LorebookBindingCreate {
            lorebook_id: shared,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind persona shared");
    PersonaLorebookBindingRepository::bind_persona_lorebook(
        &database,
        persona_id,
        bound.owner_revision,
        LorebookBindingCreate {
            lorebook_id: persona_book,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind persona book");

    let mut launch = request(character_id, "inherited-books");
    launch.persona = LaunchSelection::Explicit(persona_id);
    let plan = plan_for(&database, &launch);
    let details = direct_details(&plan);
    assert_eq!(
        lorebook_names(&details.lorebooks),
        Some(vec![
            "Character".to_owned(),
            "Shared".to_owned(),
            "Persona".to_owned(),
        ])
    );
    match &details.persona {
        SnapshotSelection::Explicit(persona) => assert_eq!(
            lorebook_names(&persona.lorebooks),
            Some(vec!["Shared".to_owned(), "Persona".to_owned()])
        ),
        other => panic!("expected an explicit persona, got {other:?}"),
    }
    let sources: Vec<SnapshotSource> = ConversationLaunchPlanner::new(&database)
        .prepare_direct(&launch)
        .expect("prepare")
        .into_parts()
        .1
        .iter()
        .map(|draft| draft.source)
        .collect();
    assert_eq!(
        sources
            .iter()
            .filter(|source| matches!(source, SnapshotSource::Lorebook(_)))
            .count(),
        3
    );
}

#[test]
fn persona_resolution_covers_explicit_inherited_and_disabled() {
    let database = database();
    let character_id = plain_character(&database);
    let persona_id = seed_persona(&database, "Traveller");

    let mut explicit = request(character_id, "persona-explicit");
    explicit.persona = LaunchSelection::Explicit(persona_id);
    assert!(matches!(
        direct_details(&plan_for(&database, &explicit)).persona,
        SnapshotSelection::Explicit(_)
    ));

    let inherit = request(character_id, "persona-inherit-none");
    assert_eq!(
        direct_details(&plan_for(&database, &inherit)).persona,
        SnapshotSelection::Disabled
    );

    let default_revision = PersonaRepository::get_default_snapshot(&database)
        .expect("default snapshot")
        .state
        .revision;
    PersonaRepository::set_default(&database, persona_id, default_revision, NOW)
        .expect("set default");
    let inherited = request(character_id, "persona-inherited");
    assert!(matches!(
        direct_details(&plan_for(&database, &inherited)).persona,
        SnapshotSelection::Inherited(_)
    ));

    let mut disabled = request(character_id, "persona-disabled");
    disabled.persona = LaunchSelection::Disabled;
    assert_eq!(
        direct_details(&plan_for(&database, &disabled)).persona,
        SnapshotSelection::Disabled
    );

    let mut missing = request(character_id, "persona-missing");
    let persona_id = PersonaId::new();
    missing.persona = LaunchSelection::Explicit(persona_id);
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&missing)
            .expect_err("missing persona"),
        ConversationLaunchError::PersonaNotFound { persona_id }
    );
}

#[test]
fn prompt_precedence_prefers_the_starter_and_rejects_a_wrong_purpose() {
    let database = database();
    let character_prompt = seed_prompt(&database, "Character prompt", PromptPurpose::DirectChat);
    let starter_prompt = seed_prompt(&database, "Starter prompt", PromptPurpose::DirectChat);
    let mut starter = starter_with(CharacterId::new(), 0, "Greeting", Vec::new());
    starter.prompt_id = Some(starter_prompt);
    let starter_id = starter.id;
    let character_id = seed_character(
        &database,
        Vec::new(),
        Vec::new(),
        vec![starter],
        |defaults| {
            defaults.direct_prompt_id = Some(character_prompt);
        },
    );

    let inherited = plan_for(&database, &request(character_id, "prompt-inherited"));
    match &direct_details(&inherited).prompt {
        SnapshotSelection::Inherited(prompt) => assert_eq!(prompt.source_id, character_prompt),
        other => panic!("expected an inherited prompt, got {other:?}"),
    }

    let mut explicit = request(character_id, "prompt-explicit");
    explicit.starter = LaunchSelection::Explicit(starter_id);
    match &direct_details(&plan_for(&database, &explicit)).prompt {
        SnapshotSelection::Explicit(prompt) => assert_eq!(prompt.source_id, starter_prompt),
        other => panic!("expected an explicit prompt, got {other:?}"),
    }

    let wrong = seed_prompt(&database, "Group prompt", PromptPurpose::GroupChatRoleplay);
    let wrong_character =
        seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
            defaults.direct_prompt_id = Some(wrong);
        });
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&request(wrong_character, "prompt-wrong-purpose"))
            .expect_err("wrong purpose"),
        ConversationLaunchError::PromptWrongPurpose { prompt_id: wrong }
    );
}

#[test]
fn companion_prompt_prefers_authored_config_and_falls_back_to_built_in() {
    let database = database_with_builtins();
    let built_in = BuiltInPromptService::new(&database)
        .expect("prompt service")
        .bootstrap(NOW)
        .expect("prompt ids")
        .get(BuiltInPromptId::Companion);
    let authored = seed_prompt(
        &database,
        "Companion override",
        PromptPurpose::CompanionChat,
    );
    let character_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.interaction_mode = InteractionMode::Companion;
        let mut config = lettuce_companions::CompanionSoulConfig::default();
        config.prompting.prompt_template_id = Some(authored);
        defaults.companion_soul = Some(config);
    });
    match &direct_details(&plan_for(
        &database,
        &request(character_id, "companion-authored-prompt"),
    ))
    .prompt
    {
        SnapshotSelection::Inherited(prompt) => assert_eq!(prompt.source_id, authored),
        other => panic!("expected inherited companion prompt, got {other:?}"),
    }

    PromptRepository::archive(&database, authored, Revision::INITIAL, NOW)
        .expect("archive companion prompt");
    match &direct_details(&plan_for(
        &database,
        &request(character_id, "companion-archived-prompt"),
    ))
    .prompt
    {
        SnapshotSelection::Inherited(prompt) => assert_eq!(prompt.source_id, built_in),
        other => panic!("expected built-in companion prompt, got {other:?}"),
    }
}

#[test]
fn model_prefers_the_character_default_over_the_application_default() {
    let database = database();
    let application = seed_model(&database, ProviderProtocol::OpenAiCompatible, "openrouter");
    let character_model = seed_model(&database, ProviderProtocol::Gemini, "gemini");
    set_application_default_model(&database, application);
    let character_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.model_profile_id = Some(character_model);
    });
    match &direct_details(&plan_for(
        &database,
        &request(character_id, "model-character"),
    ))
    .model
    {
        SnapshotSelection::Inherited(model) => {
            assert_eq!(model.source_id, character_model);
            assert_eq!(model.provider_protocol, ProviderProtocol::Gemini);
            assert_eq!(model.context_length, Some(8192));
            assert_eq!(model.max_output_tokens, Some(1024));
        }
        other => panic!("expected an inherited model, got {other:?}"),
    }

    let plain = plain_character(&database);
    match &direct_details(&plan_for(&database, &request(plain, "model-application"))).model {
        SnapshotSelection::Inherited(model) => assert_eq!(model.source_id, application),
        other => panic!("expected the application default model, got {other:?}"),
    }
}

#[test]
fn a_launch_without_any_model_default_is_disabled() {
    let database = database();
    let character_id = plain_character(&database);
    let plan = plan_for(&database, &request(character_id, "model-disabled"));
    assert_eq!(direct_details(&plan).model, SnapshotSelection::Disabled);
    let participant = plan
        .participants
        .iter()
        .find(|participant| participant.role == ParticipantRole::Character)
        .expect("character participant");
    assert_eq!(participant.model_selection, SnapshotSelection::Disabled);
}

#[test]
fn an_unmapped_provider_protocol_becomes_other() {
    let database = database();
    let model = seed_model(
        &database,
        ProviderProtocol::StableDiffusion,
        "mystery-vendor",
    );
    let character_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.model_profile_id = Some(model);
    });
    match &direct_details(&plan_for(
        &database,
        &request(character_id, "provider-other"),
    ))
    .model
    {
        SnapshotSelection::Inherited(model) => {
            assert_eq!(model.provider_protocol, ProviderProtocol::StableDiffusion);
        }
        other => panic!("expected an inherited model, got {other:?}"),
    }
}

#[test]
fn memory_is_inherited_from_the_character_policy() {
    let database = database();
    let character_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.memory_policy = MemoryPolicy::Dynamic;
    });
    match &direct_details(&plan_for(&database, &request(character_id, "memory"))).memory {
        SnapshotSelection::Inherited(memory) => {
            assert_eq!(memory.mode, MemoryModeSnapshot::Dynamic);
            assert!(memory.selected_revision_ids.is_empty());
            assert!(memory.policy_ref.is_none());
        }
        other => panic!("expected inherited memory, got {other:?}"),
    }
}

#[test]
fn a_foreign_scene_or_starter_is_rejected() {
    let database = database();
    let foreign_scene = text_scene(CharacterId::new(), 0, "Elsewhere.");
    let foreign_scene_id = foreign_scene.id;
    let foreign_starter = starter_with(CharacterId::new(), 0, "Elsewhere", Vec::new());
    let foreign_starter_id = foreign_starter.id;
    seed_character(
        &database,
        vec![foreign_scene],
        Vec::new(),
        vec![foreign_starter],
        |_| {},
    );
    let character_id = plain_character(&database);

    let mut scene_request = request(character_id, "foreign-scene");
    scene_request.scene = LaunchSelection::Explicit(foreign_scene_id);
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&scene_request)
            .expect_err("foreign scene"),
        ConversationLaunchError::SceneNotOwned {
            scene_id: foreign_scene_id,
            character_id,
        }
    );

    let mut starter_request = request(character_id, "foreign-starter");
    starter_request.starter = LaunchSelection::Explicit(foreign_starter_id);
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&starter_request)
            .expect_err("foreign starter"),
        ConversationLaunchError::StarterNotOwned {
            starter_id: foreign_starter_id,
            character_id,
        }
    );
}

#[test]
fn two_planner_runs_on_identical_state_are_byte_identical() {
    let database = database();
    let scene = text_scene(CharacterId::new(), 0, "A quiet harbour.");
    let scene_id = scene.id;
    let character_id = seed_character(&database, vec![scene], Vec::new(), Vec::new(), |defaults| {
        defaults.default_scene_id = Some(scene_id);
    });
    let launch = request(character_id, "determinism");
    let first = plan_for(&database, &launch);
    let second = plan_for(&database, &launch);
    assert_eq!(first, second);
}

#[test]
fn an_identical_retry_replays_a_single_conversation() {
    let database = database();
    let character_id = plain_character(&database);
    let launch = request(character_id, "idempotent");
    let planner = ConversationLaunchPlanner::new(&database);
    let first = planner.launch_direct(&launch, NOW).expect("first launch");
    let second = planner
        .launch_direct(&launch, TimestampMillis::new(2_000))
        .expect("replayed launch");
    assert_eq!(first.value.conversation.id, second.value.conversation.id);
    assert_eq!(first.operation.id, second.operation.id);
    assert_eq!(
        ConversationReader::page(
            &database,
            &lettuce_conversations::ConversationQuery::default()
        )
        .expect("page")
        .items
        .len(),
        1
    );
}

#[test]
fn companion_launch_rolls_continuity_episode_once_per_persona() {
    let database = database_with_builtins();
    let first_persona = seed_persona(&database, "Mira");
    let second_persona = seed_persona(&database, "Noor");
    let character_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.interaction_mode = InteractionMode::Companion;
        defaults.companion_soul = Some(lettuce_companions::CompanionSoulConfig::default());
    });
    let planner = ConversationLaunchPlanner::new(&database);

    let mut first_request = request(character_id, "continuity-first");
    first_request.persona = LaunchSelection::Explicit(first_persona);
    let first = planner
        .launch_direct(&first_request, NOW)
        .expect("first episode");
    let first_episode = database
        .get_continuity_episode(first.value.conversation.id)
        .expect("load first episode")
        .expect("first episode exists");
    assert_eq!(first_episode.episode_index, 1);
    assert_eq!(first_episode.previous_conversation_id, None);
    assert_eq!(first_episode.ended_at, None);

    let mut second_request = request(character_id, "continuity-second");
    second_request.persona = LaunchSelection::Explicit(first_persona);
    let second_at = TimestampMillis::new(NOW.get() + 100);
    let second = planner
        .launch_direct(&second_request, second_at)
        .expect("second episode");
    let second_episode = database
        .get_continuity_episode(second.value.conversation.id)
        .expect("load second episode")
        .expect("second episode exists");
    assert_eq!(second_episode.episode_index, 2);
    assert_eq!(
        second_episode.previous_conversation_id,
        Some(first.value.conversation.id)
    );
    assert_eq!(
        database
            .get_continuity_episode(first.value.conversation.id)
            .expect("reload first episode")
            .expect("first episode remains")
            .ended_at,
        Some(second_at)
    );

    planner
        .launch_direct(&second_request, TimestampMillis::new(NOW.get() + 200))
        .expect("replay second episode");
    assert_eq!(
        database
            .get_continuity_episode(second.value.conversation.id)
            .expect("reload second episode")
            .expect("second episode remains")
            .ended_at,
        None
    );

    let mut isolated_request = request(character_id, "continuity-isolated-persona");
    isolated_request.persona = LaunchSelection::Explicit(second_persona);
    let isolated = planner
        .launch_direct(&isolated_request, TimestampMillis::new(NOW.get() + 300))
        .expect("isolated persona episode");
    let isolated_episode = database
        .get_continuity_episode(isolated.value.conversation.id)
        .expect("load isolated episode")
        .expect("isolated episode exists");
    assert_eq!(isolated_episode.episode_index, 1);
    assert_eq!(isolated_episode.previous_conversation_id, None);
}

#[test]
fn lorebook_entry_admission_binds_one_restart_safe_creation_job() {
    let database = database_with_builtins();
    let model_id = seed_model(&database, ProviderProtocol::Ollama, "lorebook-entry");
    let mut stored_profile = ModelProfileRepository::get(&database, model_id)
        .expect("profile")
        .expect("profile exists");
    stored_profile.config.chat_parameters.temperature = None;
    let account = ProviderAccountRepository::get(&database, stored_profile.provider_account_id)
        .expect("account")
        .expect("account exists");
    let profile = ResolvedInferenceProfile {
        chat_profile: lettuce_models::resolve_chat_profile(
            &lettuce_models::ExpectedModelIdentity {
                model_profile_id: stored_profile.id,
                model_revision: stored_profile.revision,
                provider_account_id: account.id,
                provider_account_revision: account.revision,
                external_model_id: stored_profile.external_model_id.clone(),
                display_name: stored_profile.display_name.clone(),
                provider_protocol: account.protocol,
                model_kind: stored_profile.kind,
            },
            &stored_profile,
            &account,
            &lettuce_models::ChatParameterResolutionInput::default(),
            &lettuce_models::ChatRequirements::default(),
        )
        .expect("resolve profile"),
        tool_policy: ToolPolicy::Required,
        output_policy: OutputPolicy::Plain,
        safety_policy: SafetyContext::Standard,
        correlation_id: None,
    };
    let character_id = plain_character(&database);
    let conversation = ConversationLaunchPlanner::new(&database)
        .launch_direct(
            &request(character_id, "lorebook-entry-admission-launch"),
            NOW,
        )
        .expect("launch")
        .value
        .conversation;
    let lorebook_id = seed_lorebook(&database, "Harbour Canon");
    let prompt_id = BuiltInPromptService::new(&database)
        .expect("prompt service")
        .bootstrap(TimestampMillis::new(NOW.get() + 1))
        .expect("prompt ids")
        .get(BuiltInPromptId::LorebookEntryWriter);
    let prompt = PromptRepository::get(&database, prompt_id)
        .expect("prompt")
        .expect("prompt exists");
    let request_id = RequestId::new();
    let selected_message_id = MessageId::new();
    let values = lettuce_creation::LorebookEntryPromptValues {
        lorebook_name: "Harbour Canon".into(),
        character_name: "Ada".into(),
        session_title: "Session".into(),
        existing_entries: "(none)".into(),
        direction_prompt: "(none)".into(),
        selected_messages: "1. user: We met by the harbour.".into(),
        memory_summary: "(none)".into(),
        selected_memories: "(none)".into(),
    };
    let make_request = || crate::LorebookEntryAdmissionRequest {
        request_id,
        conversation_id: conversation.id,
        lorebook_id,
        character_id,
        persona_id: None,
        selected_message_ids: vec![selected_message_id],
        selected_memory_ids: Vec::new(),
        source: lettuce_creation::LorebookEntrySource::Messages,
        include_memory_summary: true,
        time_awareness_enabled: false,
        force: false,
        profile: profile.clone(),
        prompt: &prompt,
        prompt_values: values.clone(),
        fallback_format: lettuce_creation::LorebookEntryFallbackFormat::Json,
        now: TimestampMillis::new(NOW.get() + 2),
    };
    let coordinator = crate::LorebookEntryAdmissionCoordinator::new(&database, &database);
    let mut invalid = make_request();
    invalid.request_id = RequestId::new();
    invalid.source = lettuce_creation::LorebookEntrySource::Memory;
    invalid.selected_message_ids.clear();
    invalid.prompt_values.memory_summary = "(none)".into();
    assert!(matches!(
        coordinator.admit(invalid),
        Err(crate::LorebookEntryAdmissionError::Run(
            lettuce_creation::LorebookEntryRunRepositoryError::Invalid
        ))
    ));
    let admitted = coordinator.admit(make_request()).expect("admit run");
    assert!(admitted.created);
    assert_eq!(admitted.job.kind, JobKind::CreationRun);
    assert_eq!(admitted.run.job_id, admitted.job.id);
    assert_eq!(admitted.run.prompt_id, prompt.id);
    assert_eq!(admitted.run.profile, profile);
    let replay = coordinator.admit(make_request()).expect("replay run");
    assert!(!replay.created);
    assert_eq!(replay.run, admitted.run);
    assert_eq!(replay.job, admitted.job);

    let mut changed = make_request();
    changed.force = true;
    assert!(matches!(
        coordinator.admit(changed),
        Err(crate::LorebookEntryAdmissionError::Run(
            lettuce_creation::LorebookEntryRunRepositoryError::Conflict
        ))
    ));
}

#[tokio::test]
async fn lorebook_keyword_admission_freezes_legacy_inputs_and_replays() {
    let database = database_with_builtins();
    let model_id = seed_model(&database, ProviderProtocol::Ollama, "lorebook-keyword");
    let mut stored_profile = ModelProfileRepository::get(&database, model_id)
        .expect("profile")
        .expect("profile exists");
    stored_profile.config.chat_parameters.temperature = None;
    let account = ProviderAccountRepository::get(&database, stored_profile.provider_account_id)
        .expect("account")
        .expect("account exists");
    let profile = ResolvedInferenceProfile {
        chat_profile: lettuce_models::resolve_chat_profile(
            &lettuce_models::ExpectedModelIdentity {
                model_profile_id: stored_profile.id,
                model_revision: stored_profile.revision,
                provider_account_id: account.id,
                provider_account_revision: account.revision,
                external_model_id: stored_profile.external_model_id.clone(),
                display_name: stored_profile.display_name.clone(),
                provider_protocol: account.protocol,
                model_kind: stored_profile.kind,
            },
            &stored_profile,
            &account,
            &lettuce_models::ChatParameterResolutionInput::default(),
            &lettuce_models::ChatRequirements::default(),
        )
        .expect("resolve profile"),
        tool_policy: ToolPolicy::Required,
        output_policy: OutputPolicy::Plain,
        safety_policy: SafetyContext::Standard,
        correlation_id: None,
    };
    let prompt_id = BuiltInPromptService::new(&database)
        .expect("prompt service")
        .bootstrap(TimestampMillis::new(NOW.get() + 1))
        .expect("prompt ids")
        .get(BuiltInPromptId::LorebookKeywordGenerator);
    let prompt = PromptRepository::get(&database, prompt_id)
        .expect("prompt")
        .expect("prompt exists");
    let request_id = RequestId::new();
    let make_request = || crate::LorebookKeywordRequest {
        request_id,
        title: Some("   ".into()),
        content: "  Ada keeps the brass harbour key.  ".into(),
        direction_prompt: None,
        existing_keywords: vec!["  harbour  ".into(), "".into(), " key ".into()],
        profile: profile.clone(),
        prompt: &prompt,
        fallback_format: lettuce_creation::LorebookEntryFallbackFormat::Json,
        now: TimestampMillis::new(NOW.get() + 2),
    };
    let coordinator = crate::LorebookKeywordCoordinator::new(&database, &database);
    let mut invalid = make_request();
    invalid.request_id = RequestId::new();
    invalid.content = "  ".into();
    assert!(matches!(
        coordinator.prepare_and_admit(invalid),
        Err(crate::LorebookKeywordAdmissionError::InvalidInput)
    ));

    let admitted = coordinator
        .prepare_and_admit(make_request())
        .expect("admit keyword run");
    assert!(admitted.created);
    assert_eq!(admitted.job.kind, JobKind::CreationRun);
    assert_eq!(admitted.run.job_id, admitted.job.id);
    assert_eq!(admitted.run.prompt_id, prompt.id);
    assert_eq!(admitted.run.prompt_values.entry_title, "(untitled)");
    assert_eq!(
        admitted.run.prompt_values.entry_content,
        "Ada keeps the brass harbour key."
    );
    assert_eq!(admitted.run.prompt_values.existing_keywords, "harbour, key");
    assert_eq!(admitted.run.prompt_values.direction_prompt, "(none)");

    let replay = coordinator
        .prepare_and_admit(make_request())
        .expect("replay keyword run");
    assert!(!replay.created);
    assert_eq!(replay.run, admitted.run);
    assert_eq!(replay.job, admitted.job);

    let mut changed = make_request();
    changed.content = "Changed content".into();
    assert!(matches!(
        coordinator.prepare_and_admit(changed),
        Err(crate::LorebookKeywordAdmissionError::Run(
            lettuce_creation::LorebookKeywordRunRepositoryError::Conflict
        ))
    ));

    let dispatcher = crate::LorebookKeywordDispatchCoordinator::new(&database, &database);
    let native_work = dispatcher
        .claim(
            request_id,
            WorkerId::new(),
            TimestampMillis::new(NOW.get() + 3),
            Duration::from_secs(30),
            &ResourceAvailability::all(),
        )
        .expect("claim keyword run")
        .expect("keyword work");
    let handle = native_work.handle.clone();
    let native_inference = ScriptedInference {
        outcomes: Mutex::new(VecDeque::from([InferenceOutcome {
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts: Vec::new(),
                tool_calls: vec![ProposedToolCall {
                    provider_call_id: Some("keyword-native".into()),
                    name: "write_lorebook_keywords".into(),
                    arguments: serde_json::json!({
                        "keywords": [" Harbour ", "harbour", "Brass Key"]
                    }),
                    raw_arguments: None,
                    provider_replay: None,
                }],
                provider_replay: None,
            }],
            usage: Some(InferenceUsage {
                input_tokens: 20,
                output_tokens: 5,
            }),
            finish_reason: lettuce_conversations::FinishReason::Stop,
            provider_finish_reason: Some("tool_calls".into()),
            provider_request_id: Some("keyword-native-request".into()),
            warning_codes: Vec::new(),
        }])),
        requests: Mutex::new(Vec::new()),
    };
    let executor = crate::LorebookKeywordExecutionCoordinator::new(&database, &native_inference);
    let result = executor
        .run(
            request_id,
            &prompt,
            &handle,
            None,
            TimestampMillis::new(NOW.get() + 4),
        )
        .await
        .expect("execute native keywords");
    assert_eq!(result.result.keywords, ["Harbour", "Brass Key"]);
    assert_eq!(result.attempts, 1);
    assert!(!result.replayed);
    let replayed = executor
        .run(
            request_id,
            &prompt,
            &handle,
            None,
            TimestampMillis::new(NOW.get() + 5),
        )
        .await
        .expect("replay native keywords");
    assert!(replayed.replayed);
    {
        let requests = native_inference.requests.lock().expect("keyword requests");
        assert_eq!(requests.len(), 1);
        assert!(requests[0].tools.is_some());
        assert!(
            requests[0]
                .context
                .messages
                .last()
                .is_some_and(|message| matches!(
                    message.parts.as_slice(),
                    [ProviderContextPart::Text { text }]
                        if text == lettuce_creation::LOREBOOK_KEYWORD_FINAL_INSTRUCTION
                ))
        );
    }
    assert!(matches!(
        dispatcher
            .settle(
                native_work,
                Ok(result),
                CancellationReason::User,
                TimestampMillis::new(NOW.get() + 6),
            )
            .expect("settle keywords"),
        crate::LorebookKeywordSettledWork::Succeeded { .. }
    ));

    let fallback_request_id = RequestId::new();
    let mut fallback_request = make_request();
    fallback_request.request_id = fallback_request_id;
    let fallback_admitted = coordinator
        .prepare_and_admit(fallback_request)
        .expect("admit fallback keywords");
    let fallback_work = dispatcher
        .claim(
            fallback_request_id,
            WorkerId::new(),
            TimestampMillis::new(NOW.get() + 7),
            Duration::from_secs(30),
            &ResourceAvailability::all(),
        )
        .expect("claim fallback keywords")
        .expect("fallback keyword work");
    let fallback_inference = ScriptedInference {
        outcomes: Mutex::new(VecDeque::from([
            InferenceOutcome {
                candidates: vec![InferenceCandidate {
                    ordinal: 0,
                    parts: vec![MessagePart::Text {
                        text: "none".into(),
                    }],
                    tool_calls: Vec::new(),
                    provider_replay: None,
                }],
                usage: None,
                finish_reason: lettuce_conversations::FinishReason::Stop,
                provider_finish_reason: Some("stop".into()),
                provider_request_id: Some("keyword-primary-empty".into()),
                warning_codes: Vec::new(),
            },
            InferenceOutcome {
                candidates: vec![InferenceCandidate {
                    ordinal: 0,
                    parts: vec![MessagePart::Text {
                        text: serde_json::json!({"result": {
                            "name": "write_lorebook_keywords",
                            "arguments": {"keywords": ["Ada", "ada", "Harbour Key"]}
                        }})
                        .to_string(),
                    }],
                    tool_calls: Vec::new(),
                    provider_replay: None,
                }],
                usage: Some(InferenceUsage {
                    input_tokens: 22,
                    output_tokens: 6,
                }),
                finish_reason: lettuce_conversations::FinishReason::Stop,
                provider_finish_reason: Some("stop".into()),
                provider_request_id: Some("keyword-fallback".into()),
                warning_codes: Vec::new(),
            },
        ])),
        requests: Mutex::new(Vec::new()),
    };
    let fallback = crate::LorebookKeywordExecutionCoordinator::new(&database, &fallback_inference)
        .run(
            fallback_request_id,
            &prompt,
            &fallback_work.handle,
            None,
            TimestampMillis::new(NOW.get() + 8),
        )
        .await
        .expect("execute fallback keywords");
    assert_eq!(fallback.result.keywords, ["Ada", "Harbour Key"]);
    assert_eq!(fallback.attempts, 2);
    assert_eq!(
        lettuce_creation::LorebookKeywordRunRepository::load_lorebook_keyword_attempts(
            &database,
            fallback_admitted.run.request_id,
        )
        .expect("keyword attempts")[1]
            .usage
            .as_ref()
            .map(|usage| usage.output_tokens),
        Some(6)
    );
    assert!(matches!(
        dispatcher
            .settle(
                fallback_work,
                Ok(fallback),
                CancellationReason::User,
                TimestampMillis::new(NOW.get() + 9),
            )
            .expect("settle fallback keywords"),
        crate::LorebookKeywordSettledWork::Succeeded { .. }
    ));
}

#[tokio::test]
async fn staged_lorebook_admission_and_planning_are_restart_safe() {
    let database = database_with_builtins();
    let model_id = seed_model(&database, ProviderProtocol::Ollama, "staged-lorebook");
    let mut stored_profile = ModelProfileRepository::get(&database, model_id)
        .expect("profile")
        .expect("profile exists");
    stored_profile.config.chat_parameters.temperature = None;
    let account = ProviderAccountRepository::get(&database, stored_profile.provider_account_id)
        .expect("account")
        .expect("account exists");
    let profile = ResolvedInferenceProfile {
        chat_profile: lettuce_models::resolve_chat_profile(
            &lettuce_models::ExpectedModelIdentity {
                model_profile_id: stored_profile.id,
                model_revision: stored_profile.revision,
                provider_account_id: account.id,
                provider_account_revision: account.revision,
                external_model_id: stored_profile.external_model_id.clone(),
                display_name: stored_profile.display_name.clone(),
                provider_protocol: account.protocol,
                model_kind: stored_profile.kind,
            },
            &stored_profile,
            &account,
            &lettuce_models::ChatParameterResolutionInput::default(),
            &lettuce_models::ChatRequirements::default(),
        )
        .expect("resolve profile"),
        tool_policy: ToolPolicy::Required,
        output_policy: OutputPolicy::Plain,
        safety_policy: SafetyContext::Standard,
        correlation_id: None,
    };
    let prompt_id = BuiltInPromptService::new(&database)
        .expect("prompt service")
        .bootstrap(TimestampMillis::new(NOW.get() + 1))
        .expect("prompt ids")
        .get(BuiltInPromptId::LorebookGeneratorPlanner);
    let prompt = PromptRepository::get(&database, prompt_id)
        .expect("prompt")
        .expect("prompt exists");
    let request_id = RequestId::new();
    let project_id = lettuce_types::CreationWorkflowId::new();
    let make_request = || crate::StagedLorebookAdmissionRequest {
        request_id,
        project_id,
        brief: "  Harbour world  ".into(),
        initial_lorebook_name: Some(" Harbour Canon ".into()),
        target_count: 2,
        excerpts: vec![lettuce_creation::StagedLorebookSourceExcerpt {
            source_id: "src_01".into(),
            label: "Notes".into(),
            content: "Ada keeps the harbour key.".into(),
        }],
        planner_profile: profile.clone(),
        planner_prompt: &prompt,
        now: TimestampMillis::new(NOW.get() + 2),
    };
    let coordinator = crate::StagedLorebookCoordinator::new(&database, &database);
    let admitted = coordinator
        .admit(make_request())
        .expect("admit staged project");
    assert!(admitted.created);
    assert_eq!(admitted.run.project.target_count, 5);
    assert_eq!(admitted.run.project.brief, "Harbour world");
    assert_eq!(admitted.job.kind, JobKind::CreationRun);
    let replay = coordinator
        .admit(make_request())
        .expect("replay staged project");
    assert!(!replay.created);
    assert_eq!(replay.run, admitted.run);

    let mut changed = make_request();
    changed.brief = "Changed".into();
    assert!(matches!(
        coordinator.admit(changed),
        Err(crate::StagedLorebookAdmissionError::Repository(
            lettuce_creation::StagedLorebookRepositoryError::Conflict
        ))
    ));
    let planning_at = TimestampMillis::new(NOW.get() + 3);
    let planning = coordinator
        .start_planning(request_id, Revision::new(1), planning_at)
        .expect("start planning");
    assert_eq!(
        planning.project.stage,
        lettuce_creation::StagedLorebookStage::Planning
    );
    assert_eq!(planning.project.revision, Revision::new(2));
    let replay_after_start = coordinator
        .admit(make_request())
        .expect("replay admission after planning");
    assert!(!replay_after_start.created);
    assert_eq!(replay_after_start.run, planning);
    assert_eq!(
        coordinator
            .start_planning(request_id, Revision::new(1), planning_at)
            .expect("replay start planning"),
        planning
    );
    let inference = ScriptedInference {
        outcomes: Mutex::new(VecDeque::from([InferenceOutcome {
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts: Vec::new(),
                tool_calls: vec![ProposedToolCall {
                    provider_call_id: Some("planner-outline".into()),
                    name: lettuce_creation::STAGED_LOREBOOK_PLANNER_TOOL_NAME.into(),
                    arguments: serde_json::json!({"entries": [{
                        "title": " Harbour Key ",
                        "category": "item",
                        "proposedKeys": ["harbour key"],
                        "rationale": "Recurring object",
                        "sourceRefs": ["src_01"]
                    }]}),
                    raw_arguments: None,
                    provider_replay: None,
                }],
                provider_replay: None,
            }],
            usage: Some(InferenceUsage {
                input_tokens: 40,
                output_tokens: 10,
            }),
            finish_reason: lettuce_conversations::FinishReason::Stop,
            provider_finish_reason: Some("tool_calls".into()),
            provider_request_id: Some("planner-request".into()),
            warning_codes: Vec::new(),
        }])),
        requests: Mutex::new(Vec::new()),
    };
    let dispatcher = crate::StagedLorebookPlannerDispatchCoordinator::new(&database, &database);
    let work = dispatcher
        .claim(
            request_id,
            WorkerId::new(),
            TimestampMillis::new(NOW.get() + 4),
            Duration::from_secs(30),
            &ResourceAvailability::all(),
        )
        .expect("claim planner")
        .expect("planner work");
    let handle = work.handle.clone();
    let executor = crate::StagedLorebookPlannerExecutionCoordinator::new(&database, &inference);
    let outlined_at = TimestampMillis::new(NOW.get() + 4);
    let result = executor
        .run(request_id, &prompt, &handle, None, outlined_at)
        .await
        .expect("execute planner");
    assert!(!result.replayed);
    let outlined = result.run.clone();
    assert_eq!(
        outlined.project.stage,
        lettuce_creation::StagedLorebookStage::AwaitingOutlineApproval
    );
    assert_eq!(outlined.project.revision, Revision::new(3));
    assert_eq!(outlined.project.outline[0].title, "Harbour Key");
    assert_eq!(
        outlined
            .planner_attempt
            .as_ref()
            .and_then(|attempt| attempt.usage.as_ref())
            .map(|usage| (usage.input_tokens, usage.output_tokens)),
        Some((40, 10))
    );
    {
        let requests = inference.requests.lock().expect("requests");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].profile.tool_policy, ToolPolicy::Required);
        assert_eq!(
            requests[0]
                .tools
                .as_ref()
                .expect("planner tools")
                .definitions[0]
                .name,
            lettuce_creation::STAGED_LOREBOOK_PLANNER_TOOL_NAME
        );
        let rendered_text = requests[0]
            .context
            .messages
            .iter()
            .flat_map(|message| &message.parts)
            .filter_map(|part| match part {
                ProviderContextPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered_text.contains("Harbour world"));
        assert!(rendered_text.contains("[src_01] Notes\nAda keeps the harbour key."));
        assert!(
            rendered_text.contains(lettuce_creation::STAGED_LOREBOOK_PLANNER_FINAL_INSTRUCTION)
        );
    }
    let replayed = executor
        .run(
            request_id,
            &prompt,
            &handle,
            None,
            TimestampMillis::new(NOW.get() + 5),
        )
        .await
        .expect("replay planner checkpoint");
    assert!(replayed.replayed);
    assert_eq!(replayed.run, outlined);
    assert_eq!(inference.requests.lock().expect("requests").len(), 1);
    assert!(matches!(
        dispatcher
            .settle(
                work,
                Ok(result),
                CancellationReason::User,
                TimestampMillis::new(NOW.get() + 5),
            )
            .expect("settle planner job"),
        crate::StagedLorebookPlannerSettledWork::Succeeded { ref job, .. }
            if job.state == JobState::Succeeded
    ));
    assert!(matches!(
        coordinator.start_planning(
            request_id,
            Revision::new(1),
            TimestampMillis::new(NOW.get() + 5)
        ),
        Err(crate::StagedLorebookAdmissionError::Repository(
            lettuce_creation::StagedLorebookRepositoryError::Conflict
        ))
    ));
    let drafting_at = TimestampMillis::new(NOW.get() + 6);
    let drafting = coordinator
        .approve_outline(request_id, Revision::new(3), drafting_at)
        .expect("approve outline");
    assert_eq!(
        drafting.project.stage,
        lettuce_creation::StagedLorebookStage::Drafting
    );
    assert_eq!(drafting.project.revision, Revision::new(4));
    assert_eq!(drafting.project.drafts.len(), 1);
    assert_eq!(
        drafting.project.drafts[0].plan_id,
        drafting.project.outline[0].id
    );
    assert_eq!(drafting.project.drafts[0].title, "Harbour Key");
    assert_eq!(drafting.project.drafts[0].keywords, ["harbour key"]);
    assert!(drafting.project.drafts[0].content.is_empty());
    assert!(!drafting.project.drafts[0].always_active);
    assert!(drafting.project.drafts[0].revisions.is_empty());
    assert_eq!(
        coordinator
            .approve_outline(request_id, Revision::new(3), drafting_at)
            .expect("replay outline approval"),
        drafting
    );
    assert!(matches!(
        coordinator.approve_outline(
            request_id,
            Revision::new(3),
            TimestampMillis::new(NOW.get() + 7)
        ),
        Err(crate::StagedLorebookAdmissionError::Repository(
            lettuce_creation::StagedLorebookRepositoryError::Conflict
        ))
    ));

    let writer_prompt_id = BuiltInPromptService::new(&database)
        .expect("prompt service")
        .bootstrap(TimestampMillis::new(NOW.get() + 8))
        .expect("prompt ids")
        .get(BuiltInPromptId::LorebookGeneratorWriter);
    let writer_prompt = PromptRepository::get(&database, writer_prompt_id)
        .expect("writer prompt")
        .expect("writer prompt exists");
    let writer = crate::StagedLorebookWriterCoordinator::new(&database, &database, &database);
    let batch = writer
        .start_batch(
            request_id,
            Revision::new(4),
            profile.clone(),
            &writer_prompt,
            TimestampMillis::new(NOW.get() + 9),
        )
        .expect("start staged writer batch");
    assert_eq!(batch.writers.len(), 1);
    let admitted_writer = batch.writers[0].clone();
    let writer_request_id = admitted_writer.run.request_id;
    assert!(admitted_writer.created);
    assert_eq!(admitted_writer.run.project_revision, Revision::new(5));
    assert_eq!(admitted_writer.run.prompt_values.brief, "Harbour world");
    assert_eq!(
        admitted_writer.run.prompt_values.outline,
        "1. Harbour Key [item] keys: harbour key"
    );
    assert_eq!(admitted_writer.run.prompt_values.entry_title, "Harbour Key");
    assert_eq!(admitted_writer.run.prompt_values.entry_category, "item");
    assert_eq!(
        admitted_writer.run.prompt_values.entry_proposed_keys,
        "harbour key"
    );
    assert_eq!(
        admitted_writer.run.prompt_values.entry_rationale,
        "Recurring object"
    );
    assert_eq!(
        admitted_writer.run.prompt_values.relevant_excerpts,
        "[src_01] Notes\nAda keeps the harbour key."
    );
    assert_eq!(admitted_writer.job.kind, JobKind::CreationRun);
    let writer_replay = writer
        .start_batch(
            request_id,
            Revision::new(4),
            profile.clone(),
            &writer_prompt,
            TimestampMillis::new(NOW.get() + 10),
        )
        .expect("replay staged writer");
    assert_eq!(writer_replay.writers.len(), 1);
    assert!(!writer_replay.writers[0].created);
    assert_eq!(writer_replay.writers[0].run, admitted_writer.run);
    let foreign_writer_request_id = RequestId::new();
    assert!(matches!(
        writer.prepare_and_admit(crate::StagedLorebookWriterRequest {
            request_id: foreign_writer_request_id,
            project_request_id: request_id,
            plan_id: lettuce_types::LorebookEntryId::new(),
            profile: profile.clone(),
            prompt: &writer_prompt,
            now: TimestampMillis::new(NOW.get() + 10),
        }),
        Err(crate::StagedLorebookWriterAdmissionError::InvalidInput)
    ));
    assert!(matches!(
        lettuce_creation::StagedLorebookWriterRunRepository::load_staged_lorebook_writer_run(
            &database,
            foreign_writer_request_id
        ),
        Err(lettuce_creation::StagedLorebookWriterRunRepositoryError::NotFound)
    ));

    let writer_inference = ScriptedInference {
        outcomes: Mutex::new(VecDeque::from([InferenceOutcome {
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts: Vec::new(),
                tool_calls: vec![ProposedToolCall {
                    provider_call_id: Some("staged-writer".into()),
                    name: lettuce_creation::STAGED_LOREBOOK_WRITER_TOOL_NAME.into(),
                    arguments: serde_json::json!({
                        "title": " Harbour Key ",
                        "keywords": [" harbour ", "key"],
                        "content": " Ada keeps the brass harbour key. ",
                        "alwaysActive": true
                    }),
                    raw_arguments: None,
                    provider_replay: None,
                }],
                provider_replay: None,
            }],
            usage: Some(InferenceUsage {
                input_tokens: 60,
                output_tokens: 20,
            }),
            finish_reason: lettuce_conversations::FinishReason::Stop,
            provider_finish_reason: Some("tool_calls".into()),
            provider_request_id: Some("staged-writer-request".into()),
            warning_codes: Vec::new(),
        }])),
        requests: Mutex::new(Vec::new()),
    };
    let writer_dispatcher =
        crate::StagedLorebookWriterDispatchCoordinator::new(&database, &database, &database);
    let writer_work = writer_dispatcher
        .claim(
            writer_request_id,
            WorkerId::new(),
            TimestampMillis::new(NOW.get() + 11),
            Duration::from_secs(30),
            &ResourceAvailability::all(),
        )
        .expect("claim staged writer")
        .expect("staged writer work");
    let writer_handle = writer_work.handle.clone();
    let writer_executor = crate::StagedLorebookWriterExecutionCoordinator::new(
        &database,
        &database,
        &writer_inference,
    );
    let written = writer_executor
        .run(
            writer_request_id,
            &writer_prompt,
            &writer_handle,
            None,
            TimestampMillis::new(NOW.get() + 12),
        )
        .await
        .expect("execute staged writer");
    assert!(!written.replayed);
    assert_eq!(written.project.project.revision, Revision::new(6));
    assert_eq!(written.project.project.drafts[0].title, "Harbour Key");
    assert_eq!(
        written.project.project.drafts[0].content,
        "Ada keeps the brass harbour key."
    );
    let stored_writer =
        lettuce_creation::StagedLorebookWriterRunRepository::load_staged_lorebook_writer_run(
            &database,
            writer_request_id,
        )
        .expect("stored writer");
    assert_eq!(
        stored_writer
            .attempt
            .as_ref()
            .and_then(|attempt| attempt.usage.as_ref())
            .map(|usage| (usage.input_tokens, usage.output_tokens)),
        Some((60, 20))
    );
    {
        let requests = writer_inference.requests.lock().expect("writer requests");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].profile.tool_policy, ToolPolicy::Required);
        let text = requests[0]
            .context
            .messages
            .iter()
            .flat_map(|message| &message.parts)
            .filter_map(|part| match part {
                ProviderContextPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Harbour world"));
        assert!(text.contains("[src_01] Notes\nAda keeps the harbour key."));
        assert!(text.contains(lettuce_creation::STAGED_LOREBOOK_WRITER_FINAL_INSTRUCTION));
    }
    let written_replay = writer_executor
        .run(
            writer_request_id,
            &writer_prompt,
            &writer_handle,
            None,
            TimestampMillis::new(NOW.get() + 13),
        )
        .await
        .expect("replay staged writer");
    assert!(written_replay.replayed);
    assert_eq!(written_replay.project, written.project);
    assert_eq!(
        writer_inference
            .requests
            .lock()
            .expect("writer requests")
            .len(),
        1
    );
    assert!(matches!(
        writer_dispatcher
            .settle(
                writer_work,
                Ok(written.clone()),
                CancellationReason::User,
                TimestampMillis::new(NOW.get() + 13),
            )
            .expect("settle staged writer job"),
        crate::StagedLorebookWriterSettledWork::Succeeded { ref job, .. }
            if job.state == JobState::Succeeded
    ));

    let plan_id = written.project.project.drafts[0].plan_id;
    let edit_at = TimestampMillis::new(NOW.get() + 14);
    let edit = lettuce_creation::StagedLorebookDraftEdit {
        plan_id,
        title: "  Manual title  ".into(),
        keywords: vec![" harbour ".into(), String::new(), " key ".into()],
        content: "  Manual content  ".into(),
        always_active: false,
    };
    let edited = coordinator
        .edit_draft(
            request_id,
            written.project.project.revision,
            edit.clone(),
            edit_at,
        )
        .expect("edit staged draft");
    assert_eq!(edited.project.drafts[0].title, "  Manual title  ");
    assert_eq!(edited.project.drafts[0].content, "  Manual content  ");
    assert_eq!(edited.project.drafts[0].keywords, ["harbour", "key"]);
    assert_eq!(
        coordinator
            .edit_draft(
                request_id,
                written.project.project.revision,
                edit.clone(),
                edit_at,
            )
            .expect("replay staged draft edit"),
        edited
    );
    let mut changed_edit = edit;
    changed_edit.content = "Different".into();
    assert!(matches!(
        coordinator.edit_draft(
            request_id,
            written.project.project.revision,
            changed_edit,
            edit_at,
        ),
        Err(crate::StagedLorebookAdmissionError::Repository(
            lettuce_creation::StagedLorebookRepositoryError::Conflict
        ))
    ));
    let approval_at = TimestampMillis::new(NOW.get() + 15);
    let approved = coordinator
        .set_draft_approved(
            request_id,
            edited.project.revision,
            plan_id,
            true,
            approval_at,
        )
        .expect("approve staged draft");
    assert_eq!(
        approved.project.drafts[0].status,
        lettuce_creation::StagedLorebookDraftStatus::Approved
    );
    assert_eq!(
        coordinator
            .set_draft_approved(
                request_id,
                edited.project.revision,
                plan_id,
                true,
                approval_at,
            )
            .expect("replay staged draft approval"),
        approved
    );
    let unapproved = coordinator
        .set_draft_approved(
            request_id,
            approved.project.revision,
            plan_id,
            false,
            TimestampMillis::new(NOW.get() + 16),
        )
        .expect("unapprove staged draft");
    assert_eq!(
        unapproved.project.drafts[0].status,
        lettuce_creation::StagedLorebookDraftStatus::Drafted
    );

    let refine_prompt_id = BuiltInPromptService::new(&database)
        .expect("prompt service")
        .bootstrap(TimestampMillis::new(NOW.get() + 17))
        .expect("prompt ids")
        .get(BuiltInPromptId::LorebookGeneratorRefine);
    let refine_prompt = PromptRepository::get(&database, refine_prompt_id)
        .expect("refine prompt")
        .expect("refine prompt exists");
    let refine_request_id = RequestId::new();
    let refine_request = crate::StagedLorebookRefineRequest {
        request_id: refine_request_id,
        project_request_id: request_id,
        plan_id,
        feedback: "  Make it more precise.  ".into(),
        profile: profile.clone(),
        prompt: &refine_prompt,
        now: TimestampMillis::new(NOW.get() + 17),
    };
    let admitted_refine = writer
        .prepare_and_admit_refinement(refine_request.clone())
        .expect("admit staged refinement");
    assert!(admitted_refine.created);
    assert_eq!(admitted_refine.run.project_revision, Revision::new(9));
    assert_eq!(
        admitted_refine
            .run
            .refinement
            .as_ref()
            .map(|refinement| refinement.feedback.as_str()),
        Some("Make it more precise.")
    );
    assert_eq!(
        admitted_refine.run.prompt_values.entry_title,
        "  Manual title  "
    );
    assert_eq!(
        admitted_refine.run.prompt_values.entry_content,
        "  Manual content  "
    );
    assert_eq!(
        admitted_refine.run.prompt_values.entry_keywords,
        "harbour, key"
    );
    assert_eq!(
        admitted_refine.run.prompt_values.entry_always_active,
        "false"
    );
    assert!(
        !writer
            .prepare_and_admit_refinement(refine_request.clone())
            .expect("replay staged refinement")
            .created
    );
    let mut changed_refine = refine_request;
    changed_refine.feedback = "Different".into();
    assert!(matches!(
        writer.prepare_and_admit_refinement(changed_refine),
        Err(crate::StagedLorebookWriterAdmissionError::Run(
            lettuce_creation::StagedLorebookWriterRunRepositoryError::Conflict
        ))
    ));

    let refine_inference = ScriptedInference {
        outcomes: Mutex::new(VecDeque::from([InferenceOutcome {
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts: Vec::new(),
                tool_calls: vec![ProposedToolCall {
                    provider_call_id: Some("staged-refine".into()),
                    name: lettuce_creation::STAGED_LOREBOOK_WRITER_TOOL_NAME.into(),
                    arguments: serde_json::json!({
                        "title": " Precise title ",
                        "keywords": [" harbour ", "fact"],
                        "content": " Precise revised content. ",
                        "alwaysActive": true
                    }),
                    raw_arguments: None,
                    provider_replay: None,
                }],
                provider_replay: None,
            }],
            usage: Some(InferenceUsage {
                input_tokens: 40,
                output_tokens: 12,
            }),
            finish_reason: lettuce_conversations::FinishReason::Stop,
            provider_finish_reason: Some("tool_calls".into()),
            provider_request_id: Some("staged-refine-request".into()),
            warning_codes: Vec::new(),
        }])),
        requests: Mutex::new(Vec::new()),
    };
    let refine_work = writer_dispatcher
        .claim(
            refine_request_id,
            WorkerId::new(),
            TimestampMillis::new(NOW.get() + 18),
            Duration::from_secs(30),
            &ResourceAvailability::all(),
        )
        .expect("claim staged refinement")
        .expect("staged refinement work");
    let refine_executor = crate::StagedLorebookWriterExecutionCoordinator::new(
        &database,
        &database,
        &refine_inference,
    );
    let refined = refine_executor
        .run(
            refine_request_id,
            &refine_prompt,
            &refine_work.handle,
            None,
            TimestampMillis::new(NOW.get() + 19),
        )
        .await
        .expect("execute staged refinement");
    assert_eq!(refined.project.project.revision, Revision::new(10));
    assert_eq!(refined.project.project.drafts[0].title, "Precise title");
    assert_eq!(
        refined.project.project.drafts[0].content,
        "Precise revised content."
    );
    assert_eq!(refined.project.project.drafts[0].revisions.len(), 1);
    assert_eq!(
        refined.project.project.drafts[0].revisions[0].feedback,
        "Make it more precise."
    );
    assert_eq!(
        refined.project.project.drafts[0].revisions[0].content,
        "Precise revised content."
    );
    assert_eq!(
        refined.project.project.drafts[0].revisions[0].timestamp,
        TimestampMillis::new(NOW.get() + 19)
    );
    let refine_replay = refine_executor
        .run(
            refine_request_id,
            &refine_prompt,
            &refine_work.handle,
            None,
            TimestampMillis::new(NOW.get() + 20),
        )
        .await
        .expect("replay staged refinement");
    assert!(refine_replay.replayed);
    assert_eq!(refine_replay.project, refined.project);
    {
        let refine_requests = refine_inference.requests.lock().expect("refine requests");
        assert_eq!(refine_requests.len(), 1);
        let refine_text = refine_requests[0]
            .context
            .messages
            .iter()
            .flat_map(|message| &message.parts)
            .filter_map(|part| match part {
                ProviderContextPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(refine_text.contains("  Manual title  "));
        assert!(refine_text.contains("Make it more precise."));
        assert!(refine_text.contains(lettuce_creation::STAGED_LOREBOOK_REFINE_FINAL_INSTRUCTION));
    }
    let mut changed_refined_draft = refined.project.project.drafts[0].clone();
    changed_refined_draft.content = "Changed after settlement".into();
    changed_refined_draft.revisions[0].content = changed_refined_draft.content.clone();
    assert!(matches!(
        lettuce_creation::StagedLorebookRepository::settle_staged_lorebook_refinement(
            &database,
            request_id,
            Revision::new(9),
            changed_refined_draft,
            TimestampMillis::new(NOW.get() + 19),
        ),
        Err(lettuce_creation::StagedLorebookRepositoryError::Conflict)
    ));
    let coherence_at = TimestampMillis::new(NOW.get() + 21);
    let proposals = vec![
        lettuce_creation::StagedLorebookCoherenceChange::MergeKeys {
            id: "change_0".into(),
            plan_id,
            remove_keys: vec!["fact".into()],
            reason: "duplicate".into(),
        },
        lettuce_creation::StagedLorebookCoherenceChange::ToggleAlwaysActive {
            id: "change_1".into(),
            plan_id,
            new_value: false,
            reason: "not global".into(),
        },
    ];
    let reviewing = coordinator
        .submit_coherence(
            request_id,
            refined.project.project.revision,
            proposals.clone(),
            coherence_at,
        )
        .expect("submit coherence proposals");
    assert_eq!(
        reviewing.project.stage,
        lettuce_creation::StagedLorebookStage::CoherenceReview
    );
    assert_eq!(
        coordinator
            .submit_coherence(
                request_id,
                refined.project.project.revision,
                proposals.clone(),
                coherence_at,
            )
            .expect("replay coherence proposals"),
        reviewing
    );
    let applied_at = TimestampMillis::new(NOW.get() + 22);
    let accepted = vec!["change_1".to_owned()];
    let coherence_applied = coordinator
        .apply_coherence(
            request_id,
            reviewing.project.revision,
            accepted.clone(),
            applied_at,
        )
        .expect("apply selected coherence changes");
    assert_eq!(
        coherence_applied.project.stage,
        lettuce_creation::StagedLorebookStage::DraftsReady
    );
    assert!(!coherence_applied.project.drafts[0].always_active);
    assert_eq!(
        coherence_applied.project.drafts[0].keywords,
        ["harbour", "fact"]
    );
    assert_eq!(
        coordinator
            .apply_coherence(request_id, reviewing.project.revision, accepted, applied_at,)
            .expect("replay coherence application"),
        coherence_applied
    );
    assert!(matches!(
        coordinator.apply_coherence(
            request_id,
            reviewing.project.revision,
            vec!["change_0".into()],
            applied_at,
        ),
        Err(crate::StagedLorebookAdmissionError::Repository(
            lettuce_creation::StagedLorebookRepositoryError::Conflict
        ))
    ));

    let coherence_prompt_id = BuiltInPromptService::new(&database)
        .expect("prompt service")
        .bootstrap(TimestampMillis::new(NOW.get() + 23))
        .expect("prompt ids")
        .get(BuiltInPromptId::LorebookGeneratorCoherence);
    let coherence_prompt = PromptRepository::get(&database, coherence_prompt_id)
        .expect("coherence prompt")
        .expect("coherence prompt exists");
    let coherence_request_id = RequestId::new();
    let coherence_request = crate::StagedLorebookCoherenceRequest {
        request_id: coherence_request_id,
        project_request_id: request_id,
        profile: profile.clone(),
        prompt: &coherence_prompt,
        now: TimestampMillis::new(NOW.get() + 23),
    };
    let admitted_coherence = coordinator
        .admit_coherence(coherence_request.clone())
        .expect("admit coherence inference");
    assert!(admitted_coherence.created);
    let coherence_run = admitted_coherence
        .run
        .coherence_runs
        .iter()
        .find(|run| run.request_id == coherence_request_id)
        .expect("coherence run");
    assert_eq!(coherence_run.project_revision, Revision::new(12));
    assert_eq!(
        coherence_run.drafted_entries,
        "Entry 1 (idx 0): \"Precise title\"\nKeys: harbour, fact\nAlwaysActive: false\nContent: Precise revised content."
    );
    assert!(
        !coordinator
            .admit_coherence(coherence_request)
            .expect("replay coherence admission")
            .created
    );
    let coherence_inference = ScriptedInference {
        outcomes: Mutex::new(VecDeque::from([InferenceOutcome {
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts: Vec::new(),
                tool_calls: vec![ProposedToolCall {
                    provider_call_id: Some("coherence-call".into()),
                    name: lettuce_creation::STAGED_LOREBOOK_COHERENCE_TOOL_NAME.into(),
                    arguments: serde_json::json!({"changes": [{
                        "kind": "toggleAlwaysActive",
                        "entryIdx": 0,
                        "newValue": true,
                        "reason": "overview"
                    }]}),
                    raw_arguments: None,
                    provider_replay: None,
                }],
                provider_replay: None,
            }],
            usage: Some(InferenceUsage {
                input_tokens: 30,
                output_tokens: 10,
            }),
            finish_reason: lettuce_conversations::FinishReason::Stop,
            provider_finish_reason: Some("tool_calls".into()),
            provider_request_id: Some("coherence-request".into()),
            warning_codes: Vec::new(),
        }])),
        requests: Mutex::new(Vec::new()),
    };
    let coherence_dispatcher =
        crate::StagedLorebookCoherenceDispatchCoordinator::new(&database, &database);
    let coherence_work = coherence_dispatcher
        .claim(
            request_id,
            coherence_request_id,
            WorkerId::new(),
            TimestampMillis::new(NOW.get() + 24),
            Duration::from_secs(30),
            &ResourceAvailability::all(),
        )
        .expect("claim coherence inference")
        .expect("coherence work");
    let coherence_executor =
        crate::StagedLorebookCoherenceExecutionCoordinator::new(&database, &coherence_inference);
    let coherence_result = coherence_executor
        .run(
            request_id,
            coherence_request_id,
            &coherence_prompt,
            &coherence_work.handle,
            None,
            TimestampMillis::new(NOW.get() + 25),
        )
        .await
        .expect("execute coherence inference");
    assert!(!coherence_result.replayed);
    assert_eq!(coherence_result.run.project.revision, Revision::new(13));
    assert_eq!(coherence_result.run.project.coherence_proposals.len(), 1);
    let coherence_replay = coherence_executor
        .run(
            request_id,
            coherence_request_id,
            &coherence_prompt,
            &coherence_work.handle,
            None,
            TimestampMillis::new(NOW.get() + 26),
        )
        .await
        .expect("replay coherence inference");
    assert!(coherence_replay.replayed);
    assert_eq!(coherence_replay.run, coherence_result.run);
    {
        let requests = coherence_inference.requests.lock().expect("requests");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].profile.tool_policy, ToolPolicy::Required);
        let text = requests[0]
            .context
            .messages
            .iter()
            .flat_map(|message| &message.parts)
            .filter_map(|part| match part {
                ProviderContextPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Entry 1 (idx 0): \"Precise title\""));
        assert!(text.contains(lettuce_creation::STAGED_LOREBOOK_COHERENCE_FINAL_INSTRUCTION));
    }
    assert!(matches!(
        coherence_dispatcher
            .settle(
                coherence_work,
                Ok(coherence_result),
                CancellationReason::User,
                TimestampMillis::new(NOW.get() + 26),
            )
            .expect("settle coherence job"),
        crate::StagedLorebookCoherenceSettledWork::Succeeded { ref job, .. }
            if job.state == JobState::Succeeded
    ));

    let reviewed = coordinator
        .apply_coherence(
            request_id,
            Revision::new(13),
            vec!["change_0".into()],
            TimestampMillis::new(NOW.get() + 27),
        )
        .expect("finish coherence review");
    let accepted = coordinator
        .set_draft_approved(
            request_id,
            reviewed.project.revision,
            reviewed.project.drafts[0].plan_id,
            true,
            TimestampMillis::new(NOW.get() + 28),
        )
        .expect("approve final draft");
    let destination = lettuce_types::LorebookId::new();
    let commit = lettuce_creation::StagedLorebookCommitRequest {
        project_request_id: request_id,
        expected_project_revision: accepted.project.revision,
        target: lettuce_creation::StagedLorebookCommitTarget::New {
            id: destination,
            name: Some("  ".into()),
        },
        now: TimestampMillis::new(NOW.get() + 29),
    };
    let receipt = coordinator
        .commit(commit.clone())
        .expect("commit staged book");
    assert_eq!(
        receipt.created_entry_ids,
        vec![accepted.project.drafts[0].plan_id]
    );
    assert_eq!(
        coordinator
            .commit(commit.clone())
            .expect("replay final commit"),
        receipt
    );
    let book = lettuce_context::LorebookRepository::get(&database, destination)
        .expect("staged commit fixture")
        .expect("staged commit fixture");
    assert_eq!(book.book.name, "Harbour Canon");
    assert_eq!(book.entries.len(), 1);
    assert_eq!(book.entries[0].content, accepted.project.drafts[0].content);
    assert_eq!(
        book.entries[0].keywords,
        accepted.project.drafts[0].keywords
    );
    assert!(book.entries[0].enabled);
    assert!(!book.entries[0].case_sensitive);
    assert_eq!(book.entries[0].priority, 0);
    assert_eq!(book.entries[0].ordinal, 0);
    let mut changed_commit = commit;
    changed_commit.now = TimestampMillis::new(NOW.get() + 30);
    assert!(coordinator.commit(changed_commit).is_err());
    let committed =
        lettuce_creation::StagedLorebookRepository::load_staged_lorebook(&database, request_id)
            .expect("staged commit fixture");
    assert_eq!(
        committed.project.stage,
        lettuce_creation::StagedLorebookStage::Committed
    );

    for case in ["collision", "new", "existing"] {
        use lettuce_creation::{StagedLorebookCommitTarget, StagedLorebookRepository};
        let at = TimestampMillis::new(NOW.get() + 100);
        let mut input = make_request();
        input.request_id = RequestId::new();
        input.project_id = lettuce_types::CreationWorkflowId::new();
        let id = input.request_id;
        coordinator.admit(input).expect("staged commit fixture");
        let planning = coordinator
            .start_planning(id, Revision::INITIAL, at)
            .expect("staged commit fixture");
        let plans: Vec<_> = (0..3)
            .map(|index| lettuce_creation::StagedLorebookEntryPlan {
                id: if case == "collision" && index == 0 {
                    receipt.created_entry_ids[0]
                } else {
                    lettuce_types::LorebookEntryId::new()
                },
                ordinal: index,
                title: format!("Entry {index}"),
                category: "place".into(),
                proposed_keys: vec!["harbour".into()],
                rationale: "world context".into(),
                source_refs: Vec::new(),
            })
            .collect();
        let outline = database
            .submit_staged_lorebook_outline(id, planning.project.revision, plans, at)
            .expect("staged commit fixture");
        let mut run = coordinator
            .approve_outline(id, outline.project.revision, at)
            .expect("staged commit fixture");
        for index in 0..3 {
            run = coordinator
                .edit_draft(
                    id,
                    run.project.revision,
                    lettuce_creation::StagedLorebookDraftEdit {
                        plan_id: run.project.drafts[index].plan_id,
                        title: if index == 1 {
                            " ".into()
                        } else {
                            " Kept title ".into()
                        },
                        content: if index == 1 {
                            " ".into()
                        } else {
                            " Kept content ".into()
                        },
                        keywords: vec!["harbour".into()],
                        always_active: true,
                    },
                    at,
                )
                .expect("staged commit fixture");
            if index != 2 {
                run = coordinator
                    .set_draft_approved(
                        id,
                        run.project.revision,
                        run.project.drafts[index].plan_id,
                        true,
                        at,
                    )
                    .expect("staged commit fixture");
            }
        }
        run = database
            .start_staged_lorebook_draft_batch(id, run.project.revision, at)
            .expect("staged commit fixture");
        let target_id = if case == "existing" {
            destination
        } else {
            lettuce_types::LorebookId::new()
        };
        let request = lettuce_creation::StagedLorebookCommitRequest {
            project_request_id: id,
            expected_project_revision: run.project.revision,
            target: if case == "existing" {
                StagedLorebookCommitTarget::Existing {
                    id: target_id,
                    expected_revision: Revision::INITIAL,
                }
            } else {
                StagedLorebookCommitTarget::New {
                    id: target_id,
                    name: Some(" Override name ".into()),
                }
            },
            now: at,
        };
        if case == "collision" {
            assert!(coordinator.commit(request).is_err());
            assert!(
                lettuce_context::LorebookRepository::get(&database, target_id)
                    .expect("staged commit fixture")
                    .is_none(),
                "book insert rolls back when an entry ID already exists"
            );
            assert_eq!(
                database
                    .load_staged_lorebook(id)
                    .expect("staged commit fixture"),
                run
            );
            let cancelled = coordinator
                .cancel(id, run.project.revision, at)
                .expect("cancel reviewed project");
            assert_eq!(cancelled.project.drafts, run.project.drafts);
            assert_eq!(cancelled.project.outline, run.project.outline);
            assert_eq!(
                cancelled.project.stage,
                lettuce_creation::StagedLorebookStage::Cancelled
            );
            assert_eq!(
                coordinator
                    .cancel(id, run.project.revision, at)
                    .expect("replay cancellation"),
                cancelled
            );
            assert_eq!(
                lettuce_jobs::JobStore::get(&database, run.job_id)
                    .expect("cancelled job")
                    .expect("job exists")
                    .state,
                JobState::Cancelled
            );
            assert!(
                coordinator
                    .start_planning(id, cancelled.project.revision, at)
                    .is_err()
            );
            continue;
        }
        if case == "existing" {
            let mut stale = request.clone();
            stale.target = StagedLorebookCommitTarget::Existing {
                id: target_id,
                expected_revision: Revision::new(9),
            };
            assert!(coordinator.commit(stale).is_err());
        }
        let saved = coordinator
            .commit(request.clone())
            .expect("staged commit fixture");
        assert_eq!(saved.created_entry_ids, vec![run.project.drafts[0].plan_id]);
        assert_eq!(
            coordinator.commit(request).expect("staged commit fixture"),
            saved
        );
        let details = lettuce_context::LorebookRepository::get(&database, target_id)
            .expect("staged commit fixture")
            .expect("staged commit fixture");
        let entry = details.entries.last().expect("staged commit fixture");
        assert_eq!(entry.title, " Kept title ");
        assert_eq!(entry.content, " Kept content ");
        assert!(entry.always_active);
        assert_eq!(entry.ordinal, u32::from(case == "existing"));
        if case == "existing" {
            assert_eq!(details.entries[0], book.entries[0]);
            assert_eq!(details.book.name, book.book.name);
            assert_eq!(details.book.revision, Revision::new(2));
        } else {
            assert_eq!(details.book.name, "Override name");
        }
    }

    let cancelled_request_id = RequestId::new();
    let mut cancelled_request = make_request();
    cancelled_request.request_id = cancelled_request_id;
    cancelled_request.project_id = lettuce_types::CreationWorkflowId::new();
    cancelled_request.now = TimestampMillis::new(NOW.get() + 11);
    coordinator
        .admit(cancelled_request)
        .expect("admit cancelled planner");
    coordinator
        .start_planning(
            cancelled_request_id,
            Revision::new(1),
            TimestampMillis::new(NOW.get() + 12),
        )
        .expect("start cancelled planner");
    let cancelled_work = dispatcher
        .claim(
            cancelled_request_id,
            WorkerId::new(),
            TimestampMillis::new(NOW.get() + 13),
            Duration::from_secs(30),
            &ResourceAvailability::all(),
        )
        .expect("claim cancelled planner")
        .expect("cancelled planner work");
    let cancelled_project = coordinator
        .cancel(
            cancelled_request_id,
            Revision::new(2),
            TimestampMillis::new(NOW.get() + 14),
        )
        .expect("cancel claimed project");
    assert_eq!(
        cancelled_project.project.stage,
        lettuce_creation::StagedLorebookStage::Cancelled
    );
    assert!(
        coordinator
            .cancel(
                request_id,
                committed.project.revision,
                TimestampMillis::new(NOW.get() + 101)
            )
            .is_err(),
        "committed project cannot be cancelled"
    );
    let cancelled_inference = ScriptedInference {
        outcomes: Mutex::new(VecDeque::new()),
        requests: Mutex::new(Vec::new()),
    };
    let cancelled_executor =
        crate::StagedLorebookPlannerExecutionCoordinator::new(&database, &cancelled_inference);
    let cancelled = cancelled_executor
        .run(
            cancelled_request_id,
            &prompt,
            &cancelled_work.handle,
            None,
            TimestampMillis::new(NOW.get() + 14),
        )
        .await
        .expect_err("cancel before provider dispatch");
    assert!(matches!(
        dispatcher
            .settle(
                cancelled_work,
                Err(cancelled),
                CancellationReason::User,
                TimestampMillis::new(NOW.get() + 14),
            )
            .expect("settle cancelled planner"),
        crate::StagedLorebookPlannerSettledWork::Cancelled { ref job, .. }
            if job.state == JobState::Cancelled
    ));
    assert!(
        cancelled_inference
            .requests
            .lock()
            .expect("requests")
            .is_empty()
    );

    struct CancelDuringInference<'a> {
        database: &'a lettuce_database::Database,
        request_id: RequestId,
        at: TimestampMillis,
    }
    #[async_trait::async_trait]
    impl InferencePort for CancelDuringInference<'_> {
        async fn run(&self, _: InferenceRequest) -> Result<InferenceOutcome, PortError> {
            lettuce_creation::StagedLorebookRepository::cancel_staged_lorebook(
                self.database,
                self.request_id,
                Revision::new(2),
                self.at,
            )
            .expect("cancel while provider is running");
            Ok(InferenceOutcome {
                candidates: Vec::new(),
                usage: None,
                finish_reason: lettuce_conversations::FinishReason::Stop,
                provider_finish_reason: None,
                provider_request_id: None,
                warning_codes: Vec::new(),
            })
        }
    }
    let mut input = make_request();
    input.request_id = RequestId::new();
    input.project_id = lettuce_types::CreationWorkflowId::new();
    let during_id = input.request_id;
    coordinator.admit(input).expect("admit cancellation race");
    coordinator
        .start_planning(
            during_id,
            Revision::INITIAL,
            TimestampMillis::new(NOW.get() + 200),
        )
        .expect("start cancellation race");
    let work = dispatcher
        .claim(
            during_id,
            WorkerId::new(),
            TimestampMillis::new(NOW.get() + 201),
            Duration::from_secs(30),
            &ResourceAvailability::all(),
        )
        .expect("claim cancellation race")
        .expect("race work");
    let provider = CancelDuringInference {
        database: &database,
        request_id: during_id,
        at: TimestampMillis::new(NOW.get() + 202),
    };
    let result = crate::StagedLorebookPlannerExecutionCoordinator::new(&database, &provider)
        .run(
            during_id,
            &prompt,
            &work.handle,
            None,
            TimestampMillis::new(NOW.get() + 202),
        )
        .await;
    assert!(matches!(
        result,
        Err(crate::StagedLorebookPlannerExecutionError::Cancelled)
    ));
    assert!(matches!(
        dispatcher
            .settle(
                work,
                result,
                CancellationReason::User,
                TimestampMillis::new(NOW.get() + 203)
            )
            .expect("settle cancellation race"),
        crate::StagedLorebookPlannerSettledWork::Cancelled { .. }
    ));
    let retained =
        lettuce_creation::StagedLorebookRepository::load_staged_lorebook(&database, during_id)
            .expect("reload cancellation race");
    assert!(retained.planner_attempt.is_none());
    assert!(retained.project.outline.is_empty());
}

#[tokio::test]
async fn lorebook_entry_preparation_loads_owned_sources_and_freezes_legacy_prompt_values() {
    let database = database_with_builtins();
    let model_id = seed_model(&database, ProviderProtocol::Ollama, "lorebook-entry-source");
    let mut stored_profile = ModelProfileRepository::get(&database, model_id)
        .expect("profile")
        .expect("profile exists");
    stored_profile.config.chat_parameters.temperature = None;
    let account = ProviderAccountRepository::get(&database, stored_profile.provider_account_id)
        .expect("account")
        .expect("account exists");
    let profile = ResolvedInferenceProfile {
        chat_profile: lettuce_models::resolve_chat_profile(
            &lettuce_models::ExpectedModelIdentity {
                model_profile_id: stored_profile.id,
                model_revision: stored_profile.revision,
                provider_account_id: account.id,
                provider_account_revision: account.revision,
                external_model_id: stored_profile.external_model_id.clone(),
                display_name: stored_profile.display_name.clone(),
                provider_protocol: account.protocol,
                model_kind: stored_profile.kind,
            },
            &stored_profile,
            &account,
            &lettuce_models::ChatParameterResolutionInput::default(),
            &lettuce_models::ChatRequirements::default(),
        )
        .expect("resolve profile"),
        tool_policy: ToolPolicy::Required,
        output_policy: OutputPolicy::Plain,
        safety_policy: SafetyContext::Standard,
        correlation_id: None,
    };
    let character_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.memory_policy = MemoryPolicy::Dynamic;
    });
    let first = ConversationLaunchPlanner::new(&database)
        .launch_direct(&request(character_id, "lorebook-entry-source-first"), NOW)
        .expect("first launch")
        .value
        .conversation;
    let sent = ConversationRepository::begin_send(
        &database,
        &direct_send_command(
            &first,
            "lorebook-entry-source-first-send",
            "  We met by the harbour.  ",
        ),
        TimestampMillis::new(NOW.get() + 1),
    )
    .expect("first send");
    let selected_message_id = match sent.value.turn.input {
        GenerationInput::UserMessage { message_id } => message_id,
        ref other => panic!("expected user-message input, got {other:?}"),
    };
    let second = ConversationLaunchPlanner::new(&database)
        .launch_direct(
            &request(character_id, "lorebook-entry-source-second"),
            TimestampMillis::new(NOW.get() + 2),
        )
        .expect("second launch")
        .value
        .conversation;
    let foreign = ConversationRepository::begin_send(
        &database,
        &direct_send_command(
            &second,
            "lorebook-entry-source-second-send",
            "Foreign message.",
        ),
        TimestampMillis::new(NOW.get() + 3),
    )
    .expect("second send");
    let foreign_message_id = match foreign.value.turn.input {
        GenerationInput::UserMessage { message_id } => message_id,
        ref other => panic!("expected user-message input, got {other:?}"),
    };
    let first_space = MemoryRepository::get_for_conversation(&database, first.id)
        .expect("first memory space")
        .expect("first memory space exists");
    let selected_memory_id = MemoryId::new();
    let memory_item = MemoryItem {
        id: selected_memory_id,
        text: "  Ada keeps the brass harbour key.  ".into(),
        category: MemoryCategory::WorldDetail,
        source_message_id: Some(selected_message_id),
        source_role: Some(MessageRole::User),
        observed_at: Some(TimestampMillis::new(NOW.get() + 1)),
        observed_time_precision: Some("turn".into()),
        superseded_by: None,
        superseded_at: None,
        supersedes: Vec::new(),
        token_count: 8,
        is_cold: false,
        is_pinned: false,
        importance: Score::ZERO,
        persistence_importance: Score::ZERO,
        prompt_importance: Score::ZERO,
        volatility: Score::ZERO,
        access_count: 0,
        created_at: TimestampMillis::new(NOW.get() + 1),
        last_accessed_at: TimestampMillis::new(NOW.get() + 1),
    };
    let first_space = MemoryRepository::compare_and_apply(
        &database,
        MemoryChangeSet {
            space_id: first_space.id,
            expected_revision: first_space.revision,
            items: vec![memory_item],
        },
    )
    .expect("store selected memory");
    MemorySummaryRepository::compare_and_apply_summary(
        &database,
        MemorySummaryChange {
            expected_revision: first_space.revision,
            summary: MemorySummary {
                space_id: first_space.id,
                text: "  Harbour facts accumulated.  ".into(),
                token_count: 4,
                window_start: 0,
                window_end: 1,
                source_message_ids: vec![selected_message_id],
                updated_at: TimestampMillis::new(NOW.get() + 4),
            },
        },
    )
    .expect("store memory summary");
    let second_space = MemoryRepository::get_for_conversation(&database, second.id)
        .expect("second memory space")
        .expect("second memory space exists");
    let foreign_memory_id = MemoryId::new();
    MemoryRepository::compare_and_apply(
        &database,
        MemoryChangeSet {
            space_id: second_space.id,
            expected_revision: second_space.revision,
            items: vec![MemoryItem {
                id: foreign_memory_id,
                text: "Foreign memory.".into(),
                category: MemoryCategory::Other,
                source_message_id: Some(foreign_message_id),
                source_role: Some(MessageRole::User),
                observed_at: Some(TimestampMillis::new(NOW.get() + 3)),
                observed_time_precision: Some("turn".into()),
                superseded_by: None,
                superseded_at: None,
                supersedes: Vec::new(),
                token_count: 2,
                is_cold: false,
                is_pinned: false,
                importance: Score::ZERO,
                persistence_importance: Score::ZERO,
                prompt_importance: Score::ZERO,
                volatility: Score::ZERO,
                access_count: 0,
                created_at: TimestampMillis::new(NOW.get() + 3),
                last_accessed_at: TimestampMillis::new(NOW.get() + 3),
            }],
        },
    )
    .expect("store foreign memory");
    let lorebook_id = seed_lorebook(&database, "Harbour Canon");
    let prompt_id = BuiltInPromptService::new(&database)
        .expect("prompt service")
        .bootstrap(TimestampMillis::new(NOW.get() + 4))
        .expect("prompt ids")
        .get(BuiltInPromptId::LorebookEntryWriter);
    let prompt = PromptRepository::get(&database, prompt_id)
        .expect("prompt")
        .expect("prompt exists");
    let request_id = RequestId::new();
    let make_request = |request_id, message_ids| crate::LorebookEntryPreparationRequest {
        request_id,
        conversation_id: first.id,
        lorebook_id,
        selected_message_ids: message_ids,
        selected_memory_ids: Vec::new(),
        source: lettuce_creation::LorebookEntrySource::Messages,
        include_memory_summary: true,
        direction_prompt: Some("  Focus on lasting facts.  ".into()),
        force: false,
        time_awareness_enabled: false,
        profile: profile.clone(),
        prompt: &prompt,
        fallback_format: lettuce_creation::LorebookEntryFallbackFormat::Json,
        now: TimestampMillis::new(NOW.get() + 5),
    };
    let coordinator = crate::LorebookEntryPreparationCoordinator::new(&database, &database);
    let admitted = coordinator
        .prepare_and_admit(make_request(request_id, vec![selected_message_id]))
        .expect("prepare and admit");
    assert!(admitted.created);
    assert_eq!(admitted.run.character_id, character_id);
    assert_eq!(admitted.run.prompt_values.lorebook_name, "Harbour Canon");
    assert_eq!(admitted.run.prompt_values.character_name, "Ada");
    assert_eq!(admitted.run.prompt_values.session_title, "Session");
    assert_eq!(admitted.run.prompt_values.existing_entries, "(none)");
    assert_eq!(
        admitted.run.prompt_values.direction_prompt,
        "Focus on lasting facts."
    );
    assert_eq!(
        admitted.run.prompt_values.selected_messages,
        "1. user: We met by the harbour."
    );
    assert_eq!(admitted.run.prompt_values.memory_summary, "(none)");
    assert_eq!(admitted.run.prompt_values.selected_memories, "(none)");
    let replay = coordinator
        .prepare_and_admit(make_request(request_id, vec![selected_message_id]))
        .expect("replay");
    assert!(!replay.created);
    assert_eq!(replay.run, admitted.run);

    for rejected_message_id in [foreign_message_id, MessageId::new()] {
        let rejected_request_id = RequestId::new();
        assert!(matches!(
            coordinator
                .prepare_and_admit(make_request(rejected_request_id, vec![rejected_message_id])),
            Err(crate::LorebookEntryPreparationError::InvalidInput)
        ));
        assert!(matches!(
            lettuce_creation::LorebookEntryRunRepository::load_lorebook_entry_run(
                &database,
                rejected_request_id
            ),
            Err(lettuce_creation::LorebookEntryRunRepositoryError::NotFound)
        ));
    }
    let mut memory_only = make_request(RequestId::new(), Vec::new());
    memory_only.source = lettuce_creation::LorebookEntrySource::Memory;
    memory_only.selected_memory_ids = vec![selected_memory_id];
    let memory_admitted = coordinator
        .prepare_and_admit(memory_only)
        .expect("prepare memory source");
    assert_eq!(
        memory_admitted.run.prompt_values.selected_memories,
        "1. Ada keeps the brass harbour key."
    );
    assert_eq!(
        memory_admitted.run.prompt_values.memory_summary,
        "Harbour facts accumulated."
    );
    for rejected_memory_id in [foreign_memory_id, MemoryId::new()] {
        let rejected_request_id = RequestId::new();
        let mut rejected = make_request(rejected_request_id, Vec::new());
        rejected.source = lettuce_creation::LorebookEntrySource::Memory;
        rejected.include_memory_summary = false;
        rejected.selected_memory_ids = vec![rejected_memory_id];
        assert!(matches!(
            coordinator.prepare_and_admit(rejected),
            Err(crate::LorebookEntryPreparationError::InvalidInput)
        ));
        assert!(matches!(
            lettuce_creation::LorebookEntryRunRepository::load_lorebook_entry_run(
                &database,
                rejected_request_id
            ),
            Err(lettuce_creation::LorebookEntryRunRepositoryError::NotFound)
        ));
    }

    let native_inference = ScriptedInference {
        outcomes: Mutex::new(VecDeque::from([InferenceOutcome {
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts: Vec::new(),
                tool_calls: vec![ProposedToolCall {
                    provider_call_id: Some("native-entry".into()),
                    name: "write_lorebook_entry".into(),
                    arguments: serde_json::json!({
                        "title": "Harbour Key",
                        "keywords": ["harbour", "key"],
                        "content": "Ada keeps the brass harbour key.",
                        "alwaysActive": false
                    }),
                    raw_arguments: None,
                    provider_replay: None,
                }],
                provider_replay: None,
            }],
            usage: Some(InferenceUsage {
                input_tokens: 30,
                output_tokens: 12,
            }),
            finish_reason: lettuce_conversations::FinishReason::Stop,
            provider_finish_reason: Some("tool_calls".into()),
            provider_request_id: Some("native-request".into()),
            warning_codes: Vec::new(),
        }])),
        requests: Mutex::new(Vec::new()),
    };
    let dispatcher = crate::LorebookEntryDispatchCoordinator::new(&database, &database);
    let native_work = dispatcher
        .claim(
            admitted.run.request_id,
            WorkerId::new(),
            TimestampMillis::new(NOW.get() + 6),
            Duration::from_secs(30),
            &ResourceAvailability::all(),
        )
        .expect("claim native entry")
        .expect("native entry work");
    let handle = native_work.handle.clone();
    let executor = crate::LorebookEntryExecutionCoordinator::new(&database, &native_inference);
    let generated = executor
        .run(
            admitted.run.request_id,
            &prompt,
            &handle,
            None,
            TimestampMillis::new(NOW.get() + 6),
        )
        .await
        .expect("execute native entry");
    assert_eq!(generated.attempts, 1);
    assert!(!generated.replayed);
    assert!(matches!(
        &generated.result,
        lettuce_creation::LorebookEntryGenerationResult::Entry { draft }
            if draft.title == "Harbour Key"
    ));
    let settled = dispatcher
        .settle(
            native_work,
            Ok(generated.clone()),
            CancellationReason::User,
            TimestampMillis::new(NOW.get() + 7),
        )
        .expect("settle native entry");
    assert!(matches!(
        settled,
        crate::LorebookEntrySettledWork::Succeeded { ref job, .. }
            if job.state == JobState::Succeeded
    ));
    let replayed = executor
        .run(
            admitted.run.request_id,
            &prompt,
            &handle,
            None,
            TimestampMillis::new(NOW.get() + 8),
        )
        .await
        .expect("replay native entry");
    assert!(replayed.replayed);
    {
        let native_requests = native_inference.requests.lock().expect("native requests");
        assert_eq!(native_requests.len(), 1);
        assert!(native_requests[0].tools.is_some());
        assert!(native_requests[0]
            .context
            .messages
            .iter()
            .flat_map(|message| &message.parts)
            .any(|part| matches!(part, ProviderContextPart::Text { text } if text.contains("# Selected Messages\n1. user: We met by the harbour."))));
        assert!(
            native_requests[0]
                .context
                .messages
                .last()
                .is_some_and(|message| matches!(
                    message.parts.as_slice(),
                    [ProviderContextPart::Text { text }]
                        if text == lettuce_creation::LOREBOOK_ENTRY_MESSAGES_INSTRUCTION
                ))
        );
    }
    let native_attempts =
        lettuce_creation::LorebookEntryRunRepository::load_lorebook_entry_attempts(
            &database,
            admitted.run.request_id,
        )
        .expect("native attempts");
    assert_eq!(native_attempts.len(), 1);
    assert_eq!(
        native_attempts[0]
            .usage
            .as_ref()
            .map(|usage| usage.input_tokens),
        Some(30)
    );

    let fallback_request_id = RequestId::new();
    let fallback_admitted = coordinator
        .prepare_and_admit(make_request(fallback_request_id, vec![selected_message_id]))
        .expect("admit fallback run");
    let fallback_inference = ScriptedInference {
        outcomes: Mutex::new(VecDeque::from([
            InferenceOutcome {
                candidates: vec![InferenceCandidate {
                    ordinal: 0,
                    parts: vec![MessagePart::Text {
                        text: "No native call.".into(),
                    }],
                    tool_calls: Vec::new(),
                    provider_replay: None,
                }],
                usage: None,
                finish_reason: lettuce_conversations::FinishReason::Stop,
                provider_finish_reason: Some("stop".into()),
                provider_request_id: Some("fallback-primary".into()),
                warning_codes: Vec::new(),
            },
            InferenceOutcome {
                candidates: vec![InferenceCandidate {
                    ordinal: 0,
                    parts: vec![MessagePart::Text {
                        text: serde_json::json!({
                            "result": {
                                "name": "write_lorebook_entry",
                                "arguments": {
                                    "title": "Fallback Key",
                                    "keywords": ["key"],
                                    "content": "The key opens the harbour gate.",
                                    "alwaysActive": false
                                }
                            }
                        })
                        .to_string(),
                    }],
                    tool_calls: Vec::new(),
                    provider_replay: None,
                }],
                usage: Some(InferenceUsage {
                    input_tokens: 25,
                    output_tokens: 10,
                }),
                finish_reason: lettuce_conversations::FinishReason::Stop,
                provider_finish_reason: Some("stop".into()),
                provider_request_id: Some("fallback-structured".into()),
                warning_codes: Vec::new(),
            },
        ])),
        requests: Mutex::new(Vec::new()),
    };
    let fallback_work = dispatcher
        .claim(
            fallback_admitted.run.request_id,
            WorkerId::new(),
            TimestampMillis::new(NOW.get() + 9),
            Duration::from_secs(30),
            &ResourceAvailability::all(),
        )
        .expect("claim fallback entry")
        .expect("fallback entry work");
    let fallback_handle = fallback_work.handle.clone();
    let fallback_executor =
        crate::LorebookEntryExecutionCoordinator::new(&database, &fallback_inference);
    let fallback_result = fallback_executor
        .run(
            fallback_request_id,
            &prompt,
            &fallback_handle,
            None,
            TimestampMillis::new(NOW.get() + 10),
        )
        .await
        .expect("execute structured fallback");
    assert_eq!(fallback_result.attempts, 2);
    assert!(matches!(
        &fallback_result.result,
        lettuce_creation::LorebookEntryGenerationResult::Entry { draft }
            if draft.title == "Fallback Key"
    ));
    {
        let fallback_requests = fallback_inference
            .requests
            .lock()
            .expect("fallback requests");
        assert_eq!(fallback_requests.len(), 2);
        assert!(fallback_requests[0].tools.is_some());
        assert!(fallback_requests[1].tools.is_none());
        assert!(
            fallback_requests[1]
                .context
                .messages
                .last()
                .is_some_and(|message| matches!(
                    message.parts.as_slice(),
                    [ProviderContextPart::Text { text }]
                        if text == lettuce_creation::LOREBOOK_ENTRY_JSON_FALLBACK_PROMPT
                ))
        );
    }
    assert!(matches!(
        dispatcher
            .settle(
                fallback_work,
                Ok(fallback_result),
                CancellationReason::User,
                TimestampMillis::new(NOW.get() + 11),
            )
            .expect("settle fallback entry"),
        crate::LorebookEntrySettledWork::Succeeded { .. }
    ));

    let cancelled_request_id = RequestId::new();
    let cancelled_admitted = coordinator
        .prepare_and_admit(make_request(
            cancelled_request_id,
            vec![selected_message_id],
        ))
        .expect("admit cancelled run");
    let cancelled_work = dispatcher
        .claim(
            cancelled_admitted.run.request_id,
            WorkerId::new(),
            TimestampMillis::new(NOW.get() + 12),
            Duration::from_secs(30),
            &ResourceAvailability::all(),
        )
        .expect("claim cancelled entry")
        .expect("cancelled entry work");
    cancelled_work.handle.request_cancel();
    let cancelled_inference = ScriptedInference {
        outcomes: Mutex::new(VecDeque::new()),
        requests: Mutex::new(Vec::new()),
    };
    let cancellation_error =
        crate::LorebookEntryExecutionCoordinator::new(&database, &cancelled_inference)
            .run(
                cancelled_request_id,
                &prompt,
                &cancelled_work.handle,
                None,
                TimestampMillis::new(NOW.get() + 13),
            )
            .await
            .expect_err("cancel before dispatch");
    assert!(matches!(
        &cancellation_error,
        crate::LorebookEntryExecutionError::Cancelled
    ));
    assert!(
        cancelled_inference
            .requests
            .lock()
            .expect("cancelled requests")
            .is_empty()
    );
    assert!(matches!(
        dispatcher
            .settle(
                cancelled_work,
                Err(cancellation_error),
                CancellationReason::User,
                TimestampMillis::new(NOW.get() + 14),
            )
            .expect("settle cancelled entry"),
        crate::LorebookEntrySettledWork::Cancelled { .. }
    ));
    assert_eq!(
        LorebookRepository::get(&database, lorebook_id)
            .expect("lorebook")
            .expect("lorebook exists")
            .entries,
        Vec::new()
    );
}

#[test]
fn reusing_a_key_after_a_source_edit_conflicts() {
    let database = database();
    let character_id = plain_character(&database);
    let launch = request(character_id, "conflicting");
    let planner = ConversationLaunchPlanner::new(&database);
    planner.launch_direct(&launch, NOW).expect("first launch");
    CharacterRepository::revise_profile(
        &database,
        character_id,
        Revision::INITIAL,
        CharacterProfile {
            name: "Ada Lovelace".into(),
            nickname: Some("Addy".into()),
            description: Some("A meticulous engineer".into()),
            definition: None,
            design_description: None,
        },
        NOW,
    )
    .expect("revise character");
    assert_eq!(
        planner
            .launch_direct(&launch, TimestampMillis::new(2_000))
            .expect_err("conflict"),
        ConversationLaunchError::CreateConflict
    );
}

#[test]
fn a_different_operation_key_creates_a_second_conversation() {
    let database = database();
    let character_id = plain_character(&database);
    let planner = ConversationLaunchPlanner::new(&database);
    let first = planner
        .launch_direct(&request(character_id, "first-key"), NOW)
        .expect("first launch");
    let second = planner
        .launch_direct(&request(character_id, "second-key"), NOW)
        .expect("second launch");
    assert_ne!(first.value.conversation.id, second.value.conversation.id);
    assert_eq!(
        ConversationReader::page(
            &database,
            &lettuce_conversations::ConversationQuery::default()
        )
        .expect("page")
        .items
        .len(),
        2
    );
}

#[test]
fn the_app_backend_exposes_the_direct_launch() {
    let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
    let character_id = plain_character(backend.database());
    let result = backend
        .launch_direct_conversation(&request(character_id, "backend-launch"), NOW)
        .expect("launch");
    result.value.validate().expect("aggregate validates");
    assert_eq!(
        result.value.conversation.id,
        super::identity::launch_conversation_id(&key("backend-launch"))
    );
}

#[test]
fn delete_after_without_memory_history_only_tombstones_the_suffix() {
    let database = database();
    let model_id = seed_model(
        &database,
        ProviderProtocol::Ollama,
        "delete-after-no-memory",
    );
    set_application_default_model(&database, model_id);
    let character_id = plain_character(&database);
    let launched = ConversationLaunchPlanner::new(&database)
        .launch_direct(&request(character_id, "delete-after-no-memory-launch"), NOW)
        .expect("launch");
    let conversation = launched.value.conversation;
    let ConversationKind::Direct(details) = &conversation.kind else {
        panic!("direct conversation");
    };
    let model = match &details.model {
        SnapshotSelection::Inherited(model) | SnapshotSelection::Explicit(model) => model.clone(),
        SnapshotSelection::Disabled => panic!("resolved model"),
    };
    let sent = database
        .begin_send(
            &direct_send_command(&conversation, "delete-after-no-memory-send", "Keep this."),
            TimestampMillis::new(NOW.get() + 1),
        )
        .expect("send");
    let user_message_id = match sent.value.turn.input {
        GenerationInput::UserMessage { message_id } => message_id,
        _ => panic!("user message"),
    };
    let finalized = finalize_started_turn(
        &database,
        &sent.value,
        model,
        "delete-after-no-memory",
        NOW.get() + 2,
    );
    let current = ConversationReader::get(&database, conversation.id)
        .expect("conversation")
        .conversation;
    let jobs = InMemoryJobStore::new();
    let deleted = crate::DynamicMemoryDeleteAfterCoordinator::new(&database, &jobs)
        .delete_after(
            &crate::DeleteAfterMessages {
                conversation_id: conversation.id,
                after_message_id: user_message_id,
                expected_revision: current.revision,
                operation: OperationToken {
                    key: key("delete-after-no-memory"),
                    request_digest: ContentHash::parse("fa".repeat(32)).expect("digest"),
                },
                summary_message_interval: 20,
            },
            TimestampMillis::new(NOW.get() + 20),
        )
        .expect("delete suffix");
    assert_eq!(
        deleted
            .tombstone
            .as_ref()
            .expect("tombstone")
            .value
            .message
            .id,
        finalized.value.assistant_message.id
    );
    assert!(deleted.rewind.is_none());
    assert!(deleted.retained_effects.is_empty());
    assert!(deleted.rebuild_admission.is_none());
}

#[test]
fn group_delete_after_keeps_the_scene_anchor_and_tombstones_the_reply() {
    let database = database_with_builtins();
    let model_id = seed_model(&database, ProviderProtocol::Ollama, "group-delete-after");
    let first = seed_named_character_with(&database, "Ada", |defaults| {
        defaults.model_profile_id = Some(model_id);
    });
    let second = seed_named_character(&database, "Bea");
    let group_id = seed_group(
        &database,
        vec![member(first, 0), member(second, 1)],
        Some(group_starting_scene("Keep this scene.")),
        |group| group.chat_mode = ChatMode::Roleplay,
    );
    let launched = ConversationLaunchPlanner::new(&database)
        .launch_group(&group_request(group_id, "group-delete-after-launch"), NOW)
        .expect("launch group");
    let conversation = launched.value.conversation;
    let jobs = InMemoryJobStore::new();
    assert!(matches!(
        crate::CompanionPostTurnMemoryAdmissionCoordinator::new(&database, &jobs)
            .retry_direct_with_model_and_admit(conversation.id, 512, 20, model_id, true),
        Err(crate::CompanionPostTurnMemoryAdmissionError::InvalidBatch)
    ));
    let anchor = ConversationReader::timeline_page(
        &database,
        conversation.id,
        conversation.active_branch_id,
        &PageRequest::default(),
    )
    .expect("group timeline")
    .items
    .into_iter()
    .next()
    .expect("scene anchor")
    .message;
    let speaker = conversation
        .participants
        .iter()
        .find(|participant| {
            participant.source == lettuce_conversations::ParticipantSource::Character(first)
        })
        .expect("group speaker");
    let model = match &speaker.model_selection {
        SnapshotSelection::Inherited(model) | SnapshotSelection::Explicit(model) => model.clone(),
        SnapshotSelection::Disabled => panic!("resolved model"),
    };
    let continued = database
        .begin_continue(
            &ContinueConversation {
                conversation_id: conversation.id,
                branch_id: conversation.active_branch_id,
                expected_revision: conversation.revision,
                forced_speaker: Some(speaker.id),
                swap_roles: false,
                operation: OperationToken {
                    key: key("group-delete-after-continue"),
                    request_digest: ContentHash::parse("fb".repeat(32)).expect("digest"),
                },
            },
            TimestampMillis::new(NOW.get() + 1),
        )
        .expect("continue group");
    let finalized = finalize_started_turn(
        &database,
        &continued.value,
        model,
        "group-delete-after",
        NOW.get() + 2,
    );
    let current = ConversationReader::get(&database, conversation.id)
        .expect("group conversation")
        .conversation;
    let deleted = crate::DynamicMemoryDeleteAfterCoordinator::new(&database, &jobs)
        .delete_after(
            &crate::DeleteAfterMessages {
                conversation_id: conversation.id,
                after_message_id: anchor.id,
                expected_revision: current.revision,
                operation: OperationToken {
                    key: key("group-delete-after"),
                    request_digest: ContentHash::parse("fc".repeat(32)).expect("digest"),
                },
                summary_message_interval: 20,
            },
            TimestampMillis::new(NOW.get() + 20),
        )
        .expect("delete group suffix");
    assert_eq!(
        deleted
            .tombstone
            .as_ref()
            .expect("tombstone")
            .value
            .message
            .id,
        finalized.value.assistant_message.id
    );
    let timeline = ConversationReader::timeline_page(
        &database,
        conversation.id,
        conversation.active_branch_id,
        &PageRequest::default(),
    )
    .expect("retained group timeline");
    assert_eq!(timeline.items.len(), 2);
    assert_eq!(
        timeline.items[0].message.id,
        finalized.value.assistant_message.id
    );
    assert_eq!(
        timeline.items[0].message.visibility,
        MessageVisibility::Tombstoned
    );
    assert_eq!(timeline.items[1].message.id, anchor.id);
    assert_eq!(
        timeline.items[1].message.visibility,
        MessageVisibility::Visible
    );
    assert!(deleted.rewind.is_none());
}

#[tokio::test]
async fn dynamic_memory_two_rounds_replay_mutate_and_finalize_once() {
    let database = database();
    let model_id = seed_model(&database, ProviderProtocol::Ollama, "ollama");
    set_application_default_model(&database, model_id);
    let character_id = plain_character(&database);
    let launched = ConversationLaunchPlanner::new(&database)
        .launch_direct(&request(character_id, "terminal-finalization"), NOW)
        .expect("launch");
    let conversation_id = launched.value.conversation.id;
    let branch_id = launched.value.conversation.active_branch_id;
    let user_id = launched
        .value
        .conversation
        .participants
        .iter()
        .find(|participant| participant.role == ParticipantRole::User)
        .expect("user")
        .id;
    let model = launched
        .value
        .conversation
        .participants
        .iter()
        .find(|participant| participant.role == ParticipantRole::Character)
        .and_then(|participant| match &participant.model_selection {
            SnapshotSelection::Inherited(model) | SnapshotSelection::Explicit(model) => {
                Some(model.clone())
            }
            SnapshotSelection::Disabled => None,
        })
        .expect("resolved model snapshot");
    let operation = |key: &str| OperationToken {
        key: IdempotencyKey::new(key).expect("operation key"),
        request_digest: ContentHash::parse("cd".repeat(32)).expect("digest"),
    };
    let started = database
        .begin_send(
            &SendConversation {
                conversation_id,
                branch_id,
                expected_revision: launched.value.conversation.revision,
                operation: operation("terminal-send"),
                message: MessageDraft {
                    role: MessageRole::User,
                    author_participant_id: Some(user_id),
                    parts: vec![MessagePart::Text {
                        text: "Remember this.".into(),
                    }],
                    visibility: MessageVisibility::Visible,
                    pinned: false,
                    scene_edited: false,
                },
                swap_roles: false,
            },
            TimestampMillis::new(1_010),
        )
        .expect("begin send");
    let job_id = JobId::new();
    database
        .attach_attempt_job(
            &AttachAttemptJob {
                conversation_id,
                turn_id: started.value.turn.id,
                attempt_id: started.value.attempt.id,
                expected_revision: started.value.conversation.revision,
                expected_turn_revision: started.value.turn.revision,
                operation: operation("terminal-attach"),
                job_id,
            },
            TimestampMillis::new(1_011),
        )
        .expect("attach job");
    let mut turn = ConversationReader::get_turn(&database, started.value.turn.id).expect("turn");
    for (sequence, status) in [
        GenerationTurnStatus::Preparing,
        GenerationTurnStatus::ContextPrepared,
        GenerationTurnStatus::Running,
    ]
    .into_iter()
    .enumerate()
    {
        let prior_status = turn.status;
        turn = database
            .append_event(
                turn.id,
                turn.revision,
                &operation(&format!("terminal-stage-{sequence}")),
                GenerationCheckpointEnvelope {
                    turn_id: turn.id,
                    attempt_id: started.value.attempt.id,
                    job_id: Some(job_id),
                    correlation_id: None,
                    sequence: u64::try_from(sequence + 1).expect("sequence"),
                    event: GenerationCheckpointEvent::Stage { status },
                },
                TimestampMillis::new(1_012 + i64::try_from(sequence).expect("time")),
            )
            .unwrap_or_else(|error| {
                panic!("stage {sequence} from {prior_status:?} failed: {error:?}")
            })
            .value;
    }
    let attempt = turn.attempts[0].clone();
    let mut stored_profile = ModelProfileRepository::get(&database, model.source_id)
        .expect("profile")
        .expect("profile exists");
    stored_profile.config.chat_parameters.temperature = None;
    let account = ProviderAccountRepository::get(&database, model.provider_account_id)
        .expect("account")
        .expect("account exists");
    let profile = ResolvedInferenceProfile {
        chat_profile: lettuce_models::resolve_chat_profile(
            &model.expected_chat_identity(),
            &stored_profile,
            &account,
            &lettuce_models::ChatParameterResolutionInput::default(),
            &lettuce_models::ChatRequirements::default(),
        )
        .expect("resolve profile"),
        tool_policy: ToolPolicy::Allowed,
        output_policy: OutputPolicy::Plain,
        safety_policy: SafetyContext::Standard,
        correlation_id: None,
    };
    let space_id = MemorySpaceId::new();
    MemoryRepository::create(
        &database,
        MemorySpaceSnapshot {
            id: space_id,
            revision: Revision::INITIAL,
            items: vec![],
        },
    )
    .expect("memory space");
    let memory_id = MemoryId::new();
    let owner = lettuce_conversations::ToolExecutionOwner {
        conversation_id,
        turn_id: attempt.turn_id,
        attempt_id: attempt.id,
    };
    let initial_outcome = InferenceOutcome {
        candidates: vec![InferenceCandidate {
            ordinal: 0,
            parts: vec![],
            tool_calls: vec![ProposedToolCall {
                provider_call_id: Some("initial-create".into()),
                name: "create_memory".into(),
                arguments: serde_json::json!({
                    "text": "Mira prefers tea",
                    "category": "preference"
                }),
                raw_arguments: None,
                provider_replay: None,
            }],
            provider_replay: None,
        }],
        usage: Some(InferenceUsage {
            input_tokens: 20,
            output_tokens: 5,
        }),
        finish_reason: lettuce_conversations::FinishReason::Stop,
        provider_finish_reason: None,
        provider_request_id: Some("provider-initial".into()),
        warning_codes: vec![],
    };
    let initial_requested = lettuce_conversations::ConversationManager::new(&database)
        .request_tool_executions(
            owner,
            &dynamic_memory_tool_request(),
            initial_outcome.candidates[0].tool_calls.clone(),
            TimestampMillis::new(1_014),
        )
        .expect("admit initial round");
    let initial_validated = ToolExecutionRepository::transition_tool_execution_batch(
        &database,
        &initial_requested
            .iter()
            .map(|execution| ToolExecutionTransition {
                id: execution.id,
                expected_revision: execution.revision,
                next: ToolExecutionStatus::Validated,
                output: None,
                failure: None,
            })
            .collect::<Vec<_>>(),
        TimestampMillis::new(1_015),
    )
    .expect("validate initial round");
    let claim = Claim {
        claim: ClaimRef {
            job_id,
            worker_id: WorkerId::new(),
            attempt: AttemptNo::new(1),
            lease_id: LeaseId::new(),
        },
        lease_expires_at: TimestampMillis::new(2_000),
        input_ref: OutcomeRef::MemoryRun(memory_id),
        recovery_policy: RecoveryPolicy::Restart,
        cancellation_policy: CancellationPolicy::Cooperative,
        resources: vec![
            ResourceClass::ModelLoad,
            ResourceClass::DiskRead,
            ResourceClass::Cpu,
        ],
    };
    let handle = JobHandle::new(job_id);
    let policy = DynamicMemoryPolicy {
        max_entries: 10,
        hot_token_budget: 100,
        cold_threshold: Score::from_basis_points(2_000).expect("score"),
        delete_confidence_default: Score::from_basis_points(5_000).expect("score"),
        max_hard_delete_ratio_per_cycle: Score::from_basis_points(5_000).expect("score"),
    };
    let engine = ScenarioEmbeddingEngine;
    let executor = crate::DynamicMemoryRoundExecutor::new(
        &engine,
        &database,
        &claim,
        space_id,
        &policy,
        Score::from_basis_points(9_000).expect("score"),
    );
    let initial_round = executor
        .execute_admitted_round(
            &initial_validated,
            &[crate::MemoryCreateSeed {
                execution_id: initial_validated[0].id,
                id: memory_id,
                token_count: 4,
                created_at: TimestampMillis::new(1_016),
            }],
            &handle,
            TimestampMillis::new(1_017),
        )
        .expect("execute initial round");
    let missing_memory = MemoryId::new();
    let scripted = ScriptedInference {
        outcomes: Mutex::new(VecDeque::from([
            InferenceOutcome {
                candidates: vec![InferenceCandidate {
                    ordinal: 0,
                    parts: vec![],
                    tool_calls: vec![ProposedToolCall {
                        provider_call_id: Some("second-pin".into()),
                        name: "pin_memory".into(),
                        arguments: serde_json::json!({"id": missing_memory}),
                        raw_arguments: None,
                        provider_replay: None,
                    }],
                    provider_replay: None,
                }],
                usage: Some(InferenceUsage {
                    input_tokens: 7,
                    output_tokens: 2,
                }),
                finish_reason: lettuce_conversations::FinishReason::Stop,
                provider_finish_reason: None,
                provider_request_id: Some("provider-second".into()),
                warning_codes: vec![],
            },
            InferenceOutcome {
                candidates: vec![InferenceCandidate {
                    ordinal: 0,
                    parts: vec![MessagePart::Text {
                        text: "I will remember that.".into(),
                    }],
                    tool_calls: vec![],
                    provider_replay: None,
                }],
                usage: Some(InferenceUsage {
                    input_tokens: 5,
                    output_tokens: 3,
                }),
                finish_reason: lettuce_conversations::FinishReason::Stop,
                provider_finish_reason: None,
                provider_request_id: Some("provider-final".into()),
                warning_codes: vec![],
            },
        ])),
        requests: Mutex::new(vec![]),
    };
    let inference_request = InferenceRequest {
        turn_id: attempt.turn_id,
        attempt_id: attempt.id,
        operation: lettuce_conversations::GenerationOperation::Send,
        profile: profile.clone(),
        context: lettuce_conversations::ProviderNeutralContext {
            messages: vec![lettuce_conversations::ProviderNeutralMessage {
                role: MessageRole::User,
                parts: vec![ProviderContextPart::Text {
                    text: "Remember this.".into(),
                }],
            }],
            attributions: lettuce_conversations::ContextAttributions::default(),
            budget: lettuce_conversations::ContextBudgetReport::default(),
        },
        cancellation: Some(job_id),
        stream_sink: None,
        media_grants: vec![],
        tools: Some(dynamic_memory_tool_request()),
    };
    let continuation = crate::DynamicMemoryContinuationCoordinator::new(&database, &scripted);
    let result = continuation
        .continue_until_terminal(
            conversation_id,
            &attempt,
            &handle,
            inference_request,
            initial_round.settled_executions,
            vec![initial_outcome],
            1,
            1,
            TimestampMillis::new(1_018),
            |executions, handle, at| {
                assert_eq!(executions.len(), 1);
                assert_eq!(executions[0].definition_name, "pin_memory");
                executor.execute_admitted_round(executions, &[], handle, at)
            },
        )
        .await
        .expect("continue two rounds");
    assert!(matches!(
        result.terminal,
        crate::DynamicMemoryContinuationTerminal::Complete { .. }
    ));
    assert_eq!(result.outcomes.len(), 3);
    let replay_counts = {
        let requests = scripted.requests.lock().expect("requests");
        assert_eq!(requests.len(), 2);
        requests
            .iter()
            .map(|request| {
                request
                    .context
                    .messages
                    .iter()
                    .flat_map(|message| &message.parts)
                    .filter(|part| {
                        matches!(
                            part,
                            ProviderContextPart::ToolCall(_) | ProviderContextPart::ToolResult(_)
                        )
                    })
                    .count()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(replay_counts, vec![2, 4]);
    let stored_memory = MemoryRepository::get(&database, space_id)
        .expect("memory")
        .expect("memory exists");
    assert_eq!(stored_memory.revision, Revision::new(2));
    assert_eq!(stored_memory.items.len(), 1);
    assert_eq!(stored_memory.items[0].id, memory_id);
    let latest_plan = DynamicMemoryPreparationRepository::get_preparation_plan(
        &database,
        conversation_id,
        attempt.turn_id,
        attempt.id,
    )
    .expect("latest plan")
    .expect("plan exists");
    assert_eq!(latest_plan.first_execution_ordinal, 1);
    assert!(latest_plan.creates.is_empty());
    let aggregate = ConversationReader::get(&database, conversation_id).expect("aggregate");
    let context = crate::DynamicMemoryTerminalContext {
        conversation_id,
        expected_conversation_revision: aggregate.conversation.revision,
        expected_turn_revision: turn.revision,
        operation: operation("terminal-finalize"),
        model,
        usage_recorded_at: TimestampMillis::new(1_014),
        finalized_at: TimestampMillis::new(1_015),
    };
    assert_eq!(
        attempt.status,
        lettuce_conversations::GenerationAttemptStatus::Running
    );
    assert_eq!(
        context.model.source_id,
        profile.chat_profile.model_profile_id
    );
    assert_eq!(
        context.model.source_revision,
        profile.chat_profile.model_revision
    );
    assert_eq!(
        context.model.provider_account_id,
        profile.chat_profile.provider_account_id
    );
    assert_eq!(
        context.model.provider_account_revision,
        profile.chat_profile.provider_account_revision
    );
    assert_eq!(
        context.model.provider_protocol,
        profile.chat_profile.provider_protocol
    );
    assert_eq!(
        context.model.external_model_id,
        profile.chat_profile.external_model_id
    );
    let coordinator = crate::DynamicMemoryTerminalCoordinator::new(&database, &database);
    let done = coordinator
        .commit(
            &attempt,
            &profile,
            crate::DynamicMemoryContinuationLoopResult {
                terminal: crate::DynamicMemoryContinuationTerminal::Done {
                    summary: Some("memory updated".into()),
                },
                outcomes: result.outcomes.clone(),
            },
            context.clone(),
        )
        .await
        .expect("settle derived memory job");
    let crate::DynamicMemoryTerminalCommit::DerivedMemoryDone {
        summary,
        usage_event_id: done_usage_event_id,
    } = done
    else {
        panic!("expected derived-memory terminal");
    };
    assert_eq!(summary.as_deref(), Some("memory updated"));
    assert_eq!(
        ConversationReader::get_turn(&database, attempt.turn_id)
            .expect("turn remains live")
            .status,
        GenerationTurnStatus::Running
    );
    let first = coordinator
        .commit(&attempt, &profile, result.clone(), context.clone())
        .await
        .expect("finalize");
    let retry = coordinator
        .commit(&attempt, &profile, result, context)
        .await
        .expect("idempotent retry");
    let (
        crate::DynamicMemoryTerminalCommit::ConversationFinalized(first),
        crate::DynamicMemoryTerminalCommit::ConversationFinalized(retry),
    ) = (first, retry)
    else {
        panic!("expected finalized conversation");
    };
    assert_eq!(first.value.candidate.id, retry.value.candidate.id);
    assert_eq!(first.value.usage_event_id, retry.value.usage_event_id);
    assert_eq!(first.value.usage_event_id, done_usage_event_id);
    let usage = lettuce_usage::UsageLedger::get(&database, first.value.usage_event_id)
        .expect("read usage")
        .expect("usage exists");
    assert_eq!(usage.record.attempt_id, attempt.id);
    assert_eq!(usage.record.model_profile_id, Some(model_id));
    assert_eq!(
        usage.record.usage,
        lettuce_conversations::UsageCounters::Known(InferenceUsage {
            input_tokens: 32,
            output_tokens: 10,
        })
    );
}

#[tokio::test]
async fn companion_memory_loop_replays_two_round_checkpoint_without_duplicate_work() {
    let database = database();
    let model_id = seed_model(&database, ProviderProtocol::Ollama, "ollama");
    set_application_default_model(&database, model_id);
    let character_id = plain_character(&database);
    let launched = ConversationLaunchPlanner::new(&database)
        .launch_direct(&request(character_id, "background-memory-loop"), NOW)
        .expect("launch");
    let conversation_id = launched.value.conversation.id;
    let branch_id = launched.value.conversation.active_branch_id;
    let user_id = launched
        .value
        .conversation
        .participants
        .iter()
        .find(|participant| participant.role == ParticipantRole::User)
        .expect("user")
        .id;
    database
        .begin_send(
            &SendConversation {
                conversation_id,
                branch_id,
                expected_revision: launched.value.conversation.revision,
                operation: OperationToken {
                    key: IdempotencyKey::new("background-memory-source").expect("key"),
                    request_digest: ContentHash::parse("ef".repeat(32)).expect("digest"),
                },
                message: MessageDraft {
                    role: MessageRole::User,
                    author_participant_id: Some(user_id),
                    parts: vec![MessagePart::Text {
                        text: "I prefer tea.".into(),
                    }],
                    visibility: MessageVisibility::Visible,
                    pinned: false,
                    scene_edited: false,
                },
                swap_roles: false,
            },
            TimestampMillis::new(1_005),
        )
        .expect("source message");
    let source = ConversationReader::timeline_page(
        &database,
        conversation_id,
        branch_id,
        &PageRequest {
            cursor: None,
            limit: PageLimit::new(20),
        },
    )
    .expect("timeline")
    .items
    .into_iter()
    .find(|item| item.message.visibility == MessageVisibility::Visible)
    .expect("visible source");
    let visible_counts = || {
        let timeline = ConversationReader::timeline_page(
            &database,
            conversation_id,
            branch_id,
            &PageRequest {
                cursor: None,
                limit: PageLimit::new(200),
            },
        )
        .expect("timeline");
        let turns = ConversationReader::page_turns(
            &database,
            conversation_id,
            &PageRequest {
                cursor: None,
                limit: PageLimit::new(200),
            },
        )
        .expect("turns");
        (timeline.items.len(), turns.items.len())
    };
    let before = visible_counts();

    let model = launched
        .value
        .conversation
        .participants
        .iter()
        .find(|participant| participant.role == ParticipantRole::Character)
        .and_then(|participant| match &participant.model_selection {
            SnapshotSelection::Inherited(model) | SnapshotSelection::Explicit(model) => {
                Some(model.clone())
            }
            SnapshotSelection::Disabled => None,
        })
        .expect("model snapshot");
    let mut stored_profile = ModelProfileRepository::get(&database, model.source_id)
        .expect("profile")
        .expect("profile exists");
    stored_profile.config.chat_parameters.temperature = None;
    let account = ProviderAccountRepository::get(&database, model.provider_account_id)
        .expect("account")
        .expect("account exists");
    let profile = ResolvedInferenceProfile {
        chat_profile: lettuce_models::resolve_chat_profile(
            &model.expected_chat_identity(),
            &stored_profile,
            &account,
            &lettuce_models::ChatParameterResolutionInput::default(),
            &lettuce_models::ChatRequirements::default(),
        )
        .expect("resolve profile"),
        tool_policy: ToolPolicy::Required,
        output_policy: OutputPolicy::Plain,
        safety_policy: SafetyContext::Standard,
        correlation_id: None,
    };
    let space_id = MemoryRepository::get_for_conversation(&database, conversation_id)
        .expect("memory space")
        .expect("conversation memory")
        .id;
    let run_id = DynamicMemoryRunId::new();
    let attempt_id = DynamicMemoryAttemptId::new();
    let job_id = JobId::new();
    let admitted = database
        .admit_dynamic_memory_run_attempt(NewDynamicMemoryRunAttempt {
            run_id,
            attempt_id,
            conversation_id,
            space_id,
            starting_memory: MemoryRepository::get(&database, space_id)
                .expect("starting memory")
                .expect("memory space"),
            source_messages: vec![DynamicMemorySourceMessage {
                message_id: source.message.id,
                role: source.message.role,
                render_source: source.message.active_render_source,
                effective_time: source.message.effective_time,
            }],
            profile,
            time_awareness_enabled: false,
            supersession_enabled: false,
            structured_fallback_format: lettuce_memory::DynamicMemoryStructuredFallbackFormat::Xml,
            summary_window: lettuce_memory::DynamicMemorySummaryWindow {
                message_interval: 1,
                start: 0,
                end: 1,
            },
            job_id,
            now: TimestampMillis::new(1_010),
        })
        .expect("admit run");
    database
        .transition_dynamic_memory_attempt(
            attempt_id,
            admitted.attempt.revision,
            DynamicMemoryAttemptStatus::Processing,
            None,
            TimestampMillis::new(1_011),
        )
        .expect("processing");
    let create_id = ToolExecutionId::new();
    database
        .admit_dynamic_memory_inference_round(
            run_id,
            attempt_id,
            0,
            0,
            NewDynamicMemoryInferenceRound {
                ordinal: 0,
                request_context: lettuce_conversations::ProviderNeutralContext {
                    messages: vec![lettuce_conversations::ProviderNeutralMessage {
                        role: MessageRole::User,
                        parts: vec![ProviderContextPart::Text {
                            text: "Remember this preference.".into(),
                        }],
                    }],
                    attributions: Default::default(),
                    budget: Default::default(),
                },
                parts: Vec::new(),
                provider_replay: None,
                usage: None,
                finish_reason: DynamicMemoryRoundFinishReason::Stop,
                provider_request_id: Some("first".into()),
                calls: vec![NewDynamicMemoryToolCall {
                    id: create_id,
                    definition_version: 1,
                    call: ProposedToolCall {
                        provider_call_id: Some("create".into()),
                        name: "create_memory".into(),
                        arguments: serde_json::json!({
                            "text": "Mira prefers tea",
                            "category": "preference",
                            "source_message_id": source.message.id.to_string()
                        }),
                        raw_arguments: None,
                        provider_replay: None,
                    },
                }],
                admitted_at: TimestampMillis::new(1_012),
            },
        )
        .expect("first round");
    let scripted = ScriptedInference {
        outcomes: Mutex::new(VecDeque::from([InferenceOutcome {
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts: Vec::new(),
                tool_calls: vec![ProposedToolCall {
                    provider_call_id: Some("done".into()),
                    name: "done".into(),
                    arguments: serde_json::json!({"summary": "stored preference"}),
                    raw_arguments: None,
                    provider_replay: None,
                }],
                provider_replay: None,
            }],
            usage: None,
            finish_reason: lettuce_conversations::FinishReason::Stop,
            provider_finish_reason: None,
            provider_request_id: Some("second".into()),
            warning_codes: Vec::new(),
        }])),
        requests: Mutex::new(Vec::new()),
    };
    let claim = Claim {
        claim: ClaimRef {
            job_id,
            worker_id: WorkerId::new(),
            attempt: AttemptNo::new(1),
            lease_id: LeaseId::new(),
        },
        lease_expires_at: TimestampMillis::new(2_000),
        input_ref: OutcomeRef::Conversation(conversation_id),
        recovery_policy: RecoveryPolicy::Restart,
        cancellation_policy: CancellationPolicy::Cooperative,
        resources: vec![
            ResourceClass::ModelLoad,
            ResourceClass::DiskRead,
            ResourceClass::Cpu,
        ],
    };
    let handle = JobHandle::new(job_id);
    let policy = DynamicMemoryPolicy {
        max_entries: 10,
        hot_token_budget: 100,
        cold_threshold: Score::from_basis_points(2_000).expect("score"),
        delete_confidence_default: Score::from_basis_points(5_000).expect("score"),
        max_hard_delete_ratio_per_cycle: Score::from_basis_points(5_000).expect("score"),
    };
    let memory_id = MemoryId::new();
    let coordinator =
        crate::CompanionMemoryLoopCoordinator::new(&ScenarioEmbeddingEngine, &database, &scripted);
    let mut seeded_rounds = Vec::new();
    let first = coordinator
        .run_until_done(
            run_id,
            attempt_id,
            &policy,
            Score::from_basis_points(9_000).expect("score"),
            &claim,
            &handle,
            None,
            TimestampMillis::new(1_013),
            |round| {
                seeded_rounds.push(round.ordinal);
                if round.ordinal == 0 {
                    vec![crate::MemoryCreateSeed {
                        execution_id: create_id,
                        id: memory_id,
                        token_count: 4,
                        created_at: TimestampMillis::new(1_013),
                    }]
                } else {
                    Vec::new()
                }
            },
        )
        .await
        .expect("complete loop");
    assert_eq!(first.summary.as_deref(), Some("stored preference"));
    assert_eq!(first.completed_rounds, 2);
    assert_eq!(seeded_rounds, vec![0, 1]);

    seeded_rounds.clear();
    let replay = coordinator
        .run_until_done(
            run_id,
            attempt_id,
            &policy,
            Score::from_basis_points(9_000).expect("score"),
            &claim,
            &handle,
            None,
            TimestampMillis::new(1_099),
            |round| {
                seeded_rounds.push(round.ordinal);
                Vec::new()
            },
        )
        .await
        .expect("replay loop");
    assert_eq!(replay.summary, first.summary);
    assert!(seeded_rounds.is_empty());
    assert_eq!(scripted.requests.lock().expect("requests").len(), 1);
    let stored = MemoryRepository::get(&database, space_id)
        .expect("memory")
        .expect("space");
    assert_eq!(stored.revision, Revision::new(2));
    assert_eq!(stored.items.len(), 1);
    assert_eq!(stored.items[0].id, memory_id);
    assert_eq!(visible_counts(), before);
}

#[test]
fn scene_inherit_uses_the_default_then_the_lowest_ordinal_active_scene() {
    let database = database();
    let first = text_scene(CharacterId::new(), 0, "First scene.");
    let second = text_scene(CharacterId::new(), 1, "Second scene.");
    let second_id = second.id;
    let chosen = seed_character(
        &database,
        vec![first, second],
        Vec::new(),
        Vec::new(),
        |defaults| {
            defaults.default_scene_id = Some(second_id);
        },
    );
    match &direct_details(&plan_for(&database, &request(chosen, "scene-tier-default"))).scene {
        SnapshotSelection::Inherited(scene) => assert_eq!(scene.source_id, second_id),
        other => panic!("expected the character default scene, got {other:?}"),
    }

    let third = text_scene(CharacterId::new(), 0, "First scene.");
    let fourth = text_scene(CharacterId::new(), 1, "Second scene.");
    let third_id = third.id;
    let fallback = seed_character(
        &database,
        vec![third, fourth],
        Vec::new(),
        Vec::new(),
        |_| {},
    );
    match &direct_details(&plan_for(
        &database,
        &request(fallback, "scene-tier-lowest"),
    ))
    .scene
    {
        SnapshotSelection::Inherited(scene) => assert_eq!(scene.source_id, third_id),
        other => panic!("expected the lowest-ordinal scene, got {other:?}"),
    }
}

#[test]
fn an_inherited_scene_pointer_degrades_when_archived_and_errors_when_dangling() {
    let character_id = CharacterId::new();
    let mut archived = text_scene(character_id, 0, "A quiet harbour.");
    archived.status = LifecycleStatus::Archived;
    let archived_id = archived.id;
    let active = text_scene(character_id, 1, "A busy market.");
    let scenes = vec![archived, active];

    assert_eq!(
        policy::inherited_scene(&scenes, Some(archived_id)),
        policy::InheritedScene::None
    );
    match policy::inherited_scene(&scenes, None) {
        policy::InheritedScene::Resolved(scene) => assert_eq!(scene.ordinal, 1),
        other => panic!("expected the lowest-ordinal active scene, got {other:?}"),
    }

    let dangling = SceneId::new();
    assert_eq!(
        policy::inherited_scene(&scenes, Some(dangling)),
        policy::InheritedScene::Dangling(dangling)
    );
    assert_eq!(
        policy::inherited_scene(&[], None),
        policy::InheritedScene::None
    );
}

#[test]
fn a_default_starter_is_never_applied_without_an_explicit_request() {
    let database = database();
    let starter = starter_with(
        CharacterId::new(),
        0,
        "Greeting",
        vec![message(StarterRole::Assistant, "Welcome.")],
    );
    let starter_id = starter.id;
    let character_id = seed_character(
        &database,
        Vec::new(),
        Vec::new(),
        vec![starter],
        |defaults| {
            defaults.default_starter_id = Some(starter_id);
        },
    );
    let plan = plan_for(&database, &request(character_id, "default-starter-ignored"));
    assert_eq!(direct_details(&plan).starter, SnapshotSelection::Disabled);
    assert!(plan.initial_timeline.entries.is_empty());

    let explicit = plan_for(
        &database,
        &request_with_starter(character_id, "default-starter-explicit", starter_id),
    );
    assert!(matches!(
        direct_details(&explicit).starter,
        SnapshotSelection::Explicit(_)
    ));
}

#[test]
fn launching_without_a_default_persona_freezes_the_absence() {
    let database = database();
    let character_id = plain_character(&database);
    let persona_id = seed_persona(&database, "Traveller");
    let launch = request(character_id, "persona-absence-frozen");
    let first = plan_for(&database, &launch);
    assert_eq!(direct_details(&first).persona, SnapshotSelection::Disabled);

    let default_revision = PersonaRepository::get_default_snapshot(&database)
        .expect("default snapshot")
        .state
        .revision;
    PersonaRepository::set_default(&database, persona_id, default_revision, NOW)
        .expect("set default");
    let second = plan_for(&database, &request(character_id, "persona-absence-later"));
    assert!(matches!(
        direct_details(&second).persona,
        SnapshotSelection::Inherited(_)
    ));
}

#[test]
fn an_inherited_archived_prompt_degrades_while_an_authored_one_errors() {
    let database = database();
    let prompt_id = seed_prompt(&database, "Character prompt", PromptPurpose::DirectChat);
    let mut starter = starter_with(CharacterId::new(), 0, "Greeting", Vec::new());
    starter.prompt_id = Some(prompt_id);
    let starter_id = starter.id;
    let character_id = seed_character(
        &database,
        Vec::new(),
        Vec::new(),
        vec![starter],
        |defaults| {
            defaults.direct_prompt_id = Some(prompt_id);
        },
    );
    PromptRepository::archive(&database, prompt_id, Revision::INITIAL, NOW).expect("archive");

    let inherited = plan_for(
        &database,
        &request(character_id, "prompt-archived-inherited"),
    );
    assert_eq!(
        direct_details(&inherited).prompt,
        SnapshotSelection::Disabled
    );

    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&request_with_starter(
                character_id,
                "prompt-archived-authored",
                starter_id
            ))
            .expect_err("archived starter prompt"),
        ConversationLaunchError::PromptArchived { prompt_id }
    );
}

#[test]
fn an_archived_bound_lorebook_is_skipped_but_an_authored_one_errors() {
    let database = database();
    let kept = seed_lorebook(&database, "Kept");
    let archived = seed_lorebook(&database, "Archived");
    let mut starter = starter_with(CharacterId::new(), 0, "Greeting", Vec::new());
    starter.lorebooks = Selection::Explicit(vec![archived]);
    let starter_id = starter.id;
    let character_id = seed_character(&database, Vec::new(), Vec::new(), vec![starter], |_| {});
    let bound = CharacterLorebookBindingRepository::bind_character_lorebook(
        &database,
        character_id,
        Revision::INITIAL,
        LorebookBindingCreate {
            lorebook_id: kept,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind kept");
    CharacterLorebookBindingRepository::bind_character_lorebook(
        &database,
        character_id,
        bound.owner_revision,
        LorebookBindingCreate {
            lorebook_id: archived,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind archived");
    LorebookRepository::archive(&database, archived, Revision::INITIAL, NOW).expect("archive book");

    let inherited = plan_for(
        &database,
        &request(character_id, "lorebook-archived-skipped"),
    );
    assert_eq!(
        lorebook_names(&direct_details(&inherited).lorebooks),
        Some(vec!["Kept".to_owned()])
    );

    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&request_with_starter(
                character_id,
                "lorebook-archived-authored",
                starter_id
            ))
            .expect_err("authored archived lorebook"),
        ConversationLaunchError::LorebookArchived {
            lorebook_id: archived
        }
    );
}

#[test]
fn a_non_chat_model_or_disabled_provider_is_rejected() {
    let database = database();
    let image = seed_model_with(
        &database,
        ProviderProtocol::OpenAiCompatible,
        "openai",
        ModelKind::Image,
        true,
    );
    let character_id = seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
        defaults.model_profile_id = Some(image);
    });
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&request(character_id, "model-non-chat"))
            .expect_err("non chat model"),
        ConversationLaunchError::NonChatModel {
            model_profile_id: image
        }
    );

    let disabled = seed_model_with(
        &database,
        ProviderProtocol::Anthropic,
        "anthropic",
        ModelKind::Chat,
        false,
    );
    let disabled_character =
        seed_character(&database, Vec::new(), Vec::new(), Vec::new(), |defaults| {
            defaults.model_profile_id = Some(disabled);
        });
    let account = ModelProfileRepository::get(&database, disabled)
        .expect("profile")
        .expect("profile exists")
        .provider_account_id;
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&request(disabled_character, "model-disabled-provider"))
            .expect_err("disabled provider"),
        ConversationLaunchError::ProviderDisabled {
            provider_account_id: account
        }
    );
}

#[test]
fn an_image_application_default_model_is_rejected() {
    let database = database();
    let image = seed_model_with(
        &database,
        ProviderProtocol::OpenAiCompatible,
        "openai",
        ModelKind::Image,
        true,
    );
    set_application_default_model(&database, image);
    let character_id = plain_character(&database);
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&request(character_id, "application-default-image"))
            .expect_err("image application default"),
        ConversationLaunchError::NonChatModel {
            model_profile_id: image
        }
    );
}

#[test]
fn the_resolved_lorebook_set_is_bounded_before_loading() {
    assert!(!policy::lorebook_bound_exceeded(
        policy::MAX_LAUNCH_LOREBOOKS
    ));
    assert!(policy::lorebook_bound_exceeded(
        policy::MAX_LAUNCH_LOREBOOKS + 1
    ));

    let database = database();
    let character_id = plain_character(&database);
    let mut revision = Revision::INITIAL;
    for index in 0..policy::MAX_LAUNCH_LOREBOOKS {
        let book = seed_lorebook(&database, &format!("Book {index}"));
        revision = CharacterLorebookBindingRepository::bind_character_lorebook(
            &database,
            character_id,
            revision,
            LorebookBindingCreate {
                lorebook_id: book,
                target: BindingInsertionTarget::Append,
            },
            NOW,
        )
        .expect("bind")
        .owner_revision;
    }
    assert!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&request(character_id, "lorebooks-at-bound"))
            .is_ok()
    );

    let overflow = seed_lorebook(&database, "Book overflow");
    CharacterLorebookBindingRepository::bind_character_lorebook(
        &database,
        character_id,
        revision,
        LorebookBindingCreate {
            lorebook_id: overflow,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind overflow");
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&request(character_id, "lorebooks-over-bound"))
            .expect_err("lorebook bound"),
        ConversationLaunchError::TooManyLorebooks {
            max: policy::MAX_LAUNCH_LOREBOOKS
        }
    );
}

#[test]
fn the_initial_timeline_is_bounded_before_building() {
    assert!(!policy::timeline_bound_exceeded(
        policy::MAX_LAUNCH_TIMELINE_ENTRIES
    ));
    assert!(policy::timeline_bound_exceeded(
        policy::MAX_LAUNCH_TIMELINE_ENTRIES + 1
    ));

    let database = database();
    let at_bound = starter_with(
        CharacterId::new(),
        0,
        "At bound",
        (0..policy::MAX_LAUNCH_TIMELINE_ENTRIES)
            .map(|_| message(StarterRole::Assistant, "Line."))
            .collect(),
    );
    let over_bound = starter_with(
        CharacterId::new(),
        1,
        "Over bound",
        (0..=policy::MAX_LAUNCH_TIMELINE_ENTRIES)
            .map(|_| message(StarterRole::Assistant, "Line."))
            .collect(),
    );
    let at_bound_id = at_bound.id;
    let over_bound_id = over_bound.id;
    let character_id = seed_character(
        &database,
        Vec::new(),
        Vec::new(),
        vec![at_bound, over_bound],
        |_| {},
    );
    let plan = plan_for(
        &database,
        &request_with_starter(character_id, "timeline-at-bound", at_bound_id),
    );
    assert_eq!(
        plan.initial_timeline.entries.len(),
        policy::MAX_LAUNCH_TIMELINE_ENTRIES
    );
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_direct(&request_with_starter(
                character_id,
                "timeline-over-bound",
                over_bound_id
            ))
            .expect_err("timeline bound"),
        ConversationLaunchError::TooManyInitialMessages {
            max: policy::MAX_LAUNCH_TIMELINE_ENTRIES
        }
    );
}

#[test]
fn retrying_after_a_source_is_archived_reports_the_existing_conversation() {
    let database = database();
    let character_id = plain_character(&database);
    let launch = request(character_id, "already-launched");
    let planner = ConversationLaunchPlanner::new(&database);
    let created = planner.launch_direct(&launch, NOW).expect("first launch");
    CharacterRepository::archive(&database, character_id, Revision::INITIAL, NOW)
        .expect("archive character");
    assert_eq!(
        planner
            .launch_direct(&launch, TimestampMillis::new(2_000))
            .expect_err("already launched"),
        ConversationLaunchError::AlreadyLaunched {
            conversation_id: created.value.conversation.id
        }
    );
}

#[test]
fn source_drift_between_the_two_reads_is_detected() {
    let character_id = CharacterId::new();
    let persona_id = PersonaId::new();
    assert_eq!(
        policy::detect_source_drift(
            (character_id, Revision::INITIAL, Revision::INITIAL),
            Some((persona_id, Revision::INITIAL, Revision::INITIAL)),
        ),
        None
    );
    assert_eq!(
        policy::detect_source_drift((character_id, Revision::INITIAL, Revision::new(2)), None,),
        Some(SnapshotSource::Character(character_id))
    );
    assert_eq!(
        policy::detect_source_drift(
            (character_id, Revision::INITIAL, Revision::INITIAL),
            Some((persona_id, Revision::INITIAL, Revision::new(3))),
        ),
        Some(SnapshotSource::Persona(persona_id))
    );
}

#[test]
fn a_variant_belonging_to_another_scene_is_ignored() {
    let character_id = CharacterId::new();
    let mut scene = text_scene(character_id, 0, "Base body.");
    let variant_id = SceneVariantId::new();
    scene.selected_variant_id = Some(variant_id);
    let foreign = SceneVariant {
        id: variant_id,
        scene_id: SceneId::new(),
        ordinal: 0,
        content: SceneDocumentV1::new(vec![ScenePart::Text {
            text: "Foreign body.".into(),
        }])
        .expect("variant document"),
        direction: None,
        revision: Revision::INITIAL,
        created_at: TimestampMillis::new(1),
        updated_at: TimestampMillis::new(1),
    };
    assert_eq!(
        policy::resolve_scene_text(&scene, std::slice::from_ref(&foreign)),
        Some("Base body.".to_owned())
    );
}

#[test]
fn generated_operation_keys_are_unique() {
    let first = DirectConversationLaunchRequest::new_operation_key();
    let second = DirectConversationLaunchRequest::new_operation_key();
    assert_ne!(first, second);
    assert_ne!(
        super::identity::launch_conversation_id(&first),
        super::identity::launch_conversation_id(&second)
    );
}

fn backend() -> AppBackend {
    AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend")
}

fn seed_named_character(database: &Database, name: &str) -> CharacterId {
    seed_named_character_with(database, name, |_| {})
}

fn seed_named_character_with(
    database: &Database,
    name: &str,
    mutate: impl FnOnce(&mut CharacterDefaults),
) -> CharacterId {
    let id = CharacterId::new();
    let mut defaults = CharacterDefaults::default();
    mutate(&mut defaults);
    let character = Character::new(
        id,
        CharacterProfile {
            name: name.into(),
            nickname: None,
            description: Some("A member of the cast".into()),
            definition: None,
            design_description: None,
        },
        CharacterProvenance::default(),
        defaults,
        CharacterPresentationV1::default(),
        None,
        CharacterMedia::default(),
        TimestampMillis::new(1),
    )
    .expect("character");
    CharacterRepository::create(
        database,
        CreateCharacterPlan {
            character,
            scenes: Vec::new(),
            variants: Vec::new(),
            starters: Vec::new(),
        },
    )
    .expect("create character");
    id
}

fn member(character_id: CharacterId, ordinal: u32) -> GroupMember {
    GroupMember {
        character_id,
        ordinal,
        muted: false,
        model_profile_override: None,
    }
}

fn group_starting_scene(text: &str) -> GroupStartingScene {
    GroupStartingScene {
        scene: Scene::new(
            SceneId::new(),
            SceneOwner::Group(GroupId::new()),
            0,
            SceneDocumentV1::new(vec![ScenePart::Text { text: text.into() }]).expect("document"),
            TimestampMillis::new(1),
        )
        .expect("scene"),
        variants: Vec::new(),
    }
}

fn seed_group(
    database: &Database,
    members: Vec<GroupMember>,
    starting_scene: Option<GroupStartingScene>,
    mutate: impl FnOnce(&mut GroupProfile),
) -> GroupId {
    let id = GroupId::new();
    let starting_scene = starting_scene.map(|mut starting| {
        starting.scene.owner = SceneOwner::Group(id);
        for variant in &mut starting.variants {
            variant.scene_id = starting.scene.id;
        }
        starting
    });
    let mut group =
        GroupProfile::new(id, "Cast".into(), members, TimestampMillis::new(1)).expect("group");
    group.starting_scene_id = starting_scene.as_ref().map(|starting| starting.scene.id);
    mutate(&mut group);
    GroupRepository::create(
        database,
        CreateGroupPlan {
            group,
            starting_scene,
        },
    )
    .expect("create group");
    id
}

fn two_member_group(database: &Database) -> GroupId {
    let first = seed_named_character(database, "Ada");
    let second = seed_named_character(database, "Bea");
    seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |_| {},
    )
}

fn group_request(group_id: GroupId, operation_key: &str) -> GroupConversationLaunchRequest {
    GroupConversationLaunchRequest {
        format_version: 1,
        title: "Session".into(),
        user: DirectUserParticipant {
            display_name: "Traveller".into(),
            authored_description: None,
        },
        group_id,
        persona: LaunchSelection::Inherit,
        operation_key: key(operation_key),
    }
}

fn group_plan_for(
    database: &Database,
    request: &GroupConversationLaunchRequest,
) -> CreateConversationPlan {
    ConversationLaunchPlanner::new(database)
        .prepare_group(request, NOW)
        .expect("prepare group launch")
        .into_parts()
        .0
}

fn group_details(plan: &CreateConversationPlan) -> &GroupConversationDetails {
    match &plan.kind {
        ConversationKind::Group(details) => details,
        ConversationKind::Direct(_) => panic!("group launch produced a direct conversation"),
    }
}

fn character_participants(
    plan: &CreateConversationPlan,
) -> Vec<&lettuce_conversations::ConversationParticipantDraft> {
    plan.participants
        .iter()
        .filter(|participant| participant.role == ParticipantRole::Character)
        .collect()
}

fn model_source_id(
    selection: &SnapshotSelection<lettuce_conversations::ModelSelectionSnapshot>,
) -> Option<ModelProfileId> {
    match selection {
        SnapshotSelection::Inherited(model) | SnapshotSelection::Explicit(model) => {
            Some(model.source_id)
        }
        SnapshotSelection::Disabled => None,
    }
}

#[test]
fn a_conversation_mode_group_opens_empty_even_with_a_starting_scene() {
    let backend = backend();
    let database = backend.database();
    let first = seed_named_character(database, "Ada");
    let second = seed_named_character(database, "Bea");
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        Some(group_starting_scene("A quiet harbour at dawn.")),
        |_| {},
    );
    let plan = group_plan_for(
        database,
        &group_request(group_id, "group-conversation-scene"),
    );
    let details = group_details(&plan);
    assert_eq!(details.group.chat_mode, GroupChatModeSnapshot::Conversation);
    assert_eq!(details.group.scene, SnapshotSelection::Disabled);
    assert!(plan.initial_timeline.entries.is_empty());
}

#[test]
fn a_roleplay_group_materializes_one_trimmed_scene_message_from_the_selected_variant() {
    let backend = backend();
    let database = backend.database();
    let first = seed_named_character(database, "Ada");
    let second = seed_named_character(database, "Bea");
    let mut starting = group_starting_scene("  Base body.  ");
    let variant_id = SceneVariantId::new();
    starting.variants = vec![SceneVariant {
        id: variant_id,
        scene_id: starting.scene.id,
        ordinal: 0,
        content: SceneDocumentV1::new(vec![ScenePart::Text {
            text: "  Variant body.  ".into(),
        }])
        .expect("variant document"),
        direction: None,
        revision: Revision::INITIAL,
        created_at: TimestampMillis::new(1),
        updated_at: TimestampMillis::new(1),
    }];
    starting.scene.selected_variant_id = Some(variant_id);
    starting.scene.direction = Some("Keep it slow.".into());
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        Some(starting),
        |group| group.chat_mode = ChatMode::Roleplay,
    );
    let plan = group_plan_for(database, &group_request(group_id, "group-roleplay-scene"));
    let details = group_details(&plan);
    assert!(matches!(
        details.group.scene,
        SnapshotSelection::Inherited(_)
    ));
    assert_eq!(plan.initial_timeline.entries.len(), 1);
    let entry = &plan.initial_timeline.entries[0];
    assert_eq!(entry.role, MessageRole::Scene);
    assert_eq!(entry.author_participant_id, None);
    assert_eq!(entry.parts.len(), 1);
    assert_eq!(message_text(&entry.parts[0]), "Variant body.");
    assert!(matches!(
        entry.origin,
        InitialMessageOrigin::SelectedScene { .. }
    ));

    let launched = ConversationLaunchPlanner::new(database)
        .launch_group(&group_request(group_id, "group-roleplay-scene-launch"), NOW)
        .expect("launch");
    launched.value.validate().expect("aggregate validates");
    let timeline = ConversationReader::timeline_page(
        database,
        launched.value.conversation.id,
        launched.value.conversation.active_branch_id,
        &lettuce_types::PageRequest::default(),
    )
    .expect("timeline");
    assert_eq!(timeline.items.len(), 1);
    assert_eq!(timeline.items[0].message.role, MessageRole::Scene);
    assert!(matches!(
        timeline.items[0].initial_origin,
        Some(InitialMessageOrigin::SelectedScene { .. })
    ));
}

#[test]
fn a_blank_group_scene_is_selected_without_a_timeline_entry() {
    let backend = backend();
    let database = backend.database();
    let first = seed_named_character(database, "Ada");
    let second = seed_named_character(database, "Bea");
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        Some(group_starting_scene("   ")),
        |group| group.chat_mode = ChatMode::Roleplay,
    );
    let plan = group_plan_for(database, &group_request(group_id, "group-blank-scene"));
    match &group_details(&plan).group.scene {
        SnapshotSelection::Inherited(scene) => assert_eq!(scene.title, "Scene 1"),
        other => panic!("expected an inherited scene, got {other:?}"),
    }
    assert!(plan.initial_timeline.entries.is_empty());
}

#[test]
fn an_archived_group_scene_degrades_to_no_scene() {
    let backend = backend();
    let database = backend.database();
    let first = seed_named_character(database, "Ada");
    let second = seed_named_character(database, "Bea");
    let mut starting = group_starting_scene("A quiet harbour at dawn.");
    starting.scene.status = LifecycleStatus::Archived;
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        Some(starting),
        |group| group.chat_mode = ChatMode::Roleplay,
    );
    let plan = group_plan_for(database, &group_request(group_id, "group-archived-scene"));
    assert_eq!(
        group_details(&plan).group.scene,
        SnapshotSelection::Disabled
    );
    assert!(plan.initial_timeline.entries.is_empty());
}

#[test]
fn a_group_prompt_never_falls_back_to_a_member_direct_prompt() {
    let backend = backend();
    let database = backend.database();
    let direct_prompt = seed_prompt(database, "Member prompt", PromptPurpose::DirectChat);
    let first = seed_named_character_with(database, "Ada", |defaults| {
        defaults.direct_prompt_id = Some(direct_prompt);
    });
    let second = seed_named_character_with(database, "Bea", |defaults| {
        defaults.direct_prompt_id = Some(direct_prompt);
    });
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |_| {},
    );
    let plan = group_plan_for(
        database,
        &group_request(group_id, "group-prompt-tier-three"),
    );
    let details = group_details(&plan);
    match &details.group.prompt {
        SnapshotSelection::Inherited(prompt) => {
            assert_eq!(
                prompt.source_id,
                backend
                    .built_in_prompt_ids()
                    .get(BuiltInPromptId::GroupChat)
            );
            assert_eq!(prompt.purpose, PromptPurposeSnapshot::GroupConversational);
        }
        other => panic!("expected the built-in group prompt, got {other:?}"),
    }
    for snapshot in &details.group.members {
        assert_eq!(snapshot.prompt, SnapshotSelection::Disabled);
    }
}

#[test]
fn the_roleplay_built_in_prompt_is_used_when_the_group_sets_none() {
    let backend = backend();
    let database = backend.database();
    let first = seed_named_character(database, "Ada");
    let second = seed_named_character(database, "Bea");
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |group| group.chat_mode = ChatMode::Roleplay,
    );
    let plan = group_plan_for(database, &group_request(group_id, "group-prompt-roleplay"));
    match &group_details(&plan).group.prompt {
        SnapshotSelection::Inherited(prompt) => {
            assert_eq!(
                prompt.source_id,
                backend
                    .built_in_prompt_ids()
                    .get(BuiltInPromptId::GroupChatRoleplay)
            );
            assert_eq!(prompt.purpose, PromptPurposeSnapshot::GroupRoleplay);
        }
        other => panic!("expected the built-in roleplay prompt, got {other:?}"),
    }
}

#[test]
fn a_group_prompt_is_inherited_and_an_archived_one_degrades_to_the_built_in() {
    let backend = backend();
    let database = backend.database();
    let authored = seed_prompt(
        database,
        "Group prompt",
        PromptPurpose::GroupChatConversational,
    );
    let first = seed_named_character(database, "Ada");
    let second = seed_named_character(database, "Bea");
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |group| group.group_conversation_prompt_id = Some(authored),
    );
    let plan = group_plan_for(database, &group_request(group_id, "group-prompt-authored"));
    match &group_details(&plan).group.prompt {
        SnapshotSelection::Inherited(prompt) => assert_eq!(prompt.source_id, authored),
        other => panic!("expected the authored group prompt, got {other:?}"),
    }

    PromptRepository::archive(database, authored, Revision::INITIAL, NOW).expect("archive prompt");
    let degraded = group_plan_for(database, &group_request(group_id, "group-prompt-archived"));
    match &group_details(&degraded).group.prompt {
        SnapshotSelection::Inherited(prompt) => assert_eq!(
            prompt.source_id,
            backend
                .built_in_prompt_ids()
                .get(BuiltInPromptId::GroupChat)
        ),
        other => panic!("expected the built-in group prompt, got {other:?}"),
    }
}

#[test]
fn a_group_prompt_with_the_wrong_purpose_is_rejected() {
    let backend = backend();
    let database = backend.database();
    let wrong = seed_prompt(database, "Direct prompt", PromptPurpose::DirectChat);
    let first = seed_named_character(database, "Ada");
    let second = seed_named_character(database, "Bea");
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |group| group.group_conversation_prompt_id = Some(wrong),
    );
    assert_eq!(
        ConversationLaunchPlanner::new(database)
            .prepare_group(&group_request(group_id, "group-prompt-wrong"), NOW)
            .expect_err("wrong purpose"),
        ConversationLaunchError::PromptWrongPurpose { prompt_id: wrong }
    );
}

#[test]
fn member_ordinals_are_reindexed_from_gapped_authored_ordinals() {
    let first = CharacterId::new();
    let second = CharacterId::new();
    let reindexed = policy::ordered_members(&[member(first, 7), member(second, 3)]);
    assert_eq!(
        reindexed
            .iter()
            .map(|value| (value.character_id, value.ordinal))
            .collect::<Vec<_>>(),
        vec![(second, 0), (first, 1)]
    );
}

#[test]
fn a_group_needs_two_members_and_one_unmuted_member() {
    let first = CharacterId::new();
    let second = CharacterId::new();
    assert_eq!(
        policy::member_shape(&[member(first, 0)]),
        policy::MemberShape::TooFew
    );
    let mut muted_first = member(first, 0);
    muted_first.muted = true;
    let mut muted_second = member(second, 1);
    muted_second.muted = true;
    assert_eq!(
        policy::member_shape(&[muted_first.clone(), muted_second]),
        policy::MemberShape::AllMuted
    );
    assert_eq!(
        policy::member_shape(&[muted_first, member(second, 1)]),
        policy::MemberShape::Launchable
    );
    let at_bound: Vec<GroupMember> = (0..policy::MAX_GROUP_MEMBERS)
        .map(|ordinal| member(CharacterId::new(), ordinal as u32))
        .collect();
    assert_eq!(
        policy::member_shape(&at_bound),
        policy::MemberShape::Launchable
    );
    let mut over_bound = at_bound;
    over_bound.push(member(CharacterId::new(), policy::MAX_GROUP_MEMBERS as u32));
    assert_eq!(
        policy::member_shape(&over_bound),
        policy::MemberShape::TooMany
    );
}

#[test]
fn a_muted_member_stays_muted_and_enabled_in_the_launch() {
    let backend = backend();
    let database = backend.database();
    let first = seed_named_character(database, "Ada");
    let second = seed_named_character(database, "Bea");
    let mut muted = member(second, 1);
    muted.muted = true;
    let group_id = seed_group(database, vec![member(first, 0), muted], None, |_| {});
    let plan = group_plan_for(database, &group_request(group_id, "group-muted-member"));
    let details = group_details(&plan);
    assert_eq!(
        details
            .group
            .members
            .iter()
            .map(|value| (value.ordinal, value.enabled, value.muted))
            .collect::<Vec<_>>(),
        vec![(0, true, false), (1, true, true)]
    );
    assert_eq!(
        character_participants(&plan)
            .iter()
            .map(|participant| (participant.ordinal, participant.enabled, participant.muted))
            .collect::<Vec<_>>(),
        vec![(1, true, false), (2, true, true)]
    );
}

#[test]
fn an_archived_group_or_member_character_is_rejected() {
    let backend = backend();
    let database = backend.database();
    let first = seed_named_character(database, "Ada");
    let second = seed_named_character(database, "Bea");
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |_| {},
    );
    CharacterRepository::archive(database, second, Revision::INITIAL, NOW)
        .expect("archive member character");
    assert_eq!(
        ConversationLaunchPlanner::new(database)
            .prepare_group(&group_request(group_id, "group-archived-member"), NOW)
            .expect_err("archived member"),
        ConversationLaunchError::MemberCharacterArchived {
            character_id: second
        }
    );

    let archived_group = two_member_group(database);
    GroupRepository::archive(database, archived_group, Revision::INITIAL, NOW).expect("archive");
    assert_eq!(
        ConversationLaunchPlanner::new(database)
            .prepare_group(&group_request(archived_group, "group-archived"), NOW)
            .expect_err("archived group"),
        ConversationLaunchError::GroupArchived {
            group_id: archived_group
        }
    );

    let missing = GroupId::new();
    assert_eq!(
        ConversationLaunchPlanner::new(database)
            .prepare_group(&group_request(missing, "group-missing"), NOW)
            .expect_err("missing group"),
        ConversationLaunchError::GroupNotFound { group_id: missing }
    );
}

#[test]
fn member_models_prefer_the_override_then_the_character_default_and_never_the_app_default() {
    let backend = backend();
    let database = backend.database();
    let application = seed_model(database, ProviderProtocol::OpenAiCompatible, "openrouter");
    let character_model = seed_model(database, ProviderProtocol::Gemini, "gemini");
    let override_model = seed_model(database, ProviderProtocol::Anthropic, "anthropic");
    set_application_default_model(database, application);
    let first = seed_named_character_with(database, "Ada", |defaults| {
        defaults.model_profile_id = Some(character_model);
    });
    let second = seed_named_character(database, "Bea");
    let third = seed_named_character(database, "Cai");
    let mut overridden = member(second, 1);
    overridden.model_profile_override = Some(override_model);
    let group_id = seed_group(
        database,
        vec![member(first, 0), overridden, member(third, 2)],
        None,
        |_| {},
    );
    let plan = group_plan_for(database, &group_request(group_id, "group-member-models"));
    let details = group_details(&plan);
    match &details.group.members[0].model_override {
        SnapshotSelection::Inherited(model) => assert_eq!(model.source_id, character_model),
        other => panic!("expected the character default model, got {other:?}"),
    }
    match &details.group.members[1].model_override {
        SnapshotSelection::Explicit(model) => {
            assert_eq!(model.source_id, override_model);
            assert_eq!(model.provider_protocol, ProviderProtocol::Anthropic);
        }
        other => panic!("expected the member override model, got {other:?}"),
    }
    assert_eq!(
        details.group.members[2].model_override,
        SnapshotSelection::Disabled
    );
    match &details.group.model {
        SnapshotSelection::Inherited(model) => assert_eq!(model.source_id, application),
        other => panic!("expected the application default model, got {other:?}"),
    }
    assert_eq!(
        character_participants(&plan)
            .iter()
            .map(|participant| participant.model_selection.clone())
            .collect::<Vec<_>>(),
        details
            .group
            .members
            .iter()
            .map(|value| value.model_override.clone())
            .collect::<Vec<_>>()
    );
    let sources: Vec<SnapshotSource> = ConversationLaunchPlanner::new(database)
        .prepare_group(&group_request(group_id, "group-member-models"), NOW)
        .expect("prepare")
        .into_parts()
        .1
        .iter()
        .map(|draft| draft.source)
        .collect();
    assert_eq!(
        sources
            .iter()
            .filter(|source| matches!(source, SnapshotSource::Model(_)))
            .count(),
        3
    );
}

#[test]
fn a_group_without_an_application_default_model_has_no_group_model() {
    let backend = backend();
    let database = backend.database();
    let group_id = two_member_group(database);
    let plan = group_plan_for(database, &group_request(group_id, "group-model-disabled"));
    assert_eq!(
        group_details(&plan).group.model,
        SnapshotSelection::Disabled
    );
}

#[test]
fn a_shared_member_model_is_staged_once() {
    let backend = backend();
    let database = backend.database();
    let shared = seed_model(database, ProviderProtocol::Gemini, "gemini");
    let first = seed_named_character_with(database, "Ada", |defaults| {
        defaults.model_profile_id = Some(shared);
    });
    let second = seed_named_character_with(database, "Bea", |defaults| {
        defaults.model_profile_id = Some(shared);
    });
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |_| {},
    );
    let drafts = ConversationLaunchPlanner::new(database)
        .prepare_group(&group_request(group_id, "group-shared-model"), NOW)
        .expect("prepare")
        .into_parts()
        .1;
    assert_eq!(
        drafts
            .iter()
            .filter(|draft| matches!(draft.source, SnapshotSource::Model(_)))
            .count(),
        1
    );
}

#[test]
fn disabling_character_lorebooks_empties_member_scopes_but_keeps_the_group_and_persona_scopes() {
    let backend = backend();
    let database = backend.database();
    let group_book = seed_lorebook(database, "Group");
    let member_book = seed_lorebook(database, "Member");
    let persona_book = seed_lorebook(database, "Persona");
    let first = seed_named_character(database, "Ada");
    let second = seed_named_character(database, "Bea");
    CharacterLorebookBindingRepository::bind_character_lorebook(
        database,
        first,
        Revision::INITIAL,
        LorebookBindingCreate {
            lorebook_id: member_book,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind member book");
    let persona_id = seed_persona(database, "Traveller");
    PersonaLorebookBindingRepository::bind_persona_lorebook(
        database,
        persona_id,
        Revision::INITIAL,
        LorebookBindingCreate {
            lorebook_id: persona_book,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind persona book");

    let live = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |_| {},
    );
    GroupLorebookBindingRepository::bind_group_lorebook(
        database,
        live,
        Revision::INITIAL,
        LorebookBindingCreate {
            lorebook_id: group_book,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind group book");
    let mut request = group_request(live, "group-books-live");
    request.persona = LaunchSelection::Explicit(persona_id);
    let plan = group_plan_for(database, &request);
    let details = group_details(&plan);
    assert_eq!(
        lorebook_names(&details.group.lorebooks),
        Some(vec!["Group".to_owned()])
    );
    assert!(details.group.lorebooks.is_explicit());
    assert_eq!(
        lorebook_names(&details.group.members[0].lorebooks),
        Some(vec!["Member".to_owned()])
    );
    assert_eq!(
        lorebook_names(&details.group.members[1].lorebooks),
        Some(Vec::new())
    );
    match &details.group.persona {
        SnapshotSelection::Explicit(persona) => assert_eq!(
            lorebook_names(&persona.lorebooks),
            Some(vec!["Persona".to_owned()])
        ),
        other => panic!("expected an explicit persona, got {other:?}"),
    }

    let disabled = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |group| group.disable_character_lorebooks = true,
    );
    GroupLorebookBindingRepository::bind_group_lorebook(
        database,
        disabled,
        Revision::INITIAL,
        LorebookBindingCreate {
            lorebook_id: group_book,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind group book");
    let mut request = group_request(disabled, "group-books-disabled");
    request.persona = LaunchSelection::Explicit(persona_id);
    let plan = group_plan_for(database, &request);
    let details = group_details(&plan);
    assert!(details.group.disable_character_lorebook);
    for snapshot in &details.group.members {
        assert_eq!(snapshot.lorebooks, SnapshotSelection::Disabled);
    }
    assert_eq!(
        lorebook_names(&details.group.lorebooks),
        Some(vec!["Group".to_owned()])
    );
    match &details.group.persona {
        SnapshotSelection::Explicit(persona) => assert_eq!(
            lorebook_names(&persona.lorebooks),
            Some(vec!["Persona".to_owned()])
        ),
        other => panic!("expected an explicit persona, got {other:?}"),
    }
}

#[test]
fn an_archived_group_bound_lorebook_is_skipped() {
    let backend = backend();
    let database = backend.database();
    let kept = seed_lorebook(database, "Kept");
    let archived = seed_lorebook(database, "Archived");
    let group_id = two_member_group(database);
    let bound = GroupLorebookBindingRepository::bind_group_lorebook(
        database,
        group_id,
        Revision::INITIAL,
        LorebookBindingCreate {
            lorebook_id: kept,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind kept");
    GroupLorebookBindingRepository::bind_group_lorebook(
        database,
        group_id,
        bound.owner_revision,
        LorebookBindingCreate {
            lorebook_id: archived,
            target: BindingInsertionTarget::Append,
        },
        NOW,
    )
    .expect("bind archived");
    LorebookRepository::archive(database, archived, Revision::INITIAL, NOW).expect("archive book");
    let plan = group_plan_for(database, &group_request(group_id, "group-archived-book"));
    assert_eq!(
        lorebook_names(&group_details(&plan).group.lorebooks),
        Some(vec!["Kept".to_owned()])
    );
}

#[test]
fn group_persona_resolution_covers_explicit_inherited_and_disabled() {
    let backend = backend();
    let database = backend.database();
    let group_id = two_member_group(database);
    let persona_id = seed_persona(database, "Traveller");

    let mut explicit = group_request(group_id, "group-persona-explicit");
    explicit.persona = LaunchSelection::Explicit(persona_id);
    assert!(matches!(
        group_details(&group_plan_for(database, &explicit))
            .group
            .persona,
        SnapshotSelection::Explicit(_)
    ));

    let inherit = group_request(group_id, "group-persona-inherit-none");
    assert_eq!(
        group_details(&group_plan_for(database, &inherit))
            .group
            .persona,
        SnapshotSelection::Disabled
    );

    let default_revision = PersonaRepository::get_default_snapshot(database)
        .expect("default snapshot")
        .state
        .revision;
    PersonaRepository::set_default(database, persona_id, default_revision, NOW)
        .expect("set default");
    let inherited = group_request(group_id, "group-persona-inherited");
    assert!(matches!(
        group_details(&group_plan_for(database, &inherited))
            .group
            .persona,
        SnapshotSelection::Inherited(_)
    ));

    let mut disabled = group_request(group_id, "group-persona-disabled");
    disabled.persona = LaunchSelection::Disabled;
    assert_eq!(
        group_details(&group_plan_for(database, &disabled))
            .group
            .persona,
        SnapshotSelection::Disabled
    );
}

#[test]
fn a_blank_title_derivation_matches_the_legacy_cast_naming() {
    assert_eq!(
        policy::derive_group_title(&["Ada".into(), "Bea".into()], 1024),
        "Ada, Bea"
    );
    assert_eq!(
        policy::derive_group_title(&["Ada".into(), "Bea".into(), "Cai".into()], 1024),
        "Ada, Bea, Cai"
    );
    assert_eq!(
        policy::derive_group_title(
            &["Ada".into(), "Bea".into(), "Cai".into(), "Dee".into()],
            1024
        ),
        "Ada, Bea & 2 others"
    );
    assert!(policy::derive_group_title(&["x".repeat(4_000)], 1024).len() <= 1024);
}

#[test]
fn a_blank_group_title_falls_back_to_the_group_name() {
    let backend = backend();
    let database = backend.database();
    let first = seed_named_character(database, "Ada");
    let second = seed_named_character(database, "Bea");
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |group| group.name = "Harbour Crew".into(),
    );
    let mut blank = group_request(group_id, "group-title-blank");
    blank.title = "   ".into();
    assert_eq!(group_plan_for(database, &blank).title, "Harbour Crew");

    let mut empty = group_request(group_id, "group-title-empty");
    empty.title = String::new();
    assert_eq!(group_plan_for(database, &empty).title, "Harbour Crew");

    let authored = group_request(group_id, "group-title-authored");
    assert_eq!(group_plan_for(database, &authored).title, "Session");
}

#[test]
fn the_participant_policy_document_mirrors_the_character_participants() {
    let backend = backend();
    let database = backend.database();
    let model = seed_model(database, ProviderProtocol::Gemini, "gemini");
    let first = seed_named_character_with(database, "Ada", |defaults| {
        defaults.model_profile_id = Some(model);
    });
    let second = seed_named_character(database, "Bea");
    let mut muted = member(second, 1);
    muted.muted = true;
    let group_id = seed_group(database, vec![member(first, 0), muted], None, |_| {});
    let plan = group_plan_for(database, &group_request(group_id, "group-policy-mirror"));
    let details = group_details(&plan);
    let policy_members = &details.initial_participant_policy.members;
    let participants = character_participants(&plan);
    assert_eq!(policy_members.len(), participants.len());
    for (policy, participant) in policy_members.iter().zip(participants) {
        assert_eq!(policy.participant_id, participant.id);
        assert_eq!(policy.enabled, participant.enabled);
        assert_eq!(policy.muted, participant.muted);
        assert_eq!(policy.model_override, participant.model_selection);
    }
    assert_eq!(
        details.initial_participant_policy.revision,
        Revision::INITIAL
    );
    assert_eq!(details.initial_participant_policy.created_at, NOW);
    assert_eq!(details.initial_participant_policy.updated_at, NOW);
    assert_eq!(
        model_source_id(&details.group.members[0].model_override),
        Some(model)
    );
}

#[test]
fn the_group_memory_policy_lands_in_the_frozen_snapshot() {
    let backend = backend();
    let database = backend.database();
    let first = seed_named_character(database, "Ada");
    let second = seed_named_character(database, "Bea");
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |group| group.memory_policy = MemoryPolicy::Dynamic,
    );
    let plan = group_plan_for(database, &group_request(group_id, "group-memory"));
    match &group_details(&plan).group.memory {
        SnapshotSelection::Inherited(memory) => {
            assert_eq!(memory.mode, MemoryModeSnapshot::Dynamic);
            assert!(memory.selected_revision_ids.is_empty());
            assert!(memory.policy_ref.is_none());
        }
        other => panic!("expected inherited memory, got {other:?}"),
    }
}

#[test]
fn two_group_planner_runs_on_identical_state_are_byte_identical() {
    let backend = backend();
    let database = backend.database();
    let group_id = two_member_group(database);
    let launch = group_request(group_id, "group-determinism");
    assert_eq!(
        group_plan_for(database, &launch),
        group_plan_for(database, &launch)
    );
}

#[test]
fn an_identical_group_retry_replays_a_single_conversation() {
    let backend = backend();
    let database = backend.database();
    let group_id = two_member_group(database);
    let launch = group_request(group_id, "group-idempotent");
    let planner = ConversationLaunchPlanner::new(database);
    let first = planner.launch_group(&launch, NOW).expect("first launch");
    let second = planner
        .launch_group(&launch, TimestampMillis::new(2_000))
        .expect("replayed launch");
    assert_eq!(first.value.conversation.id, second.value.conversation.id);
    assert_eq!(first.operation.id, second.operation.id);
    assert_eq!(
        ConversationReader::page(
            database,
            &lettuce_conversations::ConversationQuery::default()
        )
        .expect("page")
        .items
        .len(),
        1
    );
}

#[test]
fn reusing_a_group_key_after_a_source_edit_conflicts() {
    let backend = backend();
    let database = backend.database();
    let group_id = two_member_group(database);
    let launch = group_request(group_id, "group-conflicting");
    let planner = ConversationLaunchPlanner::new(database);
    planner.launch_group(&launch, NOW).expect("first launch");
    GroupRepository::rename(
        database,
        group_id,
        Revision::INITIAL,
        "Renamed cast".into(),
        NOW,
    )
    .expect("rename group");
    assert_eq!(
        planner
            .launch_group(&launch, TimestampMillis::new(2_000))
            .expect_err("conflict"),
        ConversationLaunchError::CreateConflict
    );
}

#[test]
fn retrying_a_group_launch_after_the_group_is_archived_reports_the_existing_conversation() {
    let backend = backend();
    let database = backend.database();
    let group_id = two_member_group(database);
    let launch = group_request(group_id, "group-already-launched");
    let planner = ConversationLaunchPlanner::new(database);
    let created = planner.launch_group(&launch, NOW).expect("first launch");
    GroupRepository::archive(database, group_id, Revision::INITIAL, NOW).expect("archive group");
    assert_eq!(
        planner
            .launch_group(&launch, TimestampMillis::new(2_000))
            .expect_err("already launched"),
        ConversationLaunchError::AlreadyLaunched {
            conversation_id: created.value.conversation.id
        }
    );
}

#[test]
fn group_source_drift_between_the_two_reads_is_detected() {
    let group_id = GroupId::new();
    let character_id = CharacterId::new();
    let persona_id = PersonaId::new();
    assert_eq!(
        policy::detect_group_source_drift(
            (group_id, Revision::INITIAL, Revision::INITIAL),
            &[(character_id, Revision::INITIAL, Revision::INITIAL)],
            Some((persona_id, Revision::INITIAL, Revision::INITIAL)),
        ),
        None
    );
    assert_eq!(
        policy::detect_group_source_drift(
            (group_id, Revision::INITIAL, Revision::new(2)),
            &[],
            None,
        ),
        Some(SnapshotSource::Group(group_id))
    );
    assert_eq!(
        policy::detect_group_source_drift(
            (group_id, Revision::INITIAL, Revision::INITIAL),
            &[(character_id, Revision::INITIAL, Revision::new(4))],
            None,
        ),
        Some(SnapshotSource::Character(character_id))
    );
    assert_eq!(
        policy::detect_group_source_drift(
            (group_id, Revision::INITIAL, Revision::INITIAL),
            &[],
            Some((persona_id, Revision::INITIAL, Revision::new(3))),
        ),
        Some(SnapshotSource::Persona(persona_id))
    );
}

#[test]
fn a_companion_member_character_is_rejected() {
    let backend = backend();
    let database = backend.database();
    let first = seed_named_character(database, "Ada");
    let companion = seed_named_character_with(database, "Bea", |defaults| {
        defaults.interaction_mode = InteractionMode::Companion;
    });
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(companion, 1)],
        None,
        |_| {},
    );
    assert_eq!(
        ConversationLaunchPlanner::new(database)
            .prepare_group(&group_request(group_id, "group-companion-member"), NOW)
            .expect_err("companion member"),
        ConversationLaunchError::MemberCharacterCompanion {
            character_id: companion
        }
    );
}

#[test]
fn member_lorebook_scopes_are_bounded_in_aggregate() {
    let backend = backend();
    let database = backend.database();
    let first = seed_named_character(database, "Ada");
    let second = seed_named_character(database, "Bea");
    let per_member = policy::MAX_LAUNCH_LOREBOOKS / 2 + 1;
    for (slot, character_id) in [first, second].into_iter().enumerate() {
        let mut revision = Revision::INITIAL;
        for index in 0..per_member {
            let book = seed_lorebook(database, &format!("Book {slot}-{index}"));
            revision = CharacterLorebookBindingRepository::bind_character_lorebook(
                database,
                character_id,
                revision,
                LorebookBindingCreate {
                    lorebook_id: book,
                    target: BindingInsertionTarget::Append,
                },
                NOW,
            )
            .expect("bind")
            .owner_revision;
        }
    }
    assert!(!policy::lorebook_bound_exceeded(per_member));
    assert!(policy::lorebook_bound_exceeded(per_member * 2));
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |_| {},
    );
    assert_eq!(
        ConversationLaunchPlanner::new(database)
            .prepare_group(&group_request(group_id, "group-books-aggregate"), NOW)
            .expect_err("aggregate lorebook bound"),
        ConversationLaunchError::TooManyLorebooks {
            max: policy::MAX_LAUNCH_LOREBOOKS
        }
    );
}

#[test]
fn a_missing_built_in_group_prompt_is_reported_instead_of_being_skipped() {
    let database = database();
    let first = seed_named_character(&database, "Ada");
    let second = seed_named_character(&database, "Bea");
    let group_id = seed_group(
        &database,
        vec![member(first, 0), member(second, 1)],
        None,
        |_| {},
    );
    assert_eq!(
        ConversationLaunchPlanner::new(&database)
            .prepare_group(
                &group_request(group_id, "group-prompt-missing-built-in"),
                NOW
            )
            .expect_err("missing built-in prompt"),
        ConversationLaunchError::BuiltInPromptMissing {
            purpose: PromptPurpose::GroupChatConversational
        }
    );
}

#[test]
fn a_member_without_any_model_falls_back_to_the_group_model() {
    let backend = backend();
    let database = backend.database();
    let application = seed_model(database, ProviderProtocol::OpenAiCompatible, "openrouter");
    set_application_default_model(database, application);
    let first = seed_named_character(database, "Ada");
    let second = seed_named_character(database, "Bea");
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |_| {},
    );
    let plan = group_plan_for(database, &group_request(group_id, "group-model-fallback"));
    let details = group_details(&plan);
    for snapshot in &details.group.members {
        assert_eq!(snapshot.model_override, SnapshotSelection::Disabled);
    }
    match &details.group.model {
        SnapshotSelection::Inherited(model) => assert_eq!(model.source_id, application),
        other => panic!("expected the application default model, got {other:?}"),
    }
}

#[test]
fn a_broken_application_default_is_untouched_when_no_member_needs_it() {
    let backend = backend();
    let database = backend.database();
    let broken = seed_model_with(
        database,
        ProviderProtocol::Anthropic,
        "anthropic",
        ModelKind::Chat,
        false,
    );
    set_application_default_model(database, broken);
    let usable = seed_model(database, ProviderProtocol::Gemini, "gemini");
    let first = seed_named_character_with(database, "Ada", |defaults| {
        defaults.model_profile_id = Some(usable);
    });
    let second = seed_named_character(database, "Bea");
    let mut overridden = member(second, 1);
    overridden.model_profile_override = Some(usable);
    let group_id = seed_group(database, vec![member(first, 0), overridden], None, |_| {});
    let plan = group_plan_for(database, &group_request(group_id, "group-model-unneeded"));
    assert_eq!(
        group_details(&plan).group.model,
        SnapshotSelection::Disabled
    );
}

#[test]
fn a_broken_application_default_is_rejected_when_a_member_needs_it() {
    let backend = backend();
    let database = backend.database();
    let broken = seed_model_with(
        database,
        ProviderProtocol::Anthropic,
        "anthropic",
        ModelKind::Chat,
        false,
    );
    set_application_default_model(database, broken);
    let usable = seed_model(database, ProviderProtocol::Gemini, "gemini");
    let first = seed_named_character_with(database, "Ada", |defaults| {
        defaults.model_profile_id = Some(usable);
    });
    let second = seed_named_character(database, "Bea");
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |_| {},
    );
    let provider_account_id = ModelProfileRepository::get(database, broken)
        .expect("profile")
        .expect("profile exists")
        .provider_account_id;
    assert_eq!(
        ConversationLaunchPlanner::new(database)
            .prepare_group(&group_request(group_id, "group-model-needed"), NOW)
            .expect_err("disabled provider"),
        ConversationLaunchError::ProviderDisabled {
            provider_account_id
        }
    );
}

#[test]
fn a_group_scene_never_speaks_its_direction() {
    let mut scene = text_scene(CharacterId::new(), 0, "   ");
    scene.direction = Some("  Keep it slow.  ".into());
    assert_eq!(
        policy::resolve_scene_text(&scene, &[]),
        Some("Keep it slow.".to_owned())
    );
    assert_eq!(policy::resolve_group_scene_text(&scene, &[]), None);

    let backend = backend();
    let database = backend.database();
    let first = seed_named_character(database, "Ada");
    let second = seed_named_character(database, "Bea");
    let mut starting = group_starting_scene("   ");
    starting.scene.direction = Some("Keep it slow.".into());
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        Some(starting),
        |group| group.chat_mode = ChatMode::Roleplay,
    );
    let plan = group_plan_for(database, &group_request(group_id, "group-scene-direction"));
    assert!(plan.initial_timeline.entries.is_empty());
    match &group_details(&plan).group.scene {
        SnapshotSelection::Inherited(scene) => assert_eq!(scene.title, "Scene 1"),
        other => panic!("expected an inherited scene, got {other:?}"),
    }
}

#[test]
fn the_group_profile_persona_sits_between_the_request_and_the_application_default() {
    let backend = backend();
    let database = backend.database();
    let authored = seed_persona(database, "Cartographer");
    let application = seed_persona(database, "Traveller");
    let default_revision = PersonaRepository::get_default_snapshot(database)
        .expect("default snapshot")
        .state
        .revision;
    PersonaRepository::set_default(database, application, default_revision, NOW)
        .expect("set default");
    let first = seed_named_character(database, "Ada");
    let second = seed_named_character(database, "Bea");

    let explicit = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |group| group.persona = Selection::Explicit(authored),
    );
    match &group_details(&group_plan_for(
        database,
        &group_request(explicit, "group-persona-profile"),
    ))
    .group
    .persona
    {
        SnapshotSelection::Inherited(persona) => assert_eq!(persona.source_id, authored),
        other => panic!("expected the group profile persona, got {other:?}"),
    }

    let mut overridden = group_request(explicit, "group-persona-request-wins");
    overridden.persona = LaunchSelection::Explicit(application);
    match &group_details(&group_plan_for(database, &overridden))
        .group
        .persona
    {
        SnapshotSelection::Explicit(persona) => assert_eq!(persona.source_id, application),
        other => panic!("expected the requested persona, got {other:?}"),
    }

    let disabled = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |group| group.persona = Selection::Disabled,
    );
    assert_eq!(
        group_details(&group_plan_for(
            database,
            &group_request(disabled, "group-persona-profile-off")
        ))
        .group
        .persona,
        SnapshotSelection::Disabled
    );
}

#[test]
fn a_member_carries_its_own_group_prompt_for_the_launched_mode() {
    let backend = backend();
    let database = backend.database();
    let conversational = seed_prompt(
        database,
        "Member conversational",
        PromptPurpose::GroupChatConversational,
    );
    let roleplay = seed_prompt(
        database,
        "Member roleplay",
        PromptPurpose::GroupChatRoleplay,
    );
    let first = seed_named_character_with(database, "Ada", |defaults| {
        defaults.group_conversation_prompt_id = Some(conversational);
        defaults.group_roleplay_prompt_id = Some(roleplay);
    });
    let second = seed_named_character(database, "Bea");
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |_| {},
    );
    let plan = group_plan_for(database, &group_request(group_id, "group-member-prompt"));
    let details = group_details(&plan);
    match &details.group.members[0].prompt {
        SnapshotSelection::Inherited(prompt) => {
            assert_eq!(prompt.source_id, conversational);
            assert_eq!(prompt.purpose, PromptPurposeSnapshot::GroupConversational);
        }
        other => panic!("expected the member group prompt, got {other:?}"),
    }
    assert_eq!(details.group.members[1].prompt, SnapshotSelection::Disabled);

    let roleplay_group = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |group| group.chat_mode = ChatMode::Roleplay,
    );
    let plan = group_plan_for(
        database,
        &group_request(roleplay_group, "group-member-prompt-roleplay"),
    );
    match &group_details(&plan).group.members[0].prompt {
        SnapshotSelection::Inherited(prompt) => {
            assert_eq!(prompt.source_id, roleplay);
            assert_eq!(prompt.purpose, PromptPurposeSnapshot::GroupRoleplay);
        }
        other => panic!("expected the member roleplay prompt, got {other:?}"),
    }
}

#[test]
fn an_archived_member_prompt_degrades_while_dangling_and_mismatched_ones_error() {
    let backend = backend();
    let database = backend.database();
    let archived = seed_prompt(
        database,
        "Member archived",
        PromptPurpose::GroupChatConversational,
    );
    let first = seed_named_character_with(database, "Ada", |defaults| {
        defaults.group_conversation_prompt_id = Some(archived);
    });
    let second = seed_named_character(database, "Bea");
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |_| {},
    );
    PromptRepository::archive(database, archived, Revision::INITIAL, NOW).expect("archive prompt");
    let plan = group_plan_for(
        database,
        &group_request(group_id, "group-member-prompt-archived"),
    );
    assert_eq!(
        group_details(&plan).group.members[0].prompt,
        SnapshotSelection::Disabled
    );

    let dangling = lettuce_types::PromptDocumentId::new();
    let third = seed_named_character_with(database, "Cai", |defaults| {
        defaults.group_conversation_prompt_id = Some(dangling);
    });
    let dangling_group = seed_group(
        database,
        vec![member(second, 0), member(third, 1)],
        None,
        |_| {},
    );
    assert_eq!(
        ConversationLaunchPlanner::new(database)
            .prepare_group(
                &group_request(dangling_group, "group-member-prompt-dangling"),
                NOW
            )
            .expect_err("dangling member prompt"),
        ConversationLaunchError::PromptNotFound {
            prompt_id: dangling
        }
    );

    let wrong = seed_prompt(database, "Member direct", PromptPurpose::DirectChat);
    let fourth = seed_named_character_with(database, "Dee", |defaults| {
        defaults.group_conversation_prompt_id = Some(wrong);
    });
    let wrong_group = seed_group(
        database,
        vec![member(second, 0), member(fourth, 1)],
        None,
        |_| {},
    );
    assert_eq!(
        ConversationLaunchPlanner::new(database)
            .prepare_group(
                &group_request(wrong_group, "group-member-prompt-wrong"),
                NOW
            )
            .expect_err("mismatched member prompt"),
        ConversationLaunchError::PromptWrongPurpose { prompt_id: wrong }
    );
}

#[test]
fn one_prompt_document_shared_by_members_and_the_group_is_staged_once() {
    let backend = backend();
    let database = backend.database();
    let shared = seed_prompt(
        database,
        "Shared group prompt",
        PromptPurpose::GroupChatConversational,
    );
    let first = seed_named_character_with(database, "Ada", |defaults| {
        defaults.group_conversation_prompt_id = Some(shared);
    });
    let second = seed_named_character_with(database, "Bea", |defaults| {
        defaults.group_conversation_prompt_id = Some(shared);
    });
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |group| group.group_conversation_prompt_id = Some(shared),
    );
    let request = group_request(group_id, "group-prompt-shared");
    let plan = group_plan_for(database, &request);
    let details = group_details(&plan);
    let group_prompt = match &details.group.prompt {
        SnapshotSelection::Inherited(prompt) => prompt.clone(),
        other => panic!("expected the shared group prompt, got {other:?}"),
    };
    for snapshot in &details.group.members {
        match &snapshot.prompt {
            SnapshotSelection::Inherited(prompt) => assert_eq!(prompt, &group_prompt),
            other => panic!("expected the shared member prompt, got {other:?}"),
        }
    }
    let drafts = ConversationLaunchPlanner::new(database)
        .prepare_group(&request, NOW)
        .expect("prepare")
        .into_parts()
        .1;
    assert_eq!(
        drafts
            .iter()
            .filter(|draft| draft.source == SnapshotSource::Prompt(shared))
            .count(),
        1
    );
}

#[test]
fn changing_a_member_prompt_conflicts_on_the_same_operation_key() {
    let backend = backend();
    let database = backend.database();
    let prompt_id = seed_prompt(
        database,
        "Member conversational",
        PromptPurpose::GroupChatConversational,
    );
    let first = seed_named_character(database, "Ada");
    let second = seed_named_character(database, "Bea");
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |_| {},
    );
    let launch = group_request(group_id, "group-member-prompt-digest");
    let planner = ConversationLaunchPlanner::new(database);
    planner.launch_group(&launch, NOW).expect("first launch");

    let mut defaults = CharacterRepository::get(database, first)
        .expect("character")
        .expect("character exists")
        .character
        .defaults;
    defaults.group_conversation_prompt_id = Some(prompt_id);
    CharacterRepository::update_defaults(database, first, Revision::INITIAL, defaults, NOW)
        .expect("update defaults");
    assert_eq!(
        planner
            .launch_group(&launch, TimestampMillis::new(2_000))
            .expect_err("conflict"),
        ConversationLaunchError::CreateConflict
    );
}

#[test]
fn a_group_conversation_with_member_prompts_hydrates_from_the_database() {
    let backend = backend();
    let database = backend.database();
    let member_prompt = seed_prompt(
        database,
        "Member conversational",
        PromptPurpose::GroupChatConversational,
    );
    let group_prompt = seed_prompt(
        database,
        "Group conversational",
        PromptPurpose::GroupChatConversational,
    );
    let first = seed_named_character_with(database, "Ada", |defaults| {
        defaults.group_conversation_prompt_id = Some(member_prompt);
    });
    let second = seed_named_character(database, "Bea");
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |group| group.group_conversation_prompt_id = Some(group_prompt),
    );
    let created = ConversationLaunchPlanner::new(database)
        .launch_group(&group_request(group_id, "group-member-prompt-hydrate"), NOW)
        .expect("launch");
    created.value.validate().expect("aggregate validates");
    let hydrated = ConversationReader::get(database, created.value.conversation.id)
        .expect("hydrate conversation");
    let details = match &hydrated.conversation.kind {
        ConversationKind::Group(details) => details,
        ConversationKind::Direct(_) => panic!("group launch hydrated as a direct conversation"),
    };
    match &details.group.members[0].prompt {
        SnapshotSelection::Inherited(prompt) => assert_eq!(prompt.source_id, member_prompt),
        other => panic!("expected the member prompt to survive hydration, got {other:?}"),
    }
    assert_eq!(details.group.members[1].prompt, SnapshotSelection::Disabled);
    match &details.group.prompt {
        SnapshotSelection::Inherited(prompt) => assert_eq!(prompt.source_id, group_prompt),
        other => panic!("expected the group prompt to survive hydration, got {other:?}"),
    }
}

#[test]
fn the_app_backend_exposes_the_group_launch() {
    let backend = backend();
    let group_id = two_member_group(backend.database());
    let result = backend
        .launch_group_conversation(&group_request(group_id, "backend-group-launch"), NOW)
        .expect("launch");
    result.value.validate().expect("aggregate validates");
    assert_eq!(result.value.conversation.participants.len(), 3);
    assert_eq!(
        result.value.conversation.id,
        super::identity::launch_conversation_id(&key("backend-group-launch"))
    );
}
use std::collections::VecDeque;
use std::sync::Mutex;
