use super::*;

use lettuce_context::PromptVariable;

fn companion_prompt_group(
    operation_key: &str,
    companion: bool,
) -> (
    AppBackend,
    lettuce_conversations::Conversation,
    CharacterId,
    CharacterId,
) {
    let backend = backend();
    let database = backend.database();
    let model_id = seed_model(
        database,
        ProviderProtocol::Ollama,
        "group-companion-prompt-model",
    );
    set_application_default_model(database, model_id);
    let first = seed_named_character_with(database, "Ada", |defaults| {
        defaults.model_profile_id = Some(model_id);
    });
    let second = seed_named_character_with(database, "Bea", |defaults| {
        defaults.model_profile_id = Some(model_id);
    });
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |group| group.companion_mode = companion,
    );
    let conversation = ConversationLaunchPlanner::new(database)
        .launch_group(&group_request(group_id, operation_key), NOW)
        .expect("launch group companion prompt")
        .value
        .conversation;
    (backend, conversation, first, second)
}

fn selected_speaker(
    participant_id: ConversationParticipantId,
) -> lettuce_conversations::SelectedSpeakerDecision {
    lettuce_conversations::SelectedSpeakerDecision {
        participant_id,
        method: lettuce_conversations::SpeakerDecisionMethod::Explicit,
        fallback: lettuce_conversations::SpeakerFallback::None,
        reference: None,
        rationale_summary: None,
        decision_model: None,
        usage_event_id: None,
    }
}

fn finalize_group_reply(
    database: &Database,
    started: &lettuce_conversations::BeginGeneration,
    model: lettuce_conversations::ModelSelectionSnapshot,
    text: &str,
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
                parts: vec![MessagePart::Text { text: text.into() }],
                ordinal: 0,
                model,
                replay: None,
                outcome: GenerationCheckpointEvent::Completed,
                scene_follow_up: None,
            },
            UsageEventId::new(),
            TimestampMillis::new(now + 10),
        )
        .expect("finalize turn")
}

#[tokio::test]
async fn group_companion_prompt_renders_the_speakers_state_and_member_stances() {
    let (backend, conversation, first, second) =
        companion_prompt_group("group-companion-prompt-live", true);
    let database = backend.database();
    let engine = group_companion_engine();
    let coordinator = CompanionTurnCoordinator::new(database, Some(&engine));
    let sent = coordinator
        .begin_send(
            &direct_send_command(&conversation, "group-companion-prompt-user", "Hello Ada."),
            TimestampMillis::new(NOW.get() + 1),
            &CancellationToken::new(),
        )
        .expect("send");
    cancel_started_group_turn(
        database,
        &sent.value,
        "group-companion-prompt",
        NOW.get() + 2,
    );
    let current = ConversationReader::get(database, conversation.id)
        .expect("conversation after send")
        .conversation;
    let second_speaker = group_companion_participant(&current, second);
    let continued = coordinator
        .begin_continue(
            &group_companion_continue_command(
                &current,
                second_speaker,
                "group-companion-prompt-reply",
            ),
            TimestampMillis::new(NOW.get() + 4),
            &CancellationToken::new(),
        )
        .expect("continue as Bea");
    let model = group_companion_participant_model(&current, second_speaker);
    let finalized = finalize_group_reply(
        database,
        &continued.value,
        model,
        "Ada, come and look at this.",
        "group-companion-prompt-reply",
        NOW.get() + 5,
    );
    let assistant_message_id = finalized.value.assistant_message.id;
    let current = ConversationReader::get(database, conversation.id)
        .expect("conversation after reply")
        .conversation;
    let first_speaker = group_companion_participant(&current, first);
    let witnessing = coordinator
        .begin_continue(
            &group_companion_continue_command(
                &current,
                first_speaker,
                "group-companion-prompt-witness",
            ),
            TimestampMillis::new(NOW.get() + 6),
            &CancellationToken::new(),
        )
        .expect("witness Bea's reply");
    cancel_started_group_turn(
        database,
        &witnessing.value,
        "group-companion-prompt-witness",
        NOW.get() + 7,
    );
    let witness_checkpoint = database
        .checkpoints_for_message(conversation.id, assistant_message_id)
        .expect("assistant checkpoints")
        .into_iter()
        .find(|checkpoint| checkpoint.character_id == first)
        .expect("Ada witness checkpoint");
    assert_eq!(witness_checkpoint.member_relationships.len(), 1);
    assert_eq!(
        witness_checkpoint.member_relationships[0].target_character_id,
        second
    );
    let member_prompt = prompt_with_text(
        database,
        "Member group prompt",
        PromptPurpose::GroupChatRoleplay,
        "Member group voice",
    );
    let character = CharacterRepository::get(database, first)
        .expect("character")
        .expect("exists")
        .character;
    let mut defaults = character.defaults.clone();
    defaults.group_roleplay_prompt_id = Some(member_prompt);
    CharacterRepository::update_defaults(database, first, character.revision, defaults, NOW)
        .expect("member group prompt");
    let mut request = context_request_for(database, conversation.id, assistant_message_id);
    request.operation = lettuce_conversations::GenerationOperation::Continue;
    request.selected_speaker = Some(selected_speaker(first_speaker));
    request.reference_time = TimestampMillis::new(NOW.get() + 8);
    let (prompt, text) = assembled_prompt_with_text(database, request).await;
    assert_eq!(
        prompt.map(|(id, _)| id),
        Some(
            backend
                .built_in_prompt_ids()
                .get(BuiltInPromptId::GroupCompanion)
        ),
    );
    assert!(text.contains("Current Ada <-> "));
    assert!(text.contains("Current Ada <-> Bea relationship stance:"));
    assert!(!text.contains("Current Bea <-> "));
}

