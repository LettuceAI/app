use std::sync::Arc;

use async_trait::async_trait;
use lettuce_characters::{CharacterDefaults, CharacterRepository};
use lettuce_companions::{
    CompanionEffectSourceWindow, CompanionMemoryChanges, CompanionTurnEffectOutcome,
    CompanionTurnEffectRepository, CompanionTurnEffectStatus, EmotionClassification,
};
use lettuce_contracts::{self as dto, ApiErrorCode, ApiEvent, RequiredModel};
use lettuce_conversations::{
    ConversationReader, ConversationRepository, GenerationCheckpointEnvelope,
    GenerationCheckpointEvent, GenerationTurnStatus, InitialInferenceRepository,
};
use lettuce_embeddings::{EmbeddingDimensions, EmbeddingRequest, EmbeddingVector};
use lettuce_jobs::{SystemClock, handle::CancellationToken};
use lettuce_settings::GlobalSettingsStore;
use lettuce_types::{MessageId, TimestampMillis, UsageEventId};

use super::tests::{
    RecordingStream, Reply, api_events, create_character, harness, harness_in, send,
};
use super::turns_tests::{
    author_of, continue_request, conversation, group_cast, open, regenerate_request, replied_chat,
    run_generation,
};
use super::*;
use crate::{
    CompanionEmotionEngine, CompanionEmotionGenerationError, EmbeddingGenerationError,
    MemoryEmbeddingEngine,
};

fn stream() -> Arc<RecordingStream> {
    Arc::new(RecordingStream::default())
}

struct FixedEmbedding;

impl MemoryEmbeddingEngine for FixedEmbedding {
    fn source_revision(&self) -> &str {
        "fixed"
    }

    fn dimensions(&self) -> EmbeddingDimensions {
        EmbeddingDimensions::from_preference(None)
    }

    fn count_tokens(&self, _text: &str) -> Result<u32, EmbeddingGenerationError> {
        Ok(1)
    }

    fn embed_memory(
        &self,
        _request: &EmbeddingRequest,
        _cancellation: &CancellationToken,
    ) -> Result<EmbeddingVector, EmbeddingGenerationError> {
        Err(EmbeddingGenerationError::Unavailable)
    }
}

struct NeutralEmotion;

impl CompanionEmotionEngine for NeutralEmotion {
    fn classify_emotion(
        &self,
        _text: &str,
        _cancellation: &CancellationToken,
    ) -> Result<Option<EmotionClassification>, CompanionEmotionGenerationError> {
        Ok(None)
    }
}

struct AllModels;

#[async_trait]
impl ModelLoader for AllModels {
    fn installed(&self, _context: &ApiContext, _model: RequiredModel) -> bool {
        true
    }

    async fn prepare(&self, _context: &ApiContext) -> bool {
        true
    }

    fn embedding(&self, _context: &ApiContext) -> ModelLoad<Arc<dyn MemoryEmbeddingEngine>> {
        ModelLoad::Loaded(Arc::new(FixedEmbedding))
    }

    fn emotion(&self, _context: &ApiContext) -> ModelLoad<Arc<dyn CompanionEmotionEngine>> {
        ModelLoad::Loaded(Arc::new(NeutralEmotion))
    }
}

async fn stats(harness: &super::tests::Harness, chat: &str) -> dto::ParticipationStats {
    conversation_participation_stats(
        &harness.context,
        dto::ConversationRequest {
            conversation_id: chat.into(),
        },
    )
    .await
    .expect("participation stats")
}

fn stat_of<'a>(
    stats: &'a dto::ParticipationStats,
    participant: &lettuce_types::ConversationParticipantId,
) -> &'a dto::ParticipationStat {
    stats
        .items
        .iter()
        .find(|stat| stat.participant_id == participant.to_string())
        .expect("participant")
}

