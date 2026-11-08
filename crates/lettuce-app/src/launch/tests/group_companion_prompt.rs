use super::*;

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
    assert!(text.contains("has no particular history with anyone else in the room yet"));
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