#[tokio::test]
async fn group_companion_prompt_without_checkpoints_renders_the_initial_state() {
    let (backend, conversation, first, _) =
        companion_prompt_group("group-companion-prompt-initial", true);
    let database = backend.database();
    let sent = ConversationRepository::begin_send(
        database,
        &direct_send_command(
            &conversation,
            "group-companion-prompt-initial-user",
            "Hello everyone.",
        ),
        TimestampMillis::new(NOW.get() + 1),
    )
    .expect("send");
    let user_message_id = group_companion_user_message_id(&sent.value);
    let current = ConversationReader::get(database, conversation.id)
        .expect("conversation after send")
        .conversation;
    let first_speaker = group_companion_participant(&current, first);
    let mut request = context_request_for(database, conversation.id, user_message_id);
    request.selected_speaker = Some(selected_speaker(first_speaker));
    request.reference_time = TimestampMillis::new(NOW.get() + 2);
    let (prompt, text) = assembled_prompt_with_text(database, request).await;
    assert_eq!(
        prompt.map(|(id, _)| id),
        Some(
            backend
                .built_in_prompt_ids()
                .get(BuiltInPromptId::GroupCompanion)
        ),
    );
    assert!(text.contains("tracked 0 user interaction"));
    assert!(text.contains("has no particular history with anyone else here yet"));
}

#[tokio::test]
async fn group_prompt_without_companion_mode_keeps_the_roleplay_built_in() {
    let (backend, conversation, first, _) =
        companion_prompt_group("group-prompt-companion-off", false);
    let database = backend.database();
    let sent = ConversationRepository::begin_send(
        database,
        &direct_send_command(
            &conversation,
            "group-prompt-companion-off-user",
            "Hello everyone.",
        ),
        TimestampMillis::new(NOW.get() + 1),
    )
    .expect("send");
    let user_message_id = group_companion_user_message_id(&sent.value);
    let current = ConversationReader::get(database, conversation.id)
        .expect("conversation after send")
        .conversation;
    let first_speaker = group_companion_participant(&current, first);
    let mut request = context_request_for(database, conversation.id, user_message_id);
    request.selected_speaker = Some(selected_speaker(first_speaker));
    request.reference_time = TimestampMillis::new(NOW.get() + 2);
    let (prompt, text) = assembled_prompt_with_text(database, request).await;
    assert_eq!(
        prompt.map(|(id, _)| id),
        Some(
            backend
                .built_in_prompt_ids()
                .get(BuiltInPromptId::GroupChatRoleplay)
        ),
    );
    assert!(!text.contains("Only You Know This"));
}

#[test]
fn group_prompt_companion_launch_snapshot_records_the_companion_built_in() {
    let backend = backend();
    let database = backend.database();
    let first = seed_named_character(database, "Ada");
    let second = seed_named_character(database, "Bea");
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |group| group.companion_mode = true,
    );
    let plan = group_plan_for(
        database,
        &group_request(group_id, "group-prompt-companion-snapshot"),
    );
    match &group_details(&plan).group.prompt {
        SnapshotSelection::Inherited(prompt) => assert_eq!(
            prompt.source_id,
            backend
                .built_in_prompt_ids()
                .get(BuiltInPromptId::GroupCompanion)
        ),
        other => panic!("expected the companion built-in, got {other:?}"),
    }
}

#[test]
fn group_companion_prompt_ignores_the_app_default_prompt() {
    let backend = backend();
    let database = backend.database();
    assert_eq!(
        policy::group_companion_prompt(database)
            .expect("companion prompt")
            .map(|document| document.id),
        Some(
            backend
                .built_in_prompt_ids()
                .get(BuiltInPromptId::GroupCompanion)
        )
    );
    let app_default = prompt_with_text(
        database,
        "App group",
        PromptPurpose::GroupChatRoleplay,
        "App group voice",
    );
    set_app_default_prompt(database, Some(app_default));
    assert_eq!(
        policy::group_companion_prompt(database)
            .expect("companion prompt with an app default")
            .map(|document| document.id),
        Some(
            backend
                .built_in_prompt_ids()
                .get(BuiltInPromptId::GroupCompanion)
        )
    );
}