#[tokio::test(flavor = "multi_thread")]
async fn participation_is_counted_from_the_visible_replies_of_the_selected_branch() {
    let harness = harness(Reply::Text("Hello."));
    let cast = group_cast(&harness, "stats").await;
    send(&harness, &cast.chat, "stats-send", "Hi all", stream())
        .await
        .expect("send");
    run_generation(&harness).await;
    for index in 0..3 {
        conversation_continue(
            &harness.context,
            continue_request(&harness, &cast.chat, &format!("stats-continue-{index}")),
            stream(),
        )
        .await
        .expect("continue");
        run_generation(&harness).await;
    }
    let view = open(&harness, &cast.chat).await;
    let replies = view
        .messages
        .items
        .iter()
        .filter(|message| message.role == dto::MessageRole::Assistant)
        .map(|message| message.id.clone())
        .collect::<Vec<_>>();
    assert_eq!(replies.len(), 4);
    let counted = stats(&harness, &cast.chat).await;
    assert_eq!(counted.total_messages, 4);
    let (ada, bea, cleo) = (
        stat_of(&counted, &cast.ada),
        stat_of(&counted, &cast.bea),
        stat_of(&counted, &cast.cleo),
    );
    assert_eq!(
        (ada.message_count, bea.message_count, cleo.message_count),
        (2, 1, 1)
    );
    assert_eq!((ada.percent, bea.percent, cleo.percent), (50, 25, 25));
    assert_eq!(ada.last_spoke_message_id.as_deref(), Some(&*replies[3]));
    assert_eq!(bea.last_spoke_message_id.as_deref(), Some(&*replies[1]));
    assert!(ada.last_spoke_at.is_some());

    let stored = conversation(&harness, &cast.chat);
    let deleted = ConversationRepository::delete_message(
        harness.context.backend().database(),
        &lettuce_conversations::TombstoneMessage {
            conversation_id: stored.id,
            message_id: replies[3].parse().expect("message id"),
            expected_revision: stored.revision,
            operation: crate::conversation::edit_operation("stats-delete".into(), &[b"delete"])
                .expect("token"),
            descendants: lettuce_conversations::DescendantPolicy::Preserve,
        },
        harness.context.now(),
    );
    deleted.expect("delete the newest reply");
    let counted = stats(&harness, &cast.chat).await;
    assert_eq!(counted.total_messages, 3);
    assert_eq!(stat_of(&counted, &cast.ada).message_count, 1);
    assert_eq!(
        stat_of(&counted, &cast.ada)
            .last_spoke_message_id
            .as_deref(),
        Some(&*replies[0])
    );

    let mut forced = regenerate_request(&harness, &cast.chat, &replies[1], "stats-regen");
    forced.forced_speaker_participant_id = Some(cast.cleo.to_string());
    conversation_regenerate(&harness.context, forced, stream())
        .await
        .expect("regenerate with another speaker");
    run_generation(&harness).await;
    let counted = stats(&harness, &cast.chat).await;
    assert_eq!(stat_of(&counted, &cast.bea).message_count, 0);
    assert_eq!(stat_of(&counted, &cast.bea).percent, 0);
    assert_eq!(stat_of(&counted, &cast.cleo).message_count, 2);
    assert_eq!(
        author_of(&open(&harness, &cast.chat).await, &replies[1]).as_deref(),
        Some(&*cast.cleo.to_string())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_speaker_selection_preview_renders_the_prompt_without_a_provider_call() {
    let harness = harness(Reply::Text("Hello."));
    let cast = group_cast(&harness, "preview").await;
    send(&harness, &cast.chat, "preview-send", "Hi all", stream())
        .await
        .expect("send");
    run_generation(&harness).await;
    let requests = harness.provider.requests.lock().expect("requests").len();
    let preview = |user_message: Option<&str>| {
        conversation_speaker_selection_preview(
            &harness.context,
            dto::SpeakerSelectionPreviewRequest {
                conversation_id: cast.chat.clone(),
                user_message: user_message.map(str::to_owned),
            },
        )
    };
    let plain = preview(None).await.expect("preview");
    for name in ["Ada", "Bea", "Cleo"] {
        assert!(plain.prompt.contains(name), "{}", plain.prompt);
    }
    let with_message = preview(Some("  Bea, are you there?  "))
        .await
        .expect("preview with a user message");
    assert!(with_message.prompt.contains("Bea, are you there?"));
    assert!(!plain.prompt.contains("Bea, are you there?"));
    assert_eq!(
        harness.provider.requests.lock().expect("requests").len(),
        requests
    );

    let (direct, _) = replied_chat(&harness, "preview-direct").await;
    let error = conversation_speaker_selection_preview(
        &harness.context,
        dto::SpeakerSelectionPreviewRequest {
            conversation_id: direct,
            user_message: None,
        },
    )
    .await
    .expect_err("a one-to-one chat has no speaker selection");
    assert_eq!(error.code, ApiErrorCode::Unsupported);
}

async fn snapshot(
    harness: &super::tests::Harness,
    message: &str,
) -> Result<dto::PromptSnapshot, dto::ApiError> {
    message_prompt_snapshot(
        &harness.context,
        dto::MessagePromptSnapshotRequest {
            message_id: message.into(),
        },
    )
    .await
}

fn text_of(message: &dto::PromptMessage) -> String {
    message
        .parts
        .iter()
        .filter_map(|part| match part {
            dto::PromptPart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_snapshot_shows_what_was_assembled_even_after_the_character_changes() {
    let harness = harness(Reply::Text("Hello."));
    let (chat, reply) = replied_chat(&harness, "snapshot").await;
    let before = snapshot(&harness, &reply).await.expect("snapshot");
    assert_eq!(before.operation, dto::PromptOperation::Send);
    assert!(
        before
            .messages
            .iter()
            .any(|message| text_of(message).contains("Hi there")),
        "{:?}",
        before.messages
    );
    assert!(
        before
            .messages
            .iter()
            .any(|message| text_of(message).contains("A meticulous engineer")),
        "the character's description was sent"
    );
    let sections = before.sections.clone().expect("recorded sections");
    let character = sections
        .iter()
        .find(|section| section.kind == dto::PromptSectionKind::Character)
        .expect("character section");
    assert_eq!(character.label.as_deref(), Some("Ada"));
    assert!(character.estimated_tokens > 0);
    assert!(sections.iter().any(
        |section| section.kind == dto::PromptSectionKind::PromptEntry && section.label.is_some()
    ));
    assert!(before.sections_unavailable.is_none());
    assert_eq!(
        sections
            .iter()
            .map(|section| section.estimated_tokens)
            .sum::<u32>(),
        before.budget.estimated_input_tokens,
        "only actually sent bytes count once"
    );
    assert!(before.budget.estimated_input_tokens > 0);
    assert!(before.budget.selected_messages > 0);
    assert!(!before.model.external_model_id.is_empty());

    let database = harness.context.backend().database();
    let character_id = harness.character_id;
    let details = CharacterRepository::get(database, character_id)
        .expect("character")
        .expect("exists");
    let mut profile = details.character.profile.clone();
    profile.description = Some("Now a retired sailor".into());
    CharacterRepository::revise_profile(
        database,
        character_id,
        details.character.revision,
        profile,
        harness.context.now(),
    )
    .expect("edit the character");
    let after = snapshot(&harness, &reply).await.expect("snapshot");
    assert_eq!(after, before);
    assert!(
        !after
            .messages
            .iter()
            .any(|message| text_of(message).contains("retired sailor"))
    );

    conversation_regenerate(
        &harness.context,
        regenerate_request(&harness, &chat, &reply, "snapshot-regen"),
        stream(),
    )
    .await
    .expect("regenerate");
    run_generation(&harness).await;
    let regenerated = snapshot(&harness, &reply).await.expect("snapshot");
    assert_eq!(regenerated.operation, dto::PromptOperation::Regenerate);
    assert_ne!(regenerated.turn_id, before.turn_id);
    assert!(
        regenerated
            .messages
            .iter()
            .any(|message| text_of(message).contains("retired sailor"))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_record_from_before_the_breakdown_has_no_sections_and_says_why() {
    let harness = harness(Reply::Text("Hello."));
    let (chat, reply) = replied_chat(&harness, "old-record").await;
    let database = harness.context.backend().database();
    let message: MessageId = reply.parse().expect("message id");
    let candidate = ConversationReader::page_candidates(
        database,
        message,
        &lettuce_types::PageRequest {
            cursor: None,
            limit: lettuce_types::PageLimit::default(),
        },
    )
    .expect("candidates")
    .items
    .remove(0);
    let turn = ConversationReader::get_turn(database, candidate.turn_id).expect("turn");
    let attempt = &turn.attempts[0];
    let mut record = InitialInferenceRepository::initial_inference_for_attempt(
        database,
        conversation(&harness, &chat).id,
        turn.id,
        attempt.id,
        attempt.job_id.expect("job"),
    )
    .expect("record")
    .expect("recorded request");
    record.request.context.attributions.sections = None;
    let view = super::inspect::snapshot_view(&harness.context, turn.id, &candidate, &record);
    assert_eq!(view.sections, None);
    assert_eq!(
        view.sections_unavailable,
        Some(dto::PromptSectionsUnavailable::PredatesBreakdown)
    );
    assert!(!view.messages.is_empty());
    let stored = serde_json::to_string(&record.request.context.attributions).expect("attributions");
    assert!(!stored.contains("sections"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_user_message_has_no_snapshot() {
    let harness = harness(Reply::Text("Hello."));
    let (chat, _reply) = replied_chat(&harness, "user-snapshot").await;
    let view = open(&harness, &chat).await;
    let user = view
        .messages
        .items
        .iter()
        .find(|message| message.role == dto::MessageRole::User)
        .expect("user message")
        .id
        .clone();
    let error = snapshot(&harness, &user).await.expect_err("not a reply");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
    let unknown = snapshot(&harness, &uuid::Uuid::new_v4().to_string())
        .await
        .expect_err("unknown message");
    assert_eq!(unknown.code, ApiErrorCode::NotFound);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_settled_companion_effect_is_published_once_and_readable() {
    settled_companion_effect(false).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_retried_companion_effect_is_applied_and_published_once() {
    settled_companion_effect(true).await;
}

async fn settled_companion_effect(retry: bool) {
    let harness = harness_in(
        Reply::Text("Hello."),
        Arc::new(SystemClock),
        None,
        None,
        Arc::new(AllModels),
    );
    let database = harness.context.backend().database();
    let stored = database.load().expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.enabled = true;
    database
        .save(settings, stored.default_model_profile_id, stored.revision)
        .expect("enable dynamic memory");
    let companion = create_character(
        database,
        "Mira",
        CharacterDefaults {
            interaction_mode: lettuce_characters::InteractionMode::Companion,
            companion_soul: Some(lettuce_companions::CompanionSoulConfig::default()),
            memory_policy: lettuce_characters::MemoryPolicy::Dynamic,
            ..CharacterDefaults::default()
        },
    );
    let chat = conversation_launch_direct(
        &harness.context,
        dto::LaunchDirectRequest {
            character_id: companion.to_string(),
            title: None,
            scene_id: None,
            starter_id: None,
            client_operation_id: "effect-launch".into(),
        },
    )
    .await
    .expect("launch")
    .conversation_id;
    let conversation_before = conversation(&harness, &chat);
    let user = conversation_before
        .participants
        .iter()
        .find(|participant| participant.role == lettuce_conversations::ParticipantRole::User)
        .expect("user")
        .id;
    let sent = crate::CompanionTurnCoordinator::new(database, Some(&NeutralEmotion))
        .begin_send(
            &lettuce_conversations::SendConversation {
                conversation_id: conversation_before.id,
                branch_id: conversation_before.active_branch_id,
                expected_revision: conversation_before.revision,
                operation: crate::conversation::edit_operation("effect-send".into(), &[b"send"])
                    .expect("token"),
                message: lettuce_conversations::MessageDraft {
                    role: lettuce_conversations::MessageRole::User,
                    author_participant_id: Some(user),
                    parts: vec![lettuce_conversations::MessagePart::Text {
                        text: "I missed you.".into(),
                    }],
                    visibility: lettuce_conversations::MessageVisibility::Visible,
                    pinned: false,
                    scene_edited: false,
                },
                swap_roles: false,
            },
            harness.context.now(),
            &CancellationToken::new(),
        )
        .expect("companion send");
    let mut turn_id = sent.value.turn.id;
    let mut turn = sent.value.turn;
    let mut attempt_id = sent.value.attempt.id;
    let user_message_id = match &turn.input {
        lettuce_conversations::GenerationInput::UserMessage { message_id } => *message_id,
        _ => panic!("a send starts from a user message"),
    };
    let operation = |name: &str| {
        crate::conversation::edit_operation(name.into(), &[name.as_bytes()]).expect("token")
    };
    if retry {
        turn = database
            .append_event(
                turn_id,
                turn.revision,
                &operation("effect-failure-preparing"),
                GenerationCheckpointEnvelope {
                    turn_id,
                    attempt_id,
                    job_id: None,
                    correlation_id: None,
                    sequence: 1,
                    event: GenerationCheckpointEvent::Stage {
                        status: GenerationTurnStatus::Preparing,
                    },
                },
                harness.context.now(),
            )
            .expect("prepare before failure")
            .value;
        database
            .fail_generation(
                turn_id,
                attempt_id,
                conversation(&harness, &chat).revision,
                turn.revision,
                &operation("effect-fail"),
                lettuce_conversations::GenerationFailureCode::Internal,
                UsageEventId::new(),
                harness.context.now(),
            )
            .expect("fail before retry");
        let source = ConversationReader::get_turn(database, turn_id).expect("failed source");
        let command = lettuce_conversations::RetryGeneration {
            conversation_id: conversation_before.id,
            branch_id: conversation_before.active_branch_id,
            turn_id,
            expected_revision: conversation(&harness, &chat).revision,
            expected_turn_revision: source.revision,
            operation: operation("effect-retry"),
        };
        let retried = database
            .begin_retry(&command, harness.context.now())
            .expect("retry");
        let replayed = database
            .begin_retry(&command, harness.context.now())
            .expect("retry replay");
        assert_eq!(replayed.value.turn.id, retried.value.turn.id);
        turn_id = retried.value.turn.id;
        turn = retried.value.turn;
        attempt_id = retried.value.attempt.id;
    }
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
                &operation(&format!("effect-stage-{sequence}")),
                GenerationCheckpointEnvelope {
                    turn_id,
                    attempt_id,
                    job_id: None,
                    correlation_id: None,
                    sequence: u64::try_from(sequence + 1).expect("sequence"),
                    event: GenerationCheckpointEvent::Stage { status },
                },
                TimestampMillis::new(harness.context.now().get() + 1),
            )
            .expect("advance the turn")
            .value;
    }
    let stored = conversation(&harness, &chat);
    let model = turn
        .resolved_model
        .clone()
        .or_else(|| match &stored.kind {
            lettuce_conversations::ConversationKind::Direct(details) => match &details.model {
                lettuce_conversations::SnapshotSelection::Inherited(model)
                | lettuce_conversations::SnapshotSelection::Explicit(model) => Some(model.clone()),
                lettuce_conversations::SnapshotSelection::Disabled => None,
            },
            lettuce_conversations::ConversationKind::Group(_) => None,
        })
        .expect("a model snapshot");
    let finalized = database
        .finalize_generation(
            turn_id,
            attempt_id,
            stored.revision,
            turn.revision,
            &operation("effect-finalize"),
            lettuce_conversations::FinalizationDraft {
                parts: vec![lettuce_conversations::MessagePart::Text {
                    text: "I missed you too.".into(),
                }],
                ordinal: 0,
                model,
                replay: None,
                outcome: GenerationCheckpointEvent::Completed,
                scene_follow_up: None,
            },
            UsageEventId::new(),
            TimestampMillis::new(harness.context.now().get() + 2),
        )
        .expect("finalize the reply");
    let reply = finalized.value.assistant_message.id;
    let processing = message_companion_effect(
        &harness.context,
        dto::MessageCompanionEffectRequest {
            message_id: reply.to_string(),
        },
    )
    .await
    .expect("effect")
    .expect("a processing effect");
    assert_eq!(processing.status, dto::CompanionEffectStatus::Processing);

    let mut feed = super::conversation_feed::ConversationFeed::start(&harness.context)
        .await
        .expect("feed");
    let effect = CompanionTurnEffectRepository::get_for_message(database, stored.id, reply)
        .expect("effect")
        .expect("processing effect");
    assert_eq!(effect.status, CompanionTurnEffectStatus::Processing);
    let enqueued_at = harness.context.now();
    CompanionTurnEffectRepository::settle(
        database,
        effect.id,
        CompanionTurnEffectOutcome::Ready {
            summary: Some("Remembered the reunion.".into()),
            memory_changes: CompanionMemoryChanges::default(),
            source_window: CompanionEffectSourceWindow {
                message_ids: vec![user_message_id, reply],
                enqueued_at,
            },
        },
        TimestampMillis::new(enqueued_at.get() + 1),
    )
    .expect("settle the effect");
    feed.publish(&harness.context).await.expect("publish");
    feed.publish(&harness.context).await.expect("publish again");
    let settled = api_events(&harness)
        .into_iter()
        .filter(|event| matches!(event, ApiEvent::MessageEffectSettled { .. }))
        .collect::<Vec<_>>();
    assert_eq!(
        settled,
        vec![ApiEvent::MessageEffectSettled {
            conversation_id: chat.clone(),
            message_id: reply.to_string(),
        }]
    );
    let ready = message_companion_effect(
        &harness.context,
        dto::MessageCompanionEffectRequest {
            message_id: reply.to_string(),
        },
    )
    .await
    .expect("effect")
    .expect("a settled effect");
    assert_eq!(ready.status, dto::CompanionEffectStatus::Ready);
    assert_eq!(ready.summary.as_deref(), Some("Remembered the reunion."));
    let none = message_companion_effect(
        &harness.context,
        dto::MessageCompanionEffectRequest {
            message_id: user_message_id.to_string(),
        },
    )
    .await
    .expect("effect of a user message");
    assert_eq!(none, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn regeneration_guidance_breakdown_matches_dispatched_direct_and_group_content() {
    for group in [true, false] {
        let harness = harness(Reply::Text("Hello."));
        let (chat, reply, character, speaker) = if group {
            let cast = group_cast(&harness, "guidance-accounting").await;
            send(&harness, &cast.chat, "guidance-send", "Hi", stream())
                .await
                .expect("send");
            run_generation(&harness).await;
            let view = open(&harness, &cast.chat).await;
            let reply = view
                .messages
                .items
                .iter()
                .find(|item| item.role == dto::MessageRole::Assistant)
                .expect("reply")
                .id
                .clone();
            (
                cast.chat,
                reply,
                cast.ada_character,
                Some(cast.ada.to_string()),
            )
        } else {
            let (chat, reply) = replied_chat(&harness, "direct-guidance-accounting").await;
            (chat, reply, harness.character_id, None)
        };
        let database = harness.context.backend().database();
        let details = CharacterRepository::get(database, character)
            .expect("character")
            .expect("exists");
        let mut profile = details.character.profile;
        let name = "A".repeat(33);
        profile.name = name.clone();
        CharacterRepository::revise_profile(
            database,
            character,
            details.character.revision,
            profile,
            harness.context.now(),
        )
        .expect("rename");
        let mut request = regenerate_request(&harness, &chat, &reply, "guidance-regenerate");
        request.guidance = Some("{{char}}".into());
        request.forced_speaker_participant_id = speaker;
        conversation_regenerate(&harness.context, request, stream())
            .await
            .expect("regenerate");
        run_generation(&harness).await;
        let stored = snapshot(&harness, &reply)
            .await
            .expect("new dispatch snapshot");
        assert_eq!(stored.operation, dto::PromptOperation::Regenerate);
        let instruction = stored
            .messages
            .iter()
            .map(text_of)
            .find(|text| text.contains("[REGENERATE INSTRUCTION]"))
            .expect("guidance instruction");
        assert!(instruction.ends_with(if group { "{{char}}" } else { &name }));
        assert_eq!(
            stored
                .sections
                .expect("sections")
                .iter()
                .map(|section| section.estimated_tokens)
                .sum::<u32>(),
            stored.budget.estimated_input_tokens
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_snapshot_preserves_distinct_lore_titles_for_identical_placed_contents() {
    use lettuce_context::{CharacterLorebookBindingRepository, LorebookRepository};
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let book = LorebookRepository::create(
        database,
        lettuce_context::LorebookMetadataDraft {
            name: "Shared content".into(),
            detection_policy: lettuce_context::DetectionPolicy::RecentMessageWindow,
            icon_asset_id: None,
            behavior_version: lettuce_context::LorebookBehaviorVersion::LegacyV1,
        },
        [("First", true), ("Second", true), ("Disabled", false)]
            .into_iter()
            .map(|(title, enabled)| lettuce_context::LorebookEntryDraft {
                title: title.into(),
                enabled,
                always_active: true,
                keywords: Vec::new(),
                case_sensitive: false,
                match_mode: lettuce_context::KeywordMatchMode::Literal,
                content: "  same {{char}}  ".into(),
                priority: 0,
            })
            .collect(),
        harness.context.now(),
    )
    .expect("lorebook");
    let revision = CharacterRepository::get(database, harness.character_id)
        .expect("character")
        .expect("exists")
        .character
        .revision;
    CharacterLorebookBindingRepository::bind_character_lorebook(
        database,
        harness.character_id,
        revision,
        lettuce_context::LorebookBindingCreate {
            lorebook_id: book.book.id,
            target: lettuce_context::BindingInsertionTarget::Append,
        },
        harness.context.now(),
    )
    .expect("bind");
    let (_, reply) = replied_chat(&harness, "identical-lore-breakdown").await;
    let stored = snapshot(&harness, &reply).await.expect("snapshot");
    assert!(
        stored
            .messages
            .iter()
            .map(text_of)
            .any(|text| text.contains("same Ada\n\nsame Ada"))
    );
    let sections = stored.sections.expect("sections");
    assert_eq!(
        sections
            .iter()
            .filter(|section| section.kind == dto::PromptSectionKind::Lorebook)
            .map(|section| section.label.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("First"), Some("Second")]
    );
    assert!(
        sections
            .iter()
            .filter(|section| section.kind == dto::PromptSectionKind::PromptEntry)
            .all(|section| section.label.is_some())
    );
    assert_eq!(
        sections
            .iter()
            .map(|section| section.estimated_tokens)
            .sum::<u32>(),
        stored.budget.estimated_input_tokens
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn production_workers_admit_memory_after_a_finished_dynamic_turn() {
    use lettuce_jobs::JobStore;
    let mut harness = harness_in(
        Reply::Text("Hello."),
        Arc::new(SystemClock),
        None,
        None,
        Arc::new(AllModels),
    );
    let database = harness.context.backend().database();
    harness.character_id = create_character(
        database,
        "Dynamic Ada",
        CharacterDefaults {
            memory_policy: lettuce_characters::MemoryPolicy::Dynamic,
            ..CharacterDefaults::default()
        },
    );
    let stored = GlobalSettingsStore::load(database).expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.enabled = true;
    settings.dynamic_memory.summary_message_interval = 2;
    settings.dynamic_memory.run_mode = lettuce_settings::MemoryRunMode::Auto;
    GlobalSettingsStore::save(
        database,
        settings,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("enable automatic memory");
    let workers = startup(&harness.context).await.expect("startup");
    workers.started().await;
    let chat = super::tests::launch(&harness, "production-memory-launch").await;
    let accepted = send(
        &harness,
        &chat,
        "production-memory-send",
        "Hi there",
        stream(),
    )
    .await
    .expect("send");
    tokio::time::timeout(
        std::time::Duration::from_secs(3),
        harness.events.until(|events| {
            events.iter().any(|event| {
                matches!(event, ApiEvent::GenerationSettled { turn_id, .. } if turn_id == &accepted.turn_id)
            })
        }),
    )
    .await
    .expect("generation settled");
    let turn = ConversationReader::get_turn(database, accepted.turn_id.parse().expect("turn"))
        .expect("stored turn");
    assert_eq!(turn.status, GenerationTurnStatus::Succeeded);
    let admitted = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        harness.events.until(|_| {
            JobStore::list(
                database,
                lettuce_jobs::JobQuery {
                    state: None,
                    kind: Some(lettuce_jobs::JobKind::MemoryExtraction),
                    subject: None,
                    page: lettuce_types::PageRequest {
                        cursor: None,
                        limit: lettuce_types::PageLimit::new(1),
                    },
                },
            )
            .expect("memory jobs")
            .items
            .iter()
            .any(|job| job.subject.id.as_str() == chat)
        }),
    )
    .await;
    workers.stop().await;
    assert!(admitted.is_ok(), "the finished turn never admitted memory");
}