#[tokio::test]
async fn group_companion_prompt_renders_perspective_and_private_memory_layers() {
    let (backend, conversation, first, _) =
        companion_prompt_group("group-companion-memory-layers", true);
    let database = backend.database();
    let sent = ConversationRepository::begin_send(
        database,
        &direct_send_command(
            &conversation,
            "group-companion-memory-layers-user",
            "Hello everyone.",
        ),
        TimestampMillis::new(NOW.get() + 1),
    )
    .expect("send");
    let user_message_id = group_companion_user_message_id(&sent.value);
    let current = ConversationReader::get(database, conversation.id)
        .expect("conversation after send")
        .conversation;
    let first_speaker = group_companion_participant(&current, first);
    let mut request = context_request_for(database, conversation.id, user_message_id);
    request.selected_speaker = Some(selected_speaker(first_speaker));
    request.reference_time = TimestampMillis::new(NOW.get() + 2);
    request.memory = Some(lettuce_conversations::MemoryContribution {
        attribution: lettuce_conversations::MemoryAttribution {
            revision_id: lettuce_types::MemoryRevisionId::new(),
        },
        summary: None,
        key_memories: vec![lettuce_conversations::MemoryPromptLine {
            memory_id: None,
            match_score_basis_points: None,
            text: "The group met at the harbor.".into(),
            observed: None,
        }],
        perspective_memories: vec![lettuce_conversations::MemoryPromptLine {
            memory_id: None,
            match_score_basis_points: None,
            text: "Ada thought Bea was hiding something.".into(),
            observed: None,
        }],
        private_memories: vec![lettuce_conversations::MemoryPromptLine {
            memory_id: None,
            match_score_basis_points: None,
            text: "Ada told the user a secret.".into(),
            observed: None,
        }],
    });
    let (_, text) = assembled_prompt_with_text(database, request).await;
    assert!(text.contains("The group met at the harbor."));
    assert!(text.contains("## What You Make of This Group"));
    assert!(text.contains("Ada thought Bea was hiding something."));
    assert!(text.contains("## Between You and "));
    assert!(text.contains("Ada told the user a secret."));
}

struct PreviewInference;

#[async_trait::async_trait]
impl InferencePort for PreviewInference {
    async fn run(&self, _request: InferenceRequest) -> Result<InferenceOutcome, PortError> {
        Err(PortError::Empty)
    }
}

struct PreviewReplyMedia;

impl crate::ReplyMediaStore for PreviewReplyMedia {
    fn store_reply_image(
        &self,
        _asset_id: lettuce_types::AssetId,
        _origin: crate::ReplyImageOrigin,
        _bytes: &[u8],
    ) -> Result<lettuce_types::AssetId, lettuce_media::MediaStoreError> {
        Err(lettuce_media::MediaStoreError::CatalogFailure {
            reason: "preview does not store reply images",
        })
    }

    fn discard_reply_image(
        &self,
        _asset_id: lettuce_types::AssetId,
        _now: lettuce_types::TimestampMillis,
    ) {
    }
}

fn companion_memory_group(
    operation_key: &str,
    personal: bool,
) -> (
    AppBackend,
    lettuce_conversations::Conversation,
    CharacterId,
    CharacterId,
    MessageId,
) {
    let backend = backend();
    let database = backend.database();
    let stored = GlobalSettingsStore::load(database).expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.enabled = true;
    settings.dynamic_memory.retrieval_limit = 1;
    settings.dynamic_memory.min_similarity_basis_points = Some(5_000);
    GlobalSettingsStore::save(
        database,
        settings,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("enable group retrieval");
    let model_id = seed_model(
        database,
        ProviderProtocol::Ollama,
        &format!("{operation_key}-model"),
    );
    set_application_default_model(database, model_id);
    let first = seed_named_character_with(database, "Ada", |defaults| {
        defaults.model_profile_id = Some(model_id);
        defaults.interaction_mode = InteractionMode::Companion;
        defaults.companion_soul = Some(lettuce_companions::CompanionSoulConfig {
            share_memory_across_chats: true,
            ..Default::default()
        });
    });
    let second = seed_named_character_with(database, "Bea", |defaults| {
        defaults.model_profile_id = Some(model_id);
        defaults.interaction_mode = InteractionMode::Companion;
        defaults.companion_soul = Some(lettuce_companions::CompanionSoulConfig::default());
    });
    let group_id = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |group| {
            group.companion_mode = true;
            group.companion_personal_memories = personal;
            group.memory_policy = MemoryPolicy::Dynamic;
        },
    );
    let conversation = ConversationLaunchPlanner::new(database)
        .launch_group(&group_request(group_id, operation_key), NOW)
        .expect("launch group companion memory")
        .value
        .conversation;
    let engine = group_companion_engine();
    let coordinator = CompanionTurnCoordinator::new(database, Some(&engine));
    let sent = coordinator
        .begin_send(
            &direct_send_command(
                &conversation,
                &format!("{operation_key}-send"),
                "Hello Ada and Bea.",
            ),
            TimestampMillis::new(NOW.get() + 1),
            &CancellationToken::new(),
        )
        .expect("send group companion message");
    let user_message_id = group_companion_user_message_id(&sent.value);
    cancel_started_group_turn(database, &sent.value, operation_key, NOW.get() + 2);
    let conversation = ConversationReader::get(database, conversation.id)
        .expect("conversation after send")
        .conversation;
    (backend, conversation, first, second, user_message_id)
}

fn seed_axis_zero_memory(
    database: &Database,
    space_id: MemorySpaceId,
    text: &str,
    source_message_id: Option<MessageId>,
) {
    let stored = MemoryRepository::get(database, space_id)
        .expect("memory")
        .expect("memory space");
    let id = MemoryId::new();
    let item = MemoryItem {
        id,
        short_id: lettuce_memory::MemoryShortId::derived(id),
        text: text.into(),
        origin: lettuce_memory::MemoryOrigin::Model,
        category: Some(MemoryCategory::WorldDetail),
        source_message_id,
        source_role: source_message_id.map(|_| MessageRole::User),
        observed_at: None,
        observed_time_precision: None,
        superseded_by: None,
        superseded_at: None,
        supersedes: Vec::new(),
        token_count: Some(6),
        is_cold: false,
        is_pinned: false,
        importance: Score::FULL,
        persistence_importance: Score::FULL,
        prompt_importance: Score::FULL,
        volatility: Score::LEGACY_VOLATILITY,
        access_count: 0,
        created_at: TimestampMillis::new(NOW.get() + 3),
        last_accessed_at: TimestampMillis::new(NOW.get() + 3),
    };
    MemoryRepository::compare_and_apply(
        database,
        MemoryChangeSet {
            space_id,
            expected_revision: stored.revision,
            items: vec![item],
        },
    )
    .expect("seed memory");
    let mut values = vec![0.0; 128];
    values[0] = 1.0;
    lettuce_embeddings::MemoryEmbeddingRepository::put_ready(
        database,
        lettuce_embeddings::MemoryEmbeddingProjection {
            space_id,
            memory_id: id,
            source_text: text.into(),
            vector: EmbeddingVector {
                source_revision: "scenario-v1".into(),
                values,
            },
            dimensions: lettuce_embeddings::EmbeddingDimensions::D128,
            updated_at: TimestampMillis::new(NOW.get() + 3),
        },
    )
    .expect("seed projection");
}

fn seed_companion_pool(
    database: &Database,
    character_id: CharacterId,
    operation_key: &str,
    text: &str,
) -> MemorySpaceId {
    let direct = ConversationLaunchPlanner::new(database)
        .launch_direct(&request(character_id, operation_key), NOW)
        .expect("launch companion direct chat")
        .value
        .conversation;
    let pool = MemoryRepository::get_for_branch(database, direct.id, direct.active_branch_id)
        .expect("companion pool")
        .expect("companion pool space")
        .id;
    seed_axis_zero_memory(database, pool, text, None);
    pool
}

fn hide_ada_from_user_message_checkpoints(
    database: &Database,
    conversation: &lettuce_conversations::Conversation,
    user_message_id: MessageId,
    bea: CharacterId,
) {
    let bea_checkpoint = database
        .checkpoints_for_message(conversation.id, user_message_id)
        .expect("user message checkpoints")
        .into_iter()
        .find(|checkpoint| checkpoint.character_id == bea)
        .expect("Bea checkpoint");
    let prepared = lettuce_companions::PreparedGroupCompanionTurn {
        conversation_id: conversation.id,
        persona_id: None,
        members: Vec::new(),
        applied: vec![(
            user_message_id,
            lettuce_companions::GroupCompanionStep {
                checkpoints: vec![bea_checkpoint],
                persona_relationships: Vec::new(),
            },
        )],
        states: std::collections::BTreeMap::new(),
        persona_relationships: std::collections::BTreeMap::new(),
        persona_revisions: std::collections::BTreeMap::new(),
        user_event: None,
    };
    let current = ConversationReader::get(database, conversation.id)
        .expect("conversation before checkpoint replacement")
        .conversation;
    let speaker = group_companion_participant(&current, bea);
    let command =
        group_companion_continue_command(&current, speaker, "group-companion-memory-hide-ada");
    lettuce_companions::GroupCompanionTurnWriter::begin_group_companion_continue(
        database,
        &command,
        prepared,
        TimestampMillis::new(NOW.get() + 4),
    )
    .expect("replace user message checkpoints");
}

fn mark_member_left_audience(
    database: &Database,
    conversation: &lettuce_conversations::Conversation,
    message_id: MessageId,
    member: CharacterId,
    _speaker: CharacterId,
) {
    let now = TimestampMillis::new(NOW.get() + 4);
    let result = database
        .commit_api_operation(
            "group_message_audience_set",
            "group-companion-audience-left",
            "group-companion-audience-left-digest",
            now,
            |scope| {
                scope
                    .apply_group_audience_correction(
                        conversation.id,
                        conversation.active_branch_id,
                        message_id,
                        member,
                        false,
                        lettuce_memory::AudienceScope::Onward,
                        now,
                    )
                    .map(|(outcome, _)| match outcome {
                        lettuce_database::AudienceCorrectionOutcome::Applied => 0_u8,
                        lettuce_database::AudienceCorrectionOutcome::AlreadyApplied => 1,
                        lettuce_database::AudienceCorrectionOutcome::Invalid => 2,
                        lettuce_database::AudienceCorrectionOutcome::UserSet => 3,
                    })
            },
        )
        .expect("mark member away");
    assert_eq!(result, 0);
}

async fn preview_memory_values(
    backend: &AppBackend,
    conversation_id: lettuce_types::ConversationId,
    speaker: CharacterId,
) -> (String, String, String) {
    let database = backend.database();
    let engine = ScenarioEmbeddingEngine;
    let inference = PreviewInference;
    let media = PreviewReplyMedia;
    let runner = backend.prepared_conversation_generation_runner(&engine, &inference, &media);
    let document = policy::group_companion_prompt(database)
        .expect("companion prompt lookup")
        .expect("companion prompt");
    let (context, _) = runner
        .prompt_preview_context(
            conversation_id,
            &document,
            Some(speaker),
            TimestampMillis::new(NOW.get() + 10),
        )
        .await
        .expect("prompt preview");
    (
        context.values.key_memories.clone(),
        context
            .values
            .purpose_values
            .get(&PromptVariable::CompanionPerspectiveMemories)
            .cloned()
            .unwrap_or_default(),
        context
            .values
            .purpose_values
            .get(&PromptVariable::CompanionPrivateMemories)
            .cloned()
            .unwrap_or_default(),
    )
}

#[tokio::test]
async fn group_companion_perspective_and_pool_memories_stay_with_their_member() {
    let (backend, conversation, ada, bea, _) =
        companion_memory_group("group-companion-memory-owner", true);
    let database = backend.database();
    let ada_space = database
        .ensure_perspective_space(conversation.id, conversation.active_branch_id, ada)
        .expect("Ada perspective space");
    seed_axis_zero_memory(
        database,
        ada_space,
        "Ada thought Bea was hiding something.",
        None,
    );
    let bea_space = database
        .ensure_perspective_space(conversation.id, conversation.active_branch_id, bea)
        .expect("Bea perspective space");
    seed_axis_zero_memory(database, bea_space, "Bea thought Ada was bluffing.", None);
    seed_companion_pool(
        database,
        ada,
        "group-companion-memory-pool",
        "Ada told the user a secret.",
    );

    let (_, ada_perspective, ada_private) =
        preview_memory_values(&backend, conversation.id, ada).await;
    assert!(ada_perspective.contains("Ada thought Bea was hiding something."));
    assert!(ada_private.contains("Ada told the user a secret."));
    assert!(!ada_perspective.contains("Bea thought Ada was bluffing."));

    let (_, bea_perspective, bea_private) =
        preview_memory_values(&backend, conversation.id, bea).await;
    assert!(bea_perspective.contains("Bea thought Ada was bluffing."));
    assert!(!bea_perspective.contains("Ada thought Bea was hiding something."));
    assert!(!bea_private.contains("Ada told the user a secret."));
}

#[tokio::test]
async fn group_companion_personal_memories_toggle_hides_the_pool() {
    let (backend, conversation, ada, _, _) =
        companion_memory_group("group-companion-memory-personal-off", false);
    let database = backend.database();
    let ada_space = database
        .ensure_perspective_space(conversation.id, conversation.active_branch_id, ada)
        .expect("Ada perspective space");
    seed_axis_zero_memory(
        database,
        ada_space,
        "Ada thought Bea was hiding something.",
        None,
    );
    seed_companion_pool(
        database,
        ada,
        "group-companion-memory-personal-off-pool",
        "Ada told the user a secret.",
    );

    let (_, perspective, private) = preview_memory_values(&backend, conversation.id, ada).await;
    assert!(perspective.contains("Ada thought Bea was hiding something."));
    assert!(!private.contains("Ada told the user a secret."));
}

#[tokio::test]
async fn group_companion_shared_memory_visibility_follows_checkpoints() {
    let (backend, conversation, ada, bea, user_message_id) =
        companion_memory_group("group-companion-memory-visibility", false);
    let database = backend.database();
    let shared =
        MemoryRepository::get_for_branch(database, conversation.id, conversation.active_branch_id)
            .expect("shared memory")
            .expect("shared space");
    seed_axis_zero_memory(
        database,
        shared.id,
        "The group met at the harbor.",
        Some(user_message_id),
    );
    hide_ada_from_user_message_checkpoints(database, &conversation, user_message_id, bea);

    let (ada_keys, _, _) = preview_memory_values(&backend, conversation.id, ada).await;
    assert!(!ada_keys.contains("The group met at the harbor."));
    let (bea_keys, _, _) = preview_memory_values(&backend, conversation.id, bea).await;
    assert!(bea_keys.contains("The group met at the harbor."));
}

#[tokio::test]
async fn group_companion_shared_memory_visibility_follows_the_audience() {
    let (backend, conversation, ada, bea, user_message_id) =
        companion_memory_group("group-companion-memory-audience", false);
    let database = backend.database();
    let shared =
        MemoryRepository::get_for_branch(database, conversation.id, conversation.active_branch_id)
            .expect("shared memory")
            .expect("shared space");
    seed_axis_zero_memory(
        database,
        shared.id,
        "The group met at the harbor.",
        Some(user_message_id),
    );
    mark_member_left_audience(database, &conversation, user_message_id, bea, ada);

    let (ada_keys, _, _) = preview_memory_values(&backend, conversation.id, ada).await;
    assert!(ada_keys.contains("The group met at the harbor."));
    let (bea_keys, _, _) = preview_memory_values(&backend, conversation.id, bea).await;
    assert!(!bea_keys.contains("The group met at the harbor."));
}

#[tokio::test]
async fn group_companion_turn_start_skips_away_members_until_they_speak() {
    let (backend, conversation, ada, bea) =
        companion_prompt_group("group-companion-turn-audience", true);
    let database = backend.database();
    let engine = group_companion_engine();
    let coordinator = CompanionTurnCoordinator::new(database, Some(&engine));
    let sent = coordinator
        .begin_send(
            &direct_send_command(
                &conversation,
                "group-companion-turn-audience-send",
                "Hello Ada and Bea.",
            ),
            TimestampMillis::new(NOW.get() + 1),
            &CancellationToken::new(),
        )
        .expect("send");
    let user_message_id = group_companion_user_message_id(&sent.value);
    cancel_started_group_turn(
        database,
        &sent.value,
        "group-companion-turn-audience",
        NOW.get() + 2,
    );
    mark_member_left_audience(database, &conversation, user_message_id, ada, bea);
    let current = ConversationReader::get(database, conversation.id)
        .expect("conversation after audience change")
        .conversation;
    let second = coordinator
        .begin_send(
            &direct_send_command(
                &current,
                "group-companion-turn-audience-send-two",
                "Only Bea is here.",
            ),
            TimestampMillis::new(NOW.get() + 3),
            &CancellationToken::new(),
        )
        .expect("second send");
    let second_user_message_id = group_companion_user_message_id(&second.value);
    cancel_started_group_turn(
        database,
        &second.value,
        "group-companion-turn-audience-two",
        NOW.get() + 4,
    );
    let checkpoints = database
        .checkpoints_for_message(conversation.id, second_user_message_id)
        .expect("second user message checkpoints");
    assert!(
        !checkpoints
            .iter()
            .any(|checkpoint| checkpoint.character_id == ada)
    );
    assert!(
        checkpoints
            .iter()
            .any(|checkpoint| checkpoint.character_id == bea)
    );

    let current = ConversationReader::get(database, conversation.id)
        .expect("conversation before Ada speaks")
        .conversation;
    let ada_speaker = group_companion_participant(&current, ada);
    let continued = coordinator
        .begin_continue(
            &group_companion_continue_command(
                &current,
                ada_speaker,
                "group-companion-turn-audience-ada-speaks",
            ),
            TimestampMillis::new(NOW.get() + 5),
            &CancellationToken::new(),
        )
        .expect("continue as Ada");
    let model = group_companion_participant_model(&current, ada_speaker);
    let finalized = finalize_group_reply(
        database,
        &continued.value,
        model,
        "I am back.",
        "group-companion-turn-audience-ada-speaks",
        NOW.get() + 6,
    );
    let assistant_message_id = finalized.value.assistant_message.id;
    let current = ConversationReader::get(database, conversation.id)
        .expect("conversation after Ada speaks")
        .conversation;
    let bea_speaker = group_companion_participant(&current, bea);
    let witnessing = coordinator
        .begin_continue(
            &group_companion_continue_command(
                &current,
                bea_speaker,
                "group-companion-turn-audience-witness",
            ),
            TimestampMillis::new(NOW.get() + 7),
            &CancellationToken::new(),
        )
        .expect("witness Ada's reply");
    cancel_started_group_turn(
        database,
        &witnessing.value,
        "group-companion-turn-audience-witness",
        NOW.get() + 8,
    );
    let checkpoints = database
        .checkpoints_for_message(conversation.id, assistant_message_id)
        .expect("assistant message checkpoints");
    assert!(
        checkpoints
            .iter()
            .any(|checkpoint| checkpoint.character_id == ada)
    );
    assert!(
        checkpoints
            .iter()
            .any(|checkpoint| checkpoint.character_id == bea)
    );
}

fn growth_group_fixture(
    operation_key: &str,
    ada_soul: bool,
    ada_share: bool,
    bea_soul: bool,
    bea_share: bool,
) -> (
    Database,
    lettuce_conversations::Conversation,
    CharacterId,
    CharacterId,
    MessageId,
) {
    let database = database_with_builtins();
    let model_id = seed_model(
        &database,
        ProviderProtocol::Ollama,
        &format!("{operation_key}-model"),
    );
    set_application_default_model(&database, model_id);
    let first = seed_named_character_with(&database, "Ada", |defaults| {
        defaults.model_profile_id = Some(model_id);
        defaults.interaction_mode = InteractionMode::Companion;
        if ada_soul {
            defaults.companion_soul = Some(lettuce_companions::CompanionSoulConfig {
                share_soul_growth_across_chats: ada_share,
                ..Default::default()
            });
        }
    });
    let second = seed_named_character_with(&database, "Bea", |defaults| {
        defaults.model_profile_id = Some(model_id);
        defaults.interaction_mode = InteractionMode::Companion;
        if bea_soul {
            defaults.companion_soul = Some(lettuce_companions::CompanionSoulConfig {
                share_soul_growth_across_chats: bea_share,
                ..Default::default()
            });
        }
    });
    let group_id = seed_group(
        &database,
        vec![member(first, 0), member(second, 1)],
        None,
        |group| {
            group.companion_mode = true;
            group.memory_policy = MemoryPolicy::Dynamic;
        },
    );
    let conversation = ConversationLaunchPlanner::new(&database)
        .launch_group(&group_request(group_id, operation_key), NOW)
        .expect("launch group growth")
        .value
        .conversation;
    let engine = group_companion_engine();
    let coordinator = CompanionTurnCoordinator::new(&database, Some(&engine));
    let sent = coordinator
        .begin_send(
            &direct_send_command(
                &conversation,
                &format!("{operation_key}-send"),
                "Hello Ada and Bea.",
            ),
            TimestampMillis::new(NOW.get() + 1),
            &CancellationToken::new(),
        )
        .expect("send group growth message");
    let user_message_id = group_companion_user_message_id(&sent.value);
    cancel_started_group_turn(&database, &sent.value, operation_key, NOW.get() + 2);
    let conversation = ConversationReader::get(&database, conversation.id)
        .expect("conversation after send")
        .conversation;
    (database, conversation, first, second, user_message_id)
}

fn group_memory_result(
    database: &Database,
    conversation: &lettuce_conversations::Conversation,
    source_message_id: MessageId,
    seed: impl FnOnce(&Database),
) -> crate::CompanionMemoryJobRunResult {
    let model_id = GlobalSettingsStore::load(database)
        .expect("settings")
        .default_model_profile_id
        .expect("default model");
    let stored_profile = ModelProfileRepository::get(database, model_id)
        .expect("profile")
        .expect("profile exists");
    let account = ProviderAccountRepository::get(database, stored_profile.provider_account_id)
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
    let memory =
        MemoryRepository::get_for_branch(database, conversation.id, conversation.active_branch_id)
            .expect("group memory")
            .expect("group memory space");
    let source = ConversationReader::timeline_page(
        database,
        conversation.id,
        conversation.active_branch_id,
        &PageRequest {
            cursor: None,
            limit: PageLimit::new(20),
        },
    )
    .expect("group timeline")
    .items
    .into_iter()
    .find(|item| item.message.id == source_message_id)
    .expect("source message");
    let admitted = database
        .admit_dynamic_memory_run_attempt(NewDynamicMemoryRunAttempt {
            run_id: DynamicMemoryRunId::new(),
            attempt_id: DynamicMemoryAttemptId::new(),
            conversation_id: conversation.id,
            branch_id: conversation.active_branch_id,
            space_id: memory.id,
            cycle_start_change: None,
            starting_memory: memory,
            source_messages: vec![DynamicMemorySourceMessage {
                message_id: source_message_id,
                role: MessageRole::User,
                render_source: source.message.active_render_source,
                effective_time: source.message.effective_time,
            }],
            profile,
            time_awareness_enabled: true,
            supersession_enabled: true,
            group_companion_members: Vec::new(),
            structured_fallback_format: lettuce_memory::DynamicMemoryStructuredFallbackFormat::Xml,
            summary_window: lettuce_memory::DynamicMemorySummaryWindow {
                message_interval: 1,
                start: 0,
                end: 1,
            },
            tool_request: lettuce_memory::dynamic_memory_tool_request_for_run(
                lettuce_memory::DynamicMemoryToolOptions {
                    group: true,
                    supersession_enabled: true,
                    require_source_message_id: true,
                    group_companion: None,
                },
                &|key| key.to_owned(),
            ),
            job_id: JobId::new(),
            job_attempt: None,
            now: TimestampMillis::new(NOW.get() + 2),
        })
        .expect("group memory run");
    let processing = database
        .transition_dynamic_memory_attempt(
            admitted.attempt.id,
            admitted.attempt.revision,
            DynamicMemoryAttemptStatus::Processing,
            None,
            TimestampMillis::new(NOW.get() + 2),
        )
        .expect("group memory processing");
    seed(database);
    let attempt = database
        .transition_dynamic_memory_attempt(
            processing.id,
            processing.revision,
            DynamicMemoryAttemptStatus::Succeeded,
            None,
            TimestampMillis::new(NOW.get() + 4),
        )
        .expect("group memory succeeded");
    crate::CompanionMemoryJobRunResult {
        dispatch: crate::CompanionPostTurnMemoryRunDispatch {
            run: admitted.run,
            attempt,
            recovered: false,
        },
        first_round_replayed: false,
        summary_replayed: false,
        loop_result: crate::CompanionMemoryLoopResult {
            summary: None,
            completed_rounds: 0,
            projection_repairs_pending: Vec::new(),
        },
        effects: Vec::new(),
        fresh_memories: Vec::new(),
    }
}

#[test]
fn group_cycle_growth_admits_the_member_with_perspective_memories() {
    for share in [true, false] {
        let key = format!("group-growth-one-{share}");
        let (database, conversation, ada, bea, user_message_id) =
            growth_group_fixture(&key, true, share, true, true);
        let ada_space = database
            .ensure_perspective_space(conversation.id, conversation.active_branch_id, ada)
            .expect("Ada perspective space");
        database
            .ensure_perspective_space(conversation.id, conversation.active_branch_id, bea)
            .expect("Bea perspective space");
        let result = group_memory_result(&database, &conversation, user_message_id, |database| {
            seed_axis_zero_memory(
                database,
                ada_space,
                "Ada noticed the tide turning.",
                Some(user_message_id),
            );
        });
        let admissions = crate::CompanionGrowthJobAdmissionCoordinator::new(&database, &database)
            .admit_group_after_memory(&result)
            .expect("group growth admission");
        assert_eq!(admissions.len(), 1);
        let admission = &admissions[0];
        assert_eq!(admission.run.character_id, ada);
        assert_eq!(admission.job.kind, JobKind::CompanionGrowth);
        assert_eq!(
            admission.run.soul_owner(),
            lettuce_companions::SoulOwner::for_conversation(ada, conversation.id, share)
        );
        assert_eq!(admission.run.conversation_id, conversation.id);
        assert_eq!(admission.run.fresh_memories.len(), 1);
        assert_eq!(
            admission.run.fresh_memories[0].text,
            "Ada noticed the tide turning."
        );
    }
}

#[test]
fn group_cycle_growth_skips_a_member_without_a_companion_soul() {
    let (database, conversation, ada, bea, user_message_id) =
        growth_group_fixture("group-growth-no-soul", false, true, true, true);
    let ada_space = database
        .ensure_perspective_space(conversation.id, conversation.active_branch_id, ada)
        .expect("Ada perspective space");
    database
        .ensure_perspective_space(conversation.id, conversation.active_branch_id, bea)
        .expect("Bea perspective space");
    let result = group_memory_result(&database, &conversation, user_message_id, |database| {
        seed_axis_zero_memory(
            database,
            ada_space,
            "Ada noticed the tide turning.",
            Some(user_message_id),
        );
    });
    assert!(
        crate::CompanionGrowthJobAdmissionCoordinator::new(&database, &database)
            .admit_group_after_memory(&result)
            .expect("group growth admission")
            .is_empty()
    );
}

#[test]
fn group_cycle_growth_admits_each_member_once_for_the_same_memory_run() {
    let (database, conversation, ada, bea, user_message_id) =
        growth_group_fixture("group-growth-two", true, true, true, false);
    let ada_space = database
        .ensure_perspective_space(conversation.id, conversation.active_branch_id, ada)
        .expect("Ada perspective space");
    let bea_space = database
        .ensure_perspective_space(conversation.id, conversation.active_branch_id, bea)
        .expect("Bea perspective space");
    let result = group_memory_result(&database, &conversation, user_message_id, |database| {
        seed_axis_zero_memory(
            database,
            ada_space,
            "Ada noticed the tide turning.",
            Some(user_message_id),
        );
        seed_axis_zero_memory(
            database,
            bea_space,
            "Bea heard the gulls.",
            Some(user_message_id),
        );
    });
    let coordinator = crate::CompanionGrowthJobAdmissionCoordinator::new(&database, &database);
    let first = coordinator
        .admit_group_after_memory(&result)
        .expect("first group growth admission");
    assert_eq!(
        first
            .iter()
            .map(|admission| admission.run.character_id)
            .collect::<Vec<_>>(),
        [ada, bea]
    );
    assert!(first.iter().all(|admission| admission.created));
    assert!(
        first
            .iter()
            .all(|admission| admission.run.memory_run_id == result.dispatch.run.id)
    );
    let second = coordinator
        .admit_group_after_memory(&result)
        .expect("replayed group growth admission");
    assert_eq!(second.len(), 2);
    assert!(second.iter().all(|admission| !admission.created));
    assert_eq!(
        second
            .iter()
            .map(|admission| admission.job.id)
            .collect::<Vec<_>>(),
        first
            .iter()
            .map(|admission| admission.job.id)
            .collect::<Vec<_>>()
    );
}
