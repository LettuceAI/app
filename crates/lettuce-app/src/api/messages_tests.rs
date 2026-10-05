use std::sync::{Arc, Mutex};

use lettuce_characters::{
    Character, CharacterDefaults, CharacterMedia, CharacterPresentationV1, CharacterProfile,
    CharacterProvenance, CharacterRepository, CreateCharacterPlan, MemoryPolicy,
};
use lettuce_contracts::{self as dto, ApiErrorCode, ApiErrorDetails};
use lettuce_conversations::{
    ConversationChangeFeed, ConversationOverviewReader, ConversationReader, ConversationRepository,
    DescendantPolicy, ForkBranch, InferenceCandidate, InferenceOutcome, InferencePort,
    InferenceRequest, MediaAssetRole, MessageDraft, MessagePart, MessageRole, MessageVisibility,
    OperationKind, ParticipantRole, PortError, RegenerateCandidate, SendConversation,
    TombstoneMessage,
};
use lettuce_jobs::{
    JobKind, JobMutation, JobSpec, JobState, JobStore, JobSubject, OutcomeRef,
    ResourceAvailability, ResourceClass, SubjectKind, WorkerId,
};
use lettuce_media::{
    AssetKind, AssetOrigin, AssetProvenanceV1, BlobState, MediaAsset, MediaAssetRepository,
    MediaBlob, MediaBlobRepository, MediaKind, RetentionClass,
};
use lettuce_memory::{
    DynamicMemoryRunRepository, DynamicMemorySourceMessage, DynamicMemorySuffixRewindRepository,
    MemoryRepository, NewDynamicMemoryRunAttempt, PendingSuffixRewind,
    PendingSuffixRewindRepository,
};
use lettuce_types::{
    AssetId, CharacterId, ContentHash, ConversationId, DynamicMemoryAttemptId, DynamicMemoryRunId,
    JobId, MediaBlobId, MessageId, OperationId, Revision, SceneId, TimestampMillis,
};

use super::conversation_delete_tests::context_over;
use super::tests::{Harness, RecordingStream, Reply, create_character, harness, send};
use super::*;

fn text_of(message: &dto::TimelineMessage) -> String {
    message
        .parts
        .iter()
        .filter_map(|part| match part {
            dto::MessagePartView::Text { text } => Some(text.as_str()),
            dto::MessagePartView::Media { .. } => None,
        })
        .collect()
}

fn media_of(message: &dto::TimelineMessage) -> Vec<String> {
    message
        .parts
        .iter()
        .filter_map(|part| match part {
            dto::MessagePartView::Media { asset, .. } => Some(asset.asset_id.clone()),
            dto::MessagePartView::Text { .. } => None,
        })
        .collect()
}

fn invalid_field(error: &dto::ApiError) -> Option<&str> {
    match &error.details {
        Some(ApiErrorDetails::InvalidField { field })
            if error.code == ApiErrorCode::InvalidInput =>
        {
            Some(field.as_str())
        }
        _ => None,
    }
}

async fn launch_with(harness: &Harness, character_id: CharacterId, key: &str) -> ConversationId {
    conversation_launch_direct(
        &harness.context,
        dto::LaunchDirectRequest {
            character_id: character_id.to_string(),
            title: None,
            scene_id: None,
            starter_id: None,
            client_operation_id: key.into(),
        },
    )
    .await
    .expect("launch")
    .conversation_id
    .parse()
    .expect("conversation id")
}

fn revision(harness: &Harness, conversation_id: ConversationId) -> u64 {
    ConversationReader::get(harness.context.backend().database(), conversation_id)
        .expect("conversation")
        .conversation
        .revision
        .get()
}

/// Commits a user message without starting a reply.
fn append(
    harness: &Harness,
    conversation_id: ConversationId,
    key: &str,
    parts: Vec<MessagePart>,
) -> MessageId {
    let database = harness.context.backend().database();
    let conversation = ConversationReader::get(database, conversation_id)
        .expect("conversation")
        .conversation;
    let user = conversation
        .participants
        .iter()
        .find(|participant| participant.role == ParticipantRole::User)
        .expect("user")
        .id;
    ConversationRepository::append_user_message(
        database,
        &SendConversation {
            conversation_id,
            branch_id: conversation.active_branch_id,
            expected_revision: conversation.revision,
            operation: crate::conversation::edit_operation(key.into(), &[key.as_bytes()])
                .expect("token"),
            message: MessageDraft {
                role: MessageRole::User,
                author_participant_id: Some(user),
                parts,
                visibility: MessageVisibility::Visible,
                pinned: false,
                scene_edited: false,
            },
            swap_roles: false,
        },
        harness.context.now(),
    )
    .expect("append")
    .value
    .id
}

fn text(value: &str) -> Vec<MessagePart> {
    vec![MessagePart::Text { text: value.into() }]
}

fn image_asset(database: &lettuce_database::Database, seed: &str) -> AssetId {
    let blob = MediaBlobRepository::register(
        database,
        MediaBlob {
            id: MediaBlobId::new(),
            content_hash: ContentHash::parse(seed.repeat(32)).expect("hash"),
            kind: MediaKind::Image,
            mime_type: "image/webp".into(),
            byte_size: 4,
            width: Some(2),
            height: Some(2),
            duration_ms: None,
            validation_version: 1,
            state: BlobState::Staged,
            created_at: TimestampMillis::new(3),
            updated_at: TimestampMillis::new(3),
        },
    )
    .expect("blob");
    let blob =
        MediaBlobRepository::finalize_staged_to_ready(database, blob.id, TimestampMillis::new(3))
            .expect("ready blob");
    MediaAssetRepository::create(
        database,
        MediaAsset::new(
            AssetId::new(),
            blob.id,
            AssetKind::MessageImage,
            AssetOrigin::Upload,
            RetentionClass::Library,
            AssetProvenanceV1::default(),
            Revision::INITIAL,
            TimestampMillis::new(4),
            TimestampMillis::new(4),
        )
        .expect("asset"),
    )
    .expect("create asset")
    .id
}

/// A character with text scenes; the first is its default scene.
fn character_with_scenes(
    database: &lettuce_database::Database,
    texts: &[&str],
) -> (CharacterId, Vec<SceneId>) {
    let character_id = CharacterId::new();
    let scenes = texts
        .iter()
        .zip(0..)
        .map(|(text, ordinal)| crate::launch::tests::text_scene(character_id, ordinal, text))
        .collect::<Vec<_>>();
    let scene_ids = scenes.iter().map(|scene| scene.id).collect::<Vec<_>>();
    CharacterRepository::create(
        database,
        CreateCharacterPlan {
            character: Character::new(
                character_id,
                CharacterProfile {
                    name: "Iris".into(),
                    nickname: None,
                    description: Some("A lighthouse keeper".into()),
                    definition: None,
                    design_description: None,
                    scenario: None,
                    rules: Vec::new(),
                },
                CharacterProvenance::default(),
                CharacterDefaults {
                    default_scene_id: scene_ids.first().copied(),
                    ..CharacterDefaults::default()
                },
                CharacterPresentationV1::default(),
                None,
                CharacterMedia::default(),
                TimestampMillis::new(1),
            )
            .expect("character"),
            scenes,
            variants: Vec::new(),
            starters: Vec::new(),
        },
    )
    .expect("create character");
    (character_id, scene_ids)
}

async fn open(harness: &Harness, conversation_id: ConversationId) -> dto::ConversationView {
    conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: conversation_id.to_string(),
        },
    )
    .await
    .expect("open")
}

async fn edit(
    harness: &Harness,
    conversation_id: ConversationId,
    message_id: MessageId,
    key: &str,
    text: &str,
    keep_media: Vec<String>,
) -> Result<dto::MessageChanged, dto::ApiError> {
    message_edit(
        &harness.context,
        dto::MessageEditRequest {
            conversation_id: conversation_id.to_string(),
            message_id: message_id.to_string(),
            expected_revision: revision(harness, conversation_id),
            text: text.into(),
            keep_media,
            client_operation_id: key.into(),
        },
    )
    .await
}

async fn search(
    harness: &Harness,
    conversation_id: ConversationId,
    query: &str,
    cursor: Option<String>,
    limit: Option<u32>,
) -> dto::SearchHitPage {
    conversation_search(
        &harness.context,
        dto::ConversationSearchRequest {
            conversation_id: conversation_id.to_string(),
            query: query.into(),
            cursor,
            limit,
        },
    )
    .await
    .expect("search")
}

fn hit_texts(page: &dto::SearchHitPage) -> Vec<String> {
    page.items.iter().map(|hit| hit.text.clone()).collect()
}

fn delete_after_request(
    conversation_id: ConversationId,
    message_id: MessageId,
    expected_revision: u64,
    key: &str,
) -> dto::MessageDeleteRequest {
    dto::MessageDeleteRequest {
        conversation_id: conversation_id.to_string(),
        message_id: message_id.to_string(),
        expected_revision,
        client_operation_id: key.into(),
    }
}

/// The token `messages_delete_after` derives for `key`.
fn delete_after_token(
    conversation_id: ConversationId,
    anchor: MessageId,
    key: &str,
) -> lettuce_conversations::OperationToken {
    crate::conversation::edit_operation(
        key.into(),
        &[
            b"lettuce-messages-delete-after-v1",
            conversation_id.to_string().as_bytes(),
            anchor.to_string().as_bytes(),
        ],
    )
    .expect("token")
}

fn removed(result: &dto::MessagesDeleteResult) -> Vec<String> {
    match &result.outcome {
        dto::MessagesDeleteOutcome::Tombstoned { removed } => removed.clone(),
        dto::MessagesDeleteOutcome::Branched { .. } => panic!("a branch was forked"),
    }
}

fn rewind_receipt(
    harness: &Harness,
    conversation_id: ConversationId,
    token: &lettuce_conversations::OperationToken,
) -> Option<lettuce_memory::DynamicMemorySuffixRewindReceipt> {
    let database = harness.context.backend().database();
    let record = ConversationReader::operation_record(
        database,
        conversation_id,
        OperationKind::Tombstone,
        token,
    )
    .expect("operation record")?;
    DynamicMemorySuffixRewindRepository::get_dynamic_memory_suffix_rewind(
        database,
        OperationId::from_uuid(record.id.as_uuid()),
    )
    .expect("rewind receipt")
}

/// A dynamic-memory chat with two user messages and a memory run over the
/// second one, whose job is claimed by a worker.
struct MemoryChat {
    conversation_id: ConversationId,
    lead: Option<MessageId>,
    first: MessageId,
    second: MessageId,
    run_id: DynamicMemoryRunId,
    job_id: JobId,
    claim: lettuce_jobs::ClaimRef,
}

async fn memory_chat(harness: &Harness, key: &str) -> MemoryChat {
    memory_chat_with(harness, key, false).await
}

async fn memory_chat_with(harness: &Harness, key: &str, with_lead: bool) -> MemoryChat {
    let database = harness.context.backend().database();
    let character_id = create_character(
        database,
        "Mnemosyne",
        CharacterDefaults {
            memory_policy: MemoryPolicy::Dynamic,
            ..CharacterDefaults::default()
        },
    );
    let conversation_id = launch_with(harness, character_id, &format!("{key}-launch")).await;
    let lead = with_lead.then(|| {
        append(
            harness,
            conversation_id,
            &format!("{key}-lead"),
            text("Hello there."),
        )
    });
    let first = append(
        harness,
        conversation_id,
        &format!("{key}-first"),
        text("I like tea."),
    );
    let second = append(
        harness,
        conversation_id,
        &format!("{key}-second"),
        text("And I keep bees."),
    );
    let memory = MemoryRepository::get_for_conversation(database, conversation_id)
        .expect("memory")
        .expect("dynamic memory space");
    let job = JobStore::create_or_get(
        database,
        JobSpec::new(
            JobKind::MemoryExtraction,
            JobSubject::new(SubjectKind::Conversation, conversation_id.to_string())
                .expect("subject"),
            OutcomeRef::Conversation(conversation_id),
        )
        .with_resources(vec![ResourceClass::Cpu]),
    )
    .expect("memory job")
    .job;
    let claim = JobStore::claim(
        database,
        job.id,
        WorkerId::new(),
        harness.context.now(),
        std::time::Duration::from_secs(600),
        &ResourceAvailability::all(),
    )
    .expect("claim")
    .expect("claimed memory job");
    let second_item = ConversationOverviewReader::timeline_anchor(
        database,
        conversation_id,
        ConversationReader::get(database, conversation_id)
            .expect("conversation")
            .conversation
            .active_branch_id,
        second,
    )
    .expect("second message")
    .item;
    let run_id = DynamicMemoryRunId::new();
    database
        .admit_dynamic_memory_run_attempt(NewDynamicMemoryRunAttempt {
            run_id,
            attempt_id: DynamicMemoryAttemptId::new(),
            conversation_id,
            branch_id: second_item.message.branch_id,
            space_id: memory.id,
            cycle_start_change: None,
            starting_memory: memory,
            source_messages: vec![DynamicMemorySourceMessage {
                message_id: second,
                role: second_item.message.role,
                render_source: second_item.message.active_render_source,
                effective_time: second_item.message.effective_time,
            }],
            profile: crate::companion::companion_memory_run::tests::profile(),
            time_awareness_enabled: false,
            supersession_enabled: false,
            structured_fallback_format: lettuce_memory::DynamicMemoryStructuredFallbackFormat::Xml,
            summary_window: lettuce_memory::DynamicMemorySummaryWindow {
                message_interval: 1,
                start: 1,
                end: 2,
            },
            tool_request: lettuce_memory::dynamic_memory_tool_request_for_run(
                lettuce_memory::DynamicMemoryToolOptions {
                    group: false,
                    supersession_enabled: false,
                    require_source_message_id: false,
                },
                &|key| key.to_owned(),
            ),
            job_id: job.id,
            now: harness.context.now(),
        })
        .expect("memory run");
    MemoryChat {
        conversation_id,
        lead,
        first,
        second,
        run_id,
        job_id: job.id,
        claim: claim.claim,
    }
}

/// A crash between the delete-after tombstone and its rewind: the suffix
/// tombstone and the owed rewind are committed, nothing else ran.
fn tombstone_without_rewind(
    harness: &Harness,
    chat: &MemoryChat,
    key: &str,
) -> PendingSuffixRewind {
    let database = harness.context.backend().database();
    let pending = PendingSuffixRewind {
        after_message_id: chat.first,
        tombstone: TombstoneMessage {
            conversation_id: chat.conversation_id,
            message_id: chat.second,
            expected_revision: Revision::new(revision(harness, chat.conversation_id)),
            operation: delete_after_token(chat.conversation_id, chat.first, key),
            descendants: DescendantPolicy::Tombstone,
        },
        summary_message_interval: 20,
    };
    PendingSuffixRewindRepository::tombstone_suffix(database, &pending, harness.context.now())
        .expect("tombstone before the crash");
    let cancelled = JobStore::append_and_transition(
        database,
        JobMutation::RequestCancellation {
            id: chat.job_id,
            reason: lettuce_jobs::CancellationReason::User,
            at: harness.context.now(),
        },
    )
    .expect("request cancellation");
    JobStore::append_and_transition(
        database,
        JobMutation::RequestCleanup {
            claim: chat.claim.clone(),
            at: cancelled.updated_at,
        },
    )
    .expect("cleanup after cancellation");
    JobStore::append_and_transition(
        database,
        JobMutation::FinishCancellation {
            claim: chat.claim.clone(),
            at: cancelled.updated_at,
        },
    )
    .expect("the cycle ended with the process");
    assert_eq!(
        PendingSuffixRewindRepository::pending_suffix_rewinds(database, Some(chat.conversation_id))
            .expect("pending"),
        vec![pending.clone()]
    );
    assert!(rewind_receipt(harness, chat.conversation_id, &pending.tombstone.operation).is_none());
    pending
}

#[tokio::test(flavor = "multi_thread")]
async fn an_edit_refuses_blank_text() {
    let harness = harness(Reply::Text("Hello."));
    let conversation_id = launch_with(&harness, harness.character_id, "blank-edit").await;
    let message = append(
        &harness,
        conversation_id,
        "blank-edit-message",
        text("Hello"),
    );
    let error = edit(
        &harness,
        conversation_id,
        message,
        "blank-edit-1",
        " \n\t ",
        Vec::new(),
    )
    .await
    .expect_err("a blank edit is refused");
    assert_eq!(invalid_field(&error), Some("text"));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_edit_trims_its_text_replays_its_key_and_conflicts_on_another_request() {
    let harness = harness(Reply::Text("Hello."));
    let conversation_id = launch_with(&harness, harness.character_id, "edit-replay").await;
    let message = append(
        &harness,
        conversation_id,
        "edit-replay-message",
        text("Hello there"),
    );
    let edited = edit(
        &harness,
        conversation_id,
        message,
        "edit-replay-1",
        "  Hi  ",
        Vec::new(),
    )
    .await
    .expect("edit");
    assert_eq!(text_of(&edited.message), "Hi");
    assert_eq!(edited.revision, revision(&harness, conversation_id));
    let replayed = edit(
        &harness,
        conversation_id,
        message,
        "edit-replay-1",
        "Hi",
        Vec::new(),
    )
    .await
    .expect("the same key and request replay");
    assert_eq!(replayed.message, edited.message);
    let history = message_revisions(
        &harness.context,
        dto::MessageHistoryRequest {
            message_id: message.to_string(),
            cursor: None,
        },
    )
    .await
    .expect("revisions");
    assert_eq!(history.items.len(), 2, "the replay wrote no second edit");
    assert_eq!(text_of_parts(&history.items[1].parts), "Hi");
    let conflict = edit(
        &harness,
        conversation_id,
        message,
        "edit-replay-1",
        "Other",
        Vec::new(),
    )
    .await
    .expect_err("another request under the key");
    assert_eq!(conflict.code, ApiErrorCode::Conflict);
    let stale = message_edit(
        &harness.context,
        dto::MessageEditRequest {
            conversation_id: conversation_id.to_string(),
            message_id: message.to_string(),
            expected_revision: edited.revision - 1,
            text: "Late".into(),
            keep_media: Vec::new(),
            client_operation_id: "edit-replay-2".into(),
        },
    )
    .await
    .expect_err("an edit against an old revision");
    assert_eq!(stale.code, ApiErrorCode::Conflict);
}

fn text_of_parts(parts: &[dto::MessagePartView]) -> String {
    parts
        .iter()
        .filter_map(|part| match part {
            dto::MessagePartView::Text { text } => Some(text.as_str()),
            dto::MessagePartView::Media { .. } => None,
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn an_edit_removes_one_of_two_images_and_keeps_the_other() {
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let conversation_id = launch_with(&harness, harness.character_id, "edit-media").await;
    let kept = image_asset(database, "a1");
    let removed = image_asset(database, "b2");
    let message = append(
        &harness,
        conversation_id,
        "edit-media-message",
        vec![
            MessagePart::Text {
                text: "Look".into(),
            },
            MessagePart::MediaAsset {
                asset_id: kept,
                role: MediaAssetRole::Attachment,
            },
            MessagePart::MediaAsset {
                asset_id: removed,
                role: MediaAssetRole::Attachment,
            },
        ],
    );
    let unknown = edit(
        &harness,
        conversation_id,
        message,
        "edit-media-0",
        "Look again",
        vec![AssetId::new().to_string()],
    )
    .await
    .expect_err("an edit cannot add media");
    assert_eq!(invalid_field(&unknown), Some("keep_media"));
    let edited = edit(
        &harness,
        conversation_id,
        message,
        "edit-media-1",
        "Look again",
        vec![kept.to_string()],
    )
    .await
    .expect("edit");
    assert_eq!(text_of(&edited.message), "Look again");
    assert_eq!(media_of(&edited.message), vec![kept.to_string()]);
    let history = message_revisions(
        &harness.context,
        dto::MessageHistoryRequest {
            message_id: message.to_string(),
            cursor: None,
        },
    )
    .await
    .expect("revisions");
    let shows = |parts: &[dto::MessagePartView], asset: AssetId| {
        parts.iter().any(|part| {
            matches!(part, dto::MessagePartView::Media { asset: shown, .. } if shown.asset_id == asset.to_string())
        })
    };
    assert!(
        shows(&history.items[0].parts, removed),
        "history keeps the original"
    );
    assert!(!shows(&history.items[1].parts, removed));
    assert!(shows(&history.items[1].parts, kept));
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_the_scene_message_turns_the_scene_setting_off_in_the_same_commit() {
    let harness = harness(Reply::Text("Hello."));
    let (character_id, scenes) = character_with_scenes(
        harness.context.backend().database(),
        &["The harbor at dawn."],
    );
    let conversation_id = launch_with(&harness, character_id, "delete-scene").await;
    let view = open(&harness, conversation_id).await;
    let scene = view
        .messages
        .items
        .iter()
        .find(|message| message.role == dto::MessageRole::Scene)
        .expect("scene message");
    let before = conversation_settings_get(
        &harness.context,
        dto::ConversationSettingsGetRequest {
            conversation_id: conversation_id.to_string(),
        },
    )
    .await
    .expect("settings");
    assert_eq!(before.scene.id, Some(scenes[0].to_string()));
    let deleted = message_delete(
        &harness.context,
        dto::MessageDeleteRequest {
            conversation_id: conversation_id.to_string(),
            message_id: scene.id.clone(),
            expected_revision: view.revision,
            client_operation_id: "delete-scene-1".into(),
        },
    )
    .await
    .expect("delete");
    assert_eq!(removed(&deleted), vec![scene.id.clone()]);
    assert_eq!(deleted.revision, view.revision + 1, "one commit");
    let after = conversation_settings_get(
        &harness.context,
        dto::ConversationSettingsGetRequest {
            conversation_id: conversation_id.to_string(),
        },
    )
    .await
    .expect("settings");
    assert_eq!(after.scene.id, None);
    assert_eq!(after.scene.source, dto::SettingSource::Conversation);
    assert!(
        open(&harness, conversation_id)
            .await
            .messages
            .items
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_one_message_keeps_the_ones_after_it() {
    let harness = harness(Reply::Text("Hello."));
    let conversation_id = launch_with(&harness, harness.character_id, "delete-one").await;
    let first = append(&harness, conversation_id, "delete-one-1", text("One"));
    let second = append(&harness, conversation_id, "delete-one-2", text("Two"));
    let request = dto::MessageDeleteRequest {
        conversation_id: conversation_id.to_string(),
        message_id: first.to_string(),
        expected_revision: revision(&harness, conversation_id),
        client_operation_id: "delete-one-key".into(),
    };
    let deleted = message_delete(&harness.context, request.clone())
        .await
        .expect("delete");
    assert_eq!(
        message_delete(&harness.context, request)
            .await
            .expect("replay")
            .outcome,
        deleted.outcome
    );
    let shown = open(&harness, conversation_id).await.messages.items;
    assert_eq!(
        shown
            .iter()
            .map(|message| message.id.clone())
            .collect::<Vec<_>>(),
        vec![second.to_string()]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_after_stops_a_running_memory_cycle_then_tombstones_and_rewinds() {
    let harness = harness(Reply::Text("Hello."));
    let chat = memory_chat(&harness, "delete-after-running").await;
    let database = harness.context.backend().database();
    let request = delete_after_request(
        chat.conversation_id,
        chat.first,
        revision(&harness, chat.conversation_id),
        "delete-after-running-1",
    );
    let settle = async {
        let mut changes = harness.context.committed_changes();
        loop {
            let job = JobStore::get(database, chat.job_id)
                .expect("job")
                .expect("job exists");
            if job.state == JobState::CancellationRequested {
                assert!(
                    MemoryRepository::get_for_conversation(database, chat.conversation_id).is_ok(),
                );
                assert_eq!(
                    open(&harness, chat.conversation_id)
                        .await
                        .messages
                        .items
                        .len(),
                    2,
                    "nothing is deleted while the cycle still runs"
                );
                JobStore::append_and_transition(
                    database,
                    JobMutation::RequestCleanup {
                        claim: chat.claim.clone(),
                        at: job.updated_at,
                    },
                )
                .expect("cleanup after cancellation");
                JobStore::append_and_transition(
                    database,
                    JobMutation::FinishCancellation {
                        claim: chat.claim.clone(),
                        at: job.updated_at,
                    },
                )
                .expect("the runner settles the cancelled cycle");
                return;
            }
            changes.changed().await.expect("change signal");
        }
    };
    let (deleted, ()) = tokio::join!(
        messages_delete_after(&harness.context, request.clone()),
        settle
    );
    let deleted = deleted.expect("delete after");
    assert_eq!(removed(&deleted), vec![chat.second.to_string()]);
    let receipt = rewind_receipt(
        &harness,
        chat.conversation_id,
        &delete_after_token(chat.conversation_id, chat.first, "delete-after-running-1"),
    )
    .expect("the rewind committed");
    assert_eq!(receipt.invalid_run_id, Some(chat.run_id));
    assert!(
        PendingSuffixRewindRepository::pending_suffix_rewinds(database, None)
            .expect("pending")
            .is_empty()
    );
    assert_eq!(
        DynamicMemoryRunRepository::list_dynamic_memory_runs(database, chat.conversation_id)
            .expect("runs")
            .len(),
        1
    );
    let replayed = messages_delete_after(&harness.context, request)
        .await
        .expect("replay");
    assert_eq!(replayed.outcome, deleted.outcome);
    let shown = open(&harness, chat.conversation_id).await.messages.items;
    assert_eq!(
        shown
            .iter()
            .map(|message| message.id.clone())
            .collect::<Vec<_>>(),
        vec![chat.first.to_string()]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rewind_a_crash_left_owed_finishes_at_startup() {
    let harness = harness(Reply::Text("Hello."));
    let chat = memory_chat(&harness, "owed-startup").await;
    let pending = tombstone_without_rewind(&harness, &chat, "owed-startup-1");
    super::startup::complete_pending_rewinds(&harness.context)
        .await
        .expect("startup rewind");
    let receipt = rewind_receipt(&harness, chat.conversation_id, &pending.tombstone.operation)
        .expect("startup finished the rewind");
    assert_eq!(receipt.invalid_run_id, Some(chat.run_id));
    assert!(
        PendingSuffixRewindRepository::pending_suffix_rewinds(
            harness.context.backend().database(),
            None
        )
        .expect("pending")
        .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rewind_a_crash_left_owed_finishes_before_the_next_memory_cycle() {
    let harness = harness(Reply::Text("Hello."));
    let chat = memory_chat(&harness, "owed-admission").await;
    let pending = tombstone_without_rewind(&harness, &chat, "owed-admission-1");
    let embedding = harness.context.embedding();
    harness
        .context
        .backend()
        .companion_memory_host(embedding.as_ref(), harness.context.inference())
        .after_turn(
            chat.conversation_id,
            lettuce_conversations::GenerationOperation::Send,
            WorkerId::new(),
            harness.context.now(),
            std::time::Duration::from_secs(60),
            &ResourceAvailability::all(),
        )
        .expect("memory admission");
    let receipt = rewind_receipt(&harness, chat.conversation_id, &pending.tombstone.operation)
        .expect("the admission finished the rewind first");
    assert_eq!(receipt.invalid_run_id, Some(chat.run_id));
    assert!(
        PendingSuffixRewindRepository::pending_suffix_rewinds(
            harness.context.backend().database(),
            Some(chat.conversation_id)
        )
        .expect("pending")
        .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn pinning_sets_the_state_and_setting_it_again_keeps_it() {
    let harness = harness(Reply::Text("Hello."));
    let conversation_id = launch_with(&harness, harness.character_id, "pin").await;
    let first = append(&harness, conversation_id, "pin-1", text("Remember this"));
    append(&harness, conversation_id, "pin-2", text("Not this"));
    let pin = |key: &str, pinned: bool| dto::MessagePinRequest {
        conversation_id: conversation_id.to_string(),
        message_id: first.to_string(),
        expected_revision: revision(&harness, conversation_id),
        pinned,
        client_operation_id: key.into(),
    };
    assert!(
        message_pin(&harness.context, pin("pin-a", true))
            .await
            .expect("pin")
            .message
            .pinned
    );
    assert!(
        message_pin(&harness.context, pin("pin-b", true))
            .await
            .expect("pin again")
            .message
            .pinned
    );
    let pinned = conversation_pinned_messages(
        &harness.context,
        dto::ConversationPinnedMessagesRequest {
            conversation_id: conversation_id.to_string(),
            cursor: None,
            limit: None,
        },
    )
    .await
    .expect("pinned");
    assert_eq!(
        pinned
            .items
            .iter()
            .map(|message| message.id.clone())
            .collect::<Vec<_>>(),
        vec![first.to_string()]
    );
    assert!(
        !message_pin(&harness.context, pin("pin-c", false))
            .await
            .expect("unpin")
            .message
            .pinned
    );
    let replay_conflict = message_pin(&harness.context, pin("pin-a", false))
        .await
        .expect_err("the key named another request");
    assert_eq!(replay_conflict.code, ApiErrorCode::Conflict);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_scene_swipe_rerenders_the_scene_message_and_selects_the_scene() {
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let (character_id, scenes) =
        character_with_scenes(database, &["The harbor at dawn.", "A lantern-lit market."]);
    let (_, foreign) = character_with_scenes(database, &["Somewhere else."]);
    let conversation_id = launch_with(&harness, character_id, "scene-swipe").await;
    let view = open(&harness, conversation_id).await;
    let scene = view
        .messages
        .items
        .iter()
        .find(|message| message.role == dto::MessageRole::Scene)
        .expect("scene message")
        .clone();
    assert_eq!(text_of(&scene), "The harbor at dawn.");
    let scene_id: MessageId = scene.id.parse().expect("id");
    edit(
        &harness,
        conversation_id,
        scene_id,
        "scene-swipe-edit",
        "My own harbor.",
        Vec::new(),
    )
    .await
    .expect("edit the scene");
    let scene_edited = |harness: &Harness| {
        let database = harness.context.backend().database();
        ConversationOverviewReader::timeline_anchor(
            database,
            conversation_id,
            ConversationReader::get(database, conversation_id)
                .expect("conversation")
                .conversation
                .active_branch_id,
            scene_id,
        )
        .expect("scene")
        .item
        .message
        .scene_edited
    };
    assert!(scene_edited(&harness));
    let select = |key: &str, scene: SceneId, message: &str| dto::MessageSceneSelectRequest {
        conversation_id: conversation_id.to_string(),
        message_id: message.into(),
        expected_revision: revision(&harness, conversation_id),
        scene_id: scene.to_string(),
        client_operation_id: key.into(),
    };
    let wrong_scene = message_scene_select(
        &harness.context,
        select("scene-swipe-x", foreign[0], &scene.id),
    )
    .await
    .expect_err("another character's scene");
    assert_eq!(invalid_field(&wrong_scene), Some("scene_id"));
    let user = append(&harness, conversation_id, "scene-swipe-user", text("Hi"));
    let wrong_message = message_scene_select(
        &harness.context,
        select("scene-swipe-y", scenes[1], &user.to_string()),
    )
    .await
    .expect_err("not the scene message");
    assert_eq!(invalid_field(&wrong_message), Some("message_id"));
    let before = revision(&harness, conversation_id);
    let swiped = message_scene_select(
        &harness.context,
        select("scene-swipe-1", scenes[1], &scene.id),
    )
    .await
    .expect("scene swipe");
    assert_eq!(text_of(&swiped.message), "A lantern-lit market.");
    assert_eq!(swiped.revision, before + 1, "one commit");
    assert!(!scene_edited(&harness));
    let settings = conversation_settings_get(
        &harness.context,
        dto::ConversationSettingsGetRequest {
            conversation_id: conversation_id.to_string(),
        },
    )
    .await
    .expect("settings");
    assert_eq!(settings.scene.id, Some(scenes[1].to_string()));
    assert_eq!(settings.scene.source, dto::SettingSource::Conversation);
    let replayed = message_scene_select(
        &harness.context,
        select("scene-swipe-1", scenes[1], &scene.id),
    )
    .await
    .expect("replay");
    assert_eq!(replayed.message, swiped.message);
}

#[tokio::test(flavor = "multi_thread")]
async fn search_lowercases_like_unicode_and_includes_the_scene() {
    let harness = harness(Reply::Text("Hello."));
    let (character_id, _) = character_with_scenes(
        harness.context.backend().database(),
        &["A STRAßE in İstanbul."],
    );
    let conversation_id = launch_with(&harness, character_id, "search-unicode").await;
    append(
        &harness,
        conversation_id,
        "search-unicode-1",
        text("İstanbul'a gidiyoruz"),
    );
    append(
        &harness,
        conversation_id,
        "search-unicode-2",
        text("Die Straße ist lang"),
    );
    append(
        &harness,
        conversation_id,
        "search-unicode-3",
        text("KARANLIK bir gece"),
    );
    let found = search(&harness, conversation_id, "  İSTANBUL ", None, None).await;
    assert_eq!(
        hit_texts(&found),
        vec!["A STRAßE in İstanbul.", "İstanbul'a gidiyoruz"]
    );
    assert_eq!(found.items[0].role, dto::MessageRole::Scene);
    assert_eq!(found.items[0].author_participant_id, None);
    assert_eq!(found.items[1].role, dto::MessageRole::User);
    assert!(found.items[1].author_participant_id.is_some());
    assert_eq!(
        hit_texts(&search(&harness, conversation_id, "straße", None, None).await),
        vec!["A STRAßE in İstanbul.", "Die Straße ist lang"]
    );
    assert_eq!(
        hit_texts(&search(&harness, conversation_id, "karanlik", None, None).await),
        vec!["KARANLIK bir gece"],
        "I lowercases to i"
    );
    assert!(
        search(&harness, conversation_id, "karanlık", None, None)
            .await
            .items
            .is_empty(),
        "a dotless ı is not an i"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_blank_search_reads_nothing() {
    let harness = harness(Reply::Text("Hello."));
    let missing = ConversationId::new();
    let page = search(&harness, missing, " \t\n ", None, None).await;
    assert!(page.items.is_empty());
    assert_eq!(page.next_cursor, None);
    let error = conversation_search(
        &harness.context,
        dto::ConversationSearchRequest {
            conversation_id: missing.to_string(),
            query: "tea".into(),
            cursor: None,
            limit: None,
        },
    )
    .await
    .expect_err("a real search reads the chat");
    assert_eq!(error.code, ApiErrorCode::NotFound);
}

#[tokio::test(flavor = "multi_thread")]
async fn search_pages_through_a_chat_longer_than_a_timeline_page() {
    let harness = harness(Reply::Text("Hello."));
    let conversation_id = launch_with(&harness, harness.character_id, "search-long").await;
    for index in 0..250 {
        let body = if [10, 205, 240].contains(&index) {
            format!("needle {index}")
        } else {
            format!("hay {index}")
        };
        append(
            &harness,
            conversation_id,
            &format!("search-long-{index}"),
            text(&body),
        );
    }
    let first = search(&harness, conversation_id, "NEEDLE", None, Some(2)).await;
    assert_eq!(hit_texts(&first), vec!["needle 10", "needle 205"]);
    let second = search(
        &harness,
        conversation_id,
        "needle",
        Some(first.next_cursor.clone().expect("more hits")),
        Some(2),
    )
    .await;
    assert_eq!(hit_texts(&second), vec!["needle 240"]);
    assert_eq!(second.next_cursor, None);
    let exact = search(&harness, conversation_id, "needle", None, Some(3)).await;
    assert_eq!(exact.items.len(), 3);
    assert_eq!(exact.next_cursor, None, "no page past the last hit");
    let count = conversation_message_count(
        &harness.context,
        dto::ConversationRequest {
            conversation_id: conversation_id.to_string(),
        },
    )
    .await
    .expect("count");
    assert_eq!(count.count, 250);
}

/// Answers every provider call with the next queued text.
struct ScriptedReplies(Mutex<Vec<&'static str>>);

#[async_trait::async_trait]
impl InferencePort for ScriptedReplies {
    async fn run(&self, _request: InferenceRequest) -> Result<InferenceOutcome, PortError> {
        let text = self.0.lock().expect("replies").remove(0);
        Ok(InferenceOutcome {
            provider_response_id: None,
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts: vec![MessagePart::Text { text: text.into() }],
                tool_calls: vec![],
                provider_replay: None,
                media: Vec::new(),
            }],
            usage: None,
            finish_reason: lettuce_conversations::FinishReason::Stop,
            provider_finish_reason: Some("stop".into()),
            provider_request_id: None,
            warning_codes: vec![],
        })
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn search_matches_only_the_shown_variant_and_variants_can_be_chosen() {
    let harness = harness(Reply::Text("The lighthouse keeper waves."));
    let conversation_id = launch_with(&harness, harness.character_id, "search-variants").await;
    let accepted = send(
        &harness,
        &conversation_id.to_string(),
        "search-variants-send",
        "Hello",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    let worker = ConversationGenerationWorker::new(harness.context.clone());
    assert!(worker.run_once().await.expect("first reply"));
    let database = harness.context.backend().database();
    let turn = ConversationReader::get_turn(database, accepted.turn_id.parse().expect("turn"))
        .expect("turn");
    let reply = open(&harness, conversation_id)
        .await
        .messages
        .items
        .into_iter()
        .find(|message| message.role == dto::MessageRole::Assistant)
        .expect("reply");
    let reply_id: MessageId = reply.id.parse().expect("id");
    let first_candidate = message_candidates(
        &harness.context,
        dto::MessageHistoryRequest {
            message_id: reply.id.clone(),
            cursor: None,
        },
    )
    .await
    .expect("candidates")
    .items
    .remove(0);
    let conversation = ConversationReader::get(database, conversation_id)
        .expect("conversation")
        .conversation;
    let begun = ConversationRepository::begin_regenerate(
        database,
        &RegenerateCandidate {
            conversation_id,
            branch_id: conversation.active_branch_id,
            message_id: reply_id,
            turn_id: turn.id,
            expected_revision: conversation.revision,
            expected_turn_revision: turn.revision,
            operation: crate::conversation::edit_operation("search-variants-regen".into(), &[b"r"])
                .expect("token"),
            active_candidate_id: first_candidate.id.parse().expect("candidate"),
            guidance: None,
            model_override: None,
            forced_speaker: None,
            swap_roles: false,
        },
        harness.context.now(),
    )
    .expect("regenerate")
    .value;
    let regenerating = context_over(
        harness.context.shared_backend(),
        Arc::new(ScriptedReplies(Mutex::new(vec!["A quiet harbor."]))),
    );
    regenerating
        .backend()
        .conversation_generation_dispatcher()
        .schedule(&begun, regenerating.now())
        .expect("schedule");
    assert!(
        ConversationGenerationWorker::new(regenerating.clone())
            .run_once()
            .await
            .expect("second reply")
    );
    assert!(
        search(&harness, conversation_id, "lighthouse", None, None)
            .await
            .items
            .is_empty(),
        "the variant not shown is not searched"
    );
    assert_eq!(
        hit_texts(&search(&harness, conversation_id, "harbor", None, None).await),
        vec!["A quiet harbor."]
    );
    let chosen = message_candidate_select(
        &harness.context,
        dto::MessageCandidateSelectRequest {
            conversation_id: conversation_id.to_string(),
            message_id: reply.id.clone(),
            expected_revision: revision(&harness, conversation_id),
            candidate_id: first_candidate.id.clone(),
            client_operation_id: "search-variants-choose".into(),
        },
    )
    .await
    .expect("choose");
    assert_eq!(text_of(&chosen.message), "The lighthouse keeper waves.");
    assert_eq!(chosen.message.candidate_count, 2);
    assert_eq!(
        hit_texts(&search(&harness, conversation_id, "lighthouse", None, None).await),
        vec!["The lighthouse keeper waves."]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn messages_around_return_both_cursors_that_continue_the_timeline() {
    let harness = harness(Reply::Text("Hello."));
    let conversation_id = launch_with(&harness, harness.character_id, "around").await;
    let ids = (0..9)
        .map(|index| {
            append(
                &harness,
                conversation_id,
                &format!("around-{index}"),
                text(&format!("message {index}")),
            )
        })
        .collect::<Vec<_>>();
    let window = conversation_messages_around(
        &harness.context,
        dto::ConversationMessagesAroundRequest {
            conversation_id: conversation_id.to_string(),
            message_id: ids[4].to_string(),
            before: 2,
            after: 2,
        },
    )
    .await
    .expect("around");
    assert_eq!(
        window.items.iter().map(text_of).collect::<Vec<_>>(),
        vec![
            "message 2",
            "message 3",
            "message 4",
            "message 5",
            "message 6"
        ]
    );
    let older = conversation_messages(
        &harness.context,
        dto::ConversationMessagesRequest {
            conversation_id: conversation_id.to_string(),
            before_cursor: Some(window.before_cursor.clone().expect("older messages")),
            after_cursor: None,
            limit: Some(10),
        },
    )
    .await
    .expect("older page");
    assert_eq!(
        older.items.iter().map(text_of).collect::<Vec<_>>(),
        vec!["message 0", "message 1"]
    );
    assert_eq!(older.next_cursor, None);
    let newer = conversation_messages(
        &harness.context,
        dto::ConversationMessagesRequest {
            conversation_id: conversation_id.to_string(),
            before_cursor: None,
            after_cursor: Some(window.after_cursor.clone().expect("newer messages")),
            limit: Some(1),
        },
    )
    .await
    .expect("newer page");
    assert_eq!(
        newer.items.iter().map(text_of).collect::<Vec<_>>(),
        vec!["message 7"]
    );
    let last = conversation_messages(
        &harness.context,
        dto::ConversationMessagesRequest {
            conversation_id: conversation_id.to_string(),
            before_cursor: None,
            after_cursor: newer.next_cursor.clone(),
            limit: Some(5),
        },
    )
    .await
    .expect("last page");
    assert_eq!(
        last.items.iter().map(text_of).collect::<Vec<_>>(),
        vec!["message 8"]
    );
    assert_eq!(last.next_cursor, None);
    let edge = conversation_messages_around(
        &harness.context,
        dto::ConversationMessagesAroundRequest {
            conversation_id: conversation_id.to_string(),
            message_id: ids[8].to_string(),
            before: 0,
            after: 3,
        },
    )
    .await
    .expect("around the newest");
    assert_eq!(edge.items.len(), 1);
    assert!(edge.before_cursor.is_some());
    assert_eq!(edge.after_cursor, None);
    let missing = conversation_messages_around(
        &harness.context,
        dto::ConversationMessagesAroundRequest {
            conversation_id: conversation_id.to_string(),
            message_id: MessageId::new().to_string(),
            before: 1,
            after: 1,
        },
    )
    .await
    .expect_err("a message off the timeline");
    assert_eq!(missing.code, ApiErrorCode::NotFound);
}

#[tokio::test(flavor = "multi_thread")]
async fn empty_suffix_replay_never_deletes_a_later_append() {
    let harness = harness(Reply::Text("Hello."));
    let id = launch_with(&harness, harness.character_id, "empty-suffix").await;
    let anchor = append(&harness, id, "empty-anchor", text("Anchor"));
    let request = delete_after_request(id, anchor, revision(&harness, id), "empty-delete");
    let first = messages_delete_after(&harness.context, request.clone())
        .await
        .expect("empty delete");
    assert!(removed(&first).is_empty());
    let later = append(&harness, id, "empty-later", text("Later"));
    let replay = messages_delete_after(&harness.context, request.clone())
        .await
        .expect("durable replay");
    assert!(removed(&replay).is_empty());
    assert!(
        open(&harness, id)
            .await
            .messages
            .items
            .iter()
            .any(|item| item.id == later.to_string())
    );
    let conflict = messages_delete_after(
        &harness.context,
        dto::MessageDeleteRequest {
            message_id: later.to_string(),
            ..request
        },
    )
    .await
    .expect_err("same key with a new anchor");
    assert_eq!(conflict.code, ApiErrorCode::Conflict);
}

/// A chat whose root branch holds `root` and a child branch forked at the
/// last of them holds `child`; the child is selected.
struct ForkedChat {
    conversation_id: ConversationId,
    root: Vec<MessageId>,
    child: Vec<MessageId>,
    child_branch: lettuce_types::ConversationBranchId,
}

async fn forked_chat(harness: &Harness, key: &str) -> ForkedChat {
    let conversation_id = launch_with(harness, harness.character_id, key).await;
    let root = vec![
        append(
            harness,
            conversation_id,
            &format!("{key}-r1"),
            text("Root one"),
        ),
        append(
            harness,
            conversation_id,
            &format!("{key}-r2"),
            text("Root two"),
        ),
    ];
    let database = harness.context.backend().database();
    let aggregate = ConversationReader::get(database, conversation_id).expect("conversation");
    let forked = ConversationRepository::fork_branch(
        database,
        &ForkBranch {
            conversation_id,
            source_branch_id: aggregate.conversation.active_branch_id,
            at_message_id: root.last().copied(),
            expected_revision: aggregate.conversation.revision,
            operation: crate::conversation::edit_operation(
                format!("{key}-fork"),
                &[key.as_bytes()],
            )
            .expect("token"),
        },
        harness.context.now(),
    )
    .expect("fork");
    let child = vec![
        append(
            harness,
            conversation_id,
            &format!("{key}-c1"),
            text("Child one"),
        ),
        append(
            harness,
            conversation_id,
            &format!("{key}-c2"),
            text("Child two"),
        ),
    ];
    ForkedChat {
        conversation_id,
        root,
        child,
        child_branch: forked.value.branch.id,
    }
}

fn visible_ids(items: &[dto::TimelineMessage]) -> Vec<String> {
    items.iter().map(|message| message.id.clone()).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_after_before_the_fork_point_selects_a_new_branch_and_deletes_nothing() {
    let harness = harness(Reply::Text("Hello."));
    let chat = forked_chat(&harness, "fork-anchor").await;
    let database = harness.context.backend().database();
    let before = ConversationReader::get(database, chat.conversation_id).expect("conversation");
    let anchor = chat.root[0];
    let request = delete_after_request(
        chat.conversation_id,
        anchor,
        before.conversation.revision.get(),
        "fork-anchor-1",
    );
    let result = messages_delete_after(&harness.context, request.clone())
        .await
        .expect("delete after");
    let dto::MessagesDeleteOutcome::Branched { branch_id } = result.outcome.clone() else {
        panic!("expected a new branch");
    };
    let after = ConversationReader::get(database, chat.conversation_id).expect("conversation");
    assert_eq!(after.conversation.active_branch_id.to_string(), branch_id);
    assert_eq!(result.revision, after.conversation.revision.get());
    assert!(result.revision > before.conversation.revision.get());
    let created = after
        .branches
        .iter()
        .find(|branch| branch.id.to_string() == branch_id)
        .expect("new branch");
    assert_eq!(created.fork_message_id, Some(anchor));
    let root_branch = before
        .branches
        .iter()
        .find(|branch| branch.parent_branch_id.is_none())
        .expect("root")
        .id;
    assert_eq!(created.parent_branch_id, Some(root_branch));
    assert_eq!(created.head_message_id, None);
    let old_child = after
        .branches
        .iter()
        .find(|branch| branch.id == chat.child_branch)
        .expect("old child");
    let old_child_before = before
        .branches
        .iter()
        .find(|branch| branch.id == chat.child_branch)
        .expect("old child before");
    assert_eq!(old_child, old_child_before);
    for message_id in chat.root.iter().chain(&chat.child) {
        let anchor = on_timeline_of(
            &harness,
            chat.conversation_id,
            chat.child_branch,
            *message_id,
        );
        assert_eq!(anchor, MessageVisibility::Visible);
    }
    assert_eq!(
        visible_ids(&open(&harness, chat.conversation_id).await.messages.items),
        vec![anchor.to_string()]
    );
    let replay = messages_delete_after(&harness.context, request.clone())
        .await
        .expect("replay");
    assert_eq!(replay.outcome, result.outcome);
    assert_eq!(
        ConversationReader::get(database, chat.conversation_id)
            .expect("conversation")
            .branches
            .len(),
        after.branches.len()
    );
    let conflict = messages_delete_after(
        &harness.context,
        dto::MessageDeleteRequest {
            message_id: chat.root[1].to_string(),
            ..request
        },
    )
    .await
    .expect_err("same key, another anchor");
    assert_eq!(conflict.code, ApiErrorCode::Conflict);
}

fn on_timeline_of(
    harness: &Harness,
    conversation_id: ConversationId,
    branch_id: lettuce_types::ConversationBranchId,
    message_id: MessageId,
) -> MessageVisibility {
    ConversationOverviewReader::timeline_anchor(
        harness.context.backend().database(),
        conversation_id,
        branch_id,
        message_id,
    )
    .expect("message on the timeline")
    .item
    .message
    .visibility
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_after_inside_the_child_branch_tombstones_its_suffix() {
    let harness = harness(Reply::Text("Hello."));
    let chat = forked_chat(&harness, "fork-inside").await;
    let database = harness.context.backend().database();
    let before = ConversationReader::get(database, chat.conversation_id).expect("conversation");
    let request = delete_after_request(
        chat.conversation_id,
        chat.child[0],
        before.conversation.revision.get(),
        "fork-inside-1",
    );
    let result = messages_delete_after(&harness.context, request.clone())
        .await
        .expect("delete after");
    assert_eq!(removed(&result), vec![chat.child[1].to_string()]);
    let after = ConversationReader::get(database, chat.conversation_id).expect("conversation");
    assert_eq!(after.branches.len(), before.branches.len());
    assert_eq!(after.conversation.active_branch_id, chat.child_branch);
    let shown = visible_ids(&open(&harness, chat.conversation_id).await.messages.items);
    assert!(!shown.contains(&chat.child[1].to_string()));
    for kept in [chat.root[0], chat.root[1], chat.child[0]] {
        assert!(shown.contains(&kept.to_string()));
    }
    let replay = messages_delete_after(&harness.context, request)
        .await
        .expect("replay");
    assert_eq!(replay.outcome, result.outcome);
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_after_at_the_fork_message_tombstones_the_child_branch_messages() {
    let harness = harness(Reply::Text("Hello."));
    let chat = forked_chat(&harness, "fork-point").await;
    let database = harness.context.backend().database();
    let revision = revision(&harness, chat.conversation_id);
    let result = messages_delete_after(
        &harness.context,
        delete_after_request(chat.conversation_id, chat.root[1], revision, "fork-point-1"),
    )
    .await
    .expect("delete after");
    let mut expected = chat
        .child
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let mut actual = removed(&result);
    expected.sort();
    actual.sort();
    assert_eq!(actual, expected);
    assert_eq!(
        ConversationReader::get(database, chat.conversation_id)
            .expect("conversation")
            .conversation
            .active_branch_id,
        chat.child_branch
    );
}

fn branch_ids(
    harness: &Harness,
    conversation_id: ConversationId,
) -> Vec<lettuce_types::ConversationBranchId> {
    ConversationReader::get(harness.context.backend().database(), conversation_id)
        .expect("conversation")
        .branches
        .iter()
        .map(|branch| branch.id)
        .collect()
}

fn fork_at(
    harness: &Harness,
    conversation_id: ConversationId,
    source: lettuce_types::ConversationBranchId,
    at: MessageId,
    key: &str,
) -> lettuce_types::ConversationBranchId {
    let database = harness.context.backend().database();
    let conversation = ConversationReader::get(database, conversation_id)
        .expect("conversation")
        .conversation;
    ConversationRepository::fork_branch(
        database,
        &ForkBranch {
            conversation_id,
            source_branch_id: source,
            at_message_id: Some(at),
            expected_revision: conversation.revision,
            operation: crate::conversation::edit_operation(key.into(), &[key.as_bytes()])
                .expect("token"),
        },
        harness.context.now(),
    )
    .expect("fork")
    .value
    .branch
    .id
}

fn select(
    harness: &Harness,
    conversation_id: ConversationId,
    branch_id: lettuce_types::ConversationBranchId,
    key: &str,
) {
    let database = harness.context.backend().database();
    let conversation = ConversationReader::get(database, conversation_id)
        .expect("conversation")
        .conversation;
    ConversationRepository::select_branch(
        database,
        &lettuce_conversations::SelectBranch {
            conversation_id,
            branch_id,
            expected_revision: conversation.revision,
            operation: crate::conversation::edit_operation(key.into(), &[key.as_bytes()])
                .expect("token"),
        },
        harness.context.now(),
    )
    .expect("select");
}

fn active_branch_of(
    harness: &Harness,
    conversation_id: ConversationId,
) -> lettuce_types::ConversationBranchId {
    ConversationReader::get(harness.context.backend().database(), conversation_id)
        .expect("conversation")
        .conversation
        .active_branch_id
}

fn shown_on(
    harness: &Harness,
    conversation_id: ConversationId,
    branch_id: lettuce_types::ConversationBranchId,
) -> Vec<(MessageId, String, MessageVisibility)> {
    let database = harness.context.backend().database();
    let mut cursor = None;
    let mut newest_first = Vec::new();
    loop {
        let page = ConversationReader::timeline_page(
            database,
            conversation_id,
            branch_id,
            &lettuce_types::PageRequest {
                cursor,
                limit: lettuce_types::PageLimit::new(50),
            },
        )
        .expect("timeline");
        for item in &page.items {
            let text = item
                .active_revision
                .as_ref()
                .map(|revision| &revision.parts)
                .or(item
                    .active_candidate
                    .as_ref()
                    .map(|candidate| &candidate.parts))
                .and_then(|parts| {
                    parts.iter().find_map(|part| match part {
                        MessagePart::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                })
                .unwrap_or_default();
            newest_first.push((item.message.id, text, item.message.visibility));
        }
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    newest_first.reverse();
    newest_first
}

struct NestedChat {
    conversation_id: ConversationId,
    root_branch: lettuce_types::ConversationBranchId,
    child_branch: lettuce_types::ConversationBranchId,
    nested_branch: lettuce_types::ConversationBranchId,
    root: Vec<MessageId>,
    child: Vec<MessageId>,
}

/// Root r1 r2; child forked at r2 with c1 c2 c3; a branch forked at c2 from
/// the child; the child selected again.
async fn nested_chat(harness: &Harness, key: &str) -> NestedChat {
    let chat = forked_chat(harness, key).await;
    let third = append(
        harness,
        chat.conversation_id,
        &format!("{key}-c3"),
        text("Child three"),
    );
    let root_branch =
        ConversationReader::get(harness.context.backend().database(), chat.conversation_id)
            .expect("conversation")
            .branches
            .iter()
            .find(|branch| branch.parent_branch_id.is_none())
            .expect("root")
            .id;
    let nested_branch = fork_at(
        harness,
        chat.conversation_id,
        chat.child_branch,
        chat.child[1],
        &format!("{key}-nested"),
    );
    select(
        harness,
        chat.conversation_id,
        chat.child_branch,
        &format!("{key}-back"),
    );
    NestedChat {
        conversation_id: chat.conversation_id,
        root_branch,
        child_branch: chat.child_branch,
        nested_branch,
        root: chat.root,
        child: vec![chat.child[0], chat.child[1], third],
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_after_takes_the_new_branch_when_a_nested_branch_shows_the_suffix() {
    let harness = harness(Reply::Text("Hello."));
    let chat = nested_chat(&harness, "nested-after").await;
    let before_child = shown_on(&harness, chat.conversation_id, chat.child_branch);
    let before_nested = shown_on(&harness, chat.conversation_id, chat.nested_branch);
    let request = delete_after_request(
        chat.conversation_id,
        chat.child[0],
        revision(&harness, chat.conversation_id),
        "nested-after-1",
    );
    let result = messages_delete_after(&harness.context, request.clone())
        .await
        .expect("delete after");
    let dto::MessagesDeleteOutcome::Branched { branch_id } = result.outcome.clone() else {
        panic!("a nested branch shows c2, so a branch is forked");
    };
    assert_eq!(
        active_branch_of(&harness, chat.conversation_id).to_string(),
        branch_id
    );
    assert_eq!(
        shown_on(&harness, chat.conversation_id, chat.child_branch),
        before_child
    );
    assert_eq!(
        shown_on(&harness, chat.conversation_id, chat.nested_branch),
        before_nested
    );
    let created =
        ConversationReader::get(harness.context.backend().database(), chat.conversation_id)
            .expect("conversation")
            .branches
            .into_iter()
            .find(|branch| branch.id.to_string() == branch_id)
            .expect("new branch");
    assert_eq!(created.fork_message_id, Some(chat.child[0]));
    assert_eq!(created.parent_branch_id, Some(chat.child_branch));
    let replay = messages_delete_after(&harness.context, request.clone())
        .await
        .expect("replay");
    assert_eq!(replay.outcome, result.outcome);
    let conflict = messages_delete_after(
        &harness.context,
        dto::MessageDeleteRequest {
            message_id: chat.child[1].to_string(),
            ..request
        },
    )
    .await
    .expect_err("another request under the key");
    assert_eq!(conflict.code, ApiErrorCode::Conflict);
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_after_takes_the_new_branch_from_a_grandparent_anchor() {
    let harness = harness(Reply::Text("Hello."));
    let chat = nested_chat(&harness, "grandparent").await;
    select(
        &harness,
        chat.conversation_id,
        chat.nested_branch,
        "grandparent-nested",
    );
    let before_root = shown_on(&harness, chat.conversation_id, chat.root_branch);
    let before_child = shown_on(&harness, chat.conversation_id, chat.child_branch);
    let result = messages_delete_after(
        &harness.context,
        delete_after_request(
            chat.conversation_id,
            chat.root[0],
            revision(&harness, chat.conversation_id),
            "grandparent-1",
        ),
    )
    .await
    .expect("delete after");
    let dto::MessagesDeleteOutcome::Branched { branch_id } = result.outcome else {
        panic!("branched");
    };
    let created =
        ConversationReader::get(harness.context.backend().database(), chat.conversation_id)
            .expect("conversation")
            .branches
            .into_iter()
            .find(|branch| branch.id.to_string() == branch_id)
            .expect("new branch");
    assert_eq!(created.parent_branch_id, Some(chat.root_branch));
    assert_eq!(created.fork_message_id, Some(chat.root[0]));
    assert_eq!(
        shown_on(&harness, chat.conversation_id, chat.root_branch),
        before_root
    );
    assert_eq!(
        shown_on(&harness, chat.conversation_id, chat.child_branch),
        before_child
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn message_delete_of_a_message_a_nested_branch_shows_forks_without_it() {
    let harness = harness(Reply::Text("Hello."));
    let chat = nested_chat(&harness, "delete-nested").await;
    let before_child = shown_on(&harness, chat.conversation_id, chat.child_branch);
    let before_nested = shown_on(&harness, chat.conversation_id, chat.nested_branch);
    let request = delete_after_request(
        chat.conversation_id,
        chat.child[1],
        revision(&harness, chat.conversation_id),
        "delete-nested-1",
    );
    let result = message_delete(&harness.context, request.clone())
        .await
        .expect("delete");
    let dto::MessagesDeleteOutcome::Branched { branch_id } = result.outcome.clone() else {
        panic!("c2 is shown by the nested branch");
    };
    let new_branch =
        ConversationReader::get(harness.context.backend().database(), chat.conversation_id)
            .expect("conversation")
            .branches
            .into_iter()
            .find(|branch| branch.id.to_string() == branch_id)
            .expect("new branch");
    assert_eq!(new_branch.parent_branch_id, Some(chat.child_branch));
    assert_eq!(new_branch.fork_message_id, Some(chat.child[0]));
    assert_eq!(
        active_branch_of(&harness, chat.conversation_id),
        new_branch.id
    );
    assert_eq!(
        shown_on(&harness, chat.conversation_id, chat.child_branch),
        before_child
    );
    assert_eq!(
        shown_on(&harness, chat.conversation_id, chat.nested_branch),
        before_nested
    );
    let shown = shown_on(&harness, chat.conversation_id, new_branch.id);
    let texts = shown
        .iter()
        .map(|(_, text, _)| text.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        texts[texts.len() - 3..],
        ["Root two", "Child one", "Child three"]
    );
    let copy = shown.last().expect("copy");
    assert_ne!(copy.0, chat.child[2]);
    assert!(shown.iter().all(|(id, _, _)| *id != chat.child[1]));
    let replay = message_delete(&harness.context, request.clone())
        .await
        .expect("replay");
    assert_eq!(replay.outcome, result.outcome);
    let conflict = message_delete(
        &harness.context,
        dto::MessageDeleteRequest {
            message_id: chat.child[2].to_string(),
            ..request
        },
    )
    .await
    .expect_err("another request under the key");
    assert_eq!(conflict.code, ApiErrorCode::Conflict);
}

#[tokio::test(flavor = "multi_thread")]
async fn message_delete_of_an_ancestor_message_forks_at_its_parent() {
    let harness = harness(Reply::Text("Hello."));
    let chat = forked_chat(&harness, "delete-ancestor").await;
    let root_branch =
        ConversationReader::get(harness.context.backend().database(), chat.conversation_id)
            .expect("conversation")
            .branches
            .iter()
            .find(|branch| branch.parent_branch_id.is_none())
            .expect("root")
            .id;
    let before_root = shown_on(&harness, chat.conversation_id, root_branch);
    let before_child = shown_on(&harness, chat.conversation_id, chat.child_branch);
    let result = message_delete(
        &harness.context,
        delete_after_request(
            chat.conversation_id,
            chat.root[1],
            revision(&harness, chat.conversation_id),
            "delete-ancestor-1",
        ),
    )
    .await
    .expect("delete");
    let dto::MessagesDeleteOutcome::Branched { branch_id } = result.outcome else {
        panic!("r2 is shown by the root branch");
    };
    assert_eq!(
        shown_on(&harness, chat.conversation_id, root_branch),
        before_root
    );
    assert_eq!(
        shown_on(&harness, chat.conversation_id, chat.child_branch),
        before_child
    );
    let new_branch =
        ConversationReader::get(harness.context.backend().database(), chat.conversation_id)
            .expect("conversation")
            .branches
            .into_iter()
            .find(|branch| branch.id.to_string() == branch_id)
            .expect("new branch");
    assert_eq!(new_branch.fork_message_id, Some(chat.root[0]));
    let texts = shown_on(&harness, chat.conversation_id, new_branch.id)
        .into_iter()
        .map(|(_, text, _)| text)
        .collect::<Vec<_>>();
    assert_eq!(
        texts[texts.len() - 3..],
        ["Root one", "Child one", "Child two"]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn message_delete_of_an_exclusive_message_still_tombstones() {
    let harness = harness(Reply::Text("Hello."));
    let chat = nested_chat(&harness, "delete-exclusive").await;
    let branches = branch_ids(&harness, chat.conversation_id).len();
    let result = message_delete(
        &harness.context,
        delete_after_request(
            chat.conversation_id,
            chat.child[2],
            revision(&harness, chat.conversation_id),
            "delete-exclusive-1",
        ),
    )
    .await
    .expect("delete");
    assert_eq!(
        result.outcome,
        dto::MessagesDeleteOutcome::Tombstoned {
            removed: vec![chat.child[2].to_string()]
        }
    );
    assert_eq!(branch_ids(&harness, chat.conversation_id).len(), branches);
    assert_eq!(
        active_branch_of(&harness, chat.conversation_id),
        chat.child_branch
    );
}

fn memory_revision(harness: &Harness, conversation_id: ConversationId) -> Revision {
    MemoryRepository::get_for_conversation(harness.context.backend().database(), conversation_id)
        .expect("memory")
        .expect("memory space")
        .revision
}

fn pending_of(harness: &Harness, conversation_id: ConversationId) -> Vec<PendingSuffixRewind> {
    PendingSuffixRewindRepository::pending_suffix_rewinds(
        harness.context.backend().database(),
        Some(conversation_id),
    )
    .expect("pending")
}

#[tokio::test(flavor = "multi_thread")]
async fn an_owed_rewind_finishes_before_a_delete_after_with_an_earlier_anchor() {
    let harness = harness(Reply::Text("Hello."));
    let chat = memory_chat_with(&harness, "owed-two", true).await;
    let lead = chat.lead.expect("lead message");
    let owed = tombstone_without_rewind(&harness, &chat, "owed-two-1");
    let before = memory_revision(&harness, chat.conversation_id);
    let result = messages_delete_after(
        &harness.context,
        delete_after_request(
            chat.conversation_id,
            lead,
            revision(&harness, chat.conversation_id),
            "owed-two-2",
        ),
    )
    .await
    .expect("second delete-after");
    assert_eq!(removed(&result), vec![chat.first.to_string()]);
    let receipt = rewind_receipt(&harness, chat.conversation_id, &owed.tombstone.operation)
        .expect("the owed rewind finished first");
    assert_eq!(receipt.invalid_run_id, Some(chat.run_id));
    assert!(pending_of(&harness, chat.conversation_id).is_empty());
    assert_eq!(memory_revision(&harness, chat.conversation_id), before);
    let shown = open(&harness, chat.conversation_id).await.messages.items;
    assert_eq!(visible_ids(&shown), vec![lead.to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failing_owed_rewind_is_recorded_per_chat_and_startup_goes_on() {
    let harness = harness(Reply::Text("Hello."));
    let broken = memory_chat(&harness, "owed-broken").await;
    let healthy = memory_chat(&harness, "owed-healthy").await;
    let broken_owed = tombstone_without_rewind(&harness, &broken, "owed-broken-1");
    let healthy_owed = tombstone_without_rewind(&harness, &healthy, "owed-healthy-1");
    let database = harness.context.backend().database();
    let record = ConversationReader::operation_record(
        database,
        broken.conversation_id,
        OperationKind::Tombstone,
        &broken_owed.tombstone.operation,
    )
    .expect("record")
    .expect("tombstone record");
    let earlier = extra_memory_run(&harness, &broken, broken.first, "owed-broken-extra");
    DynamicMemorySuffixRewindRepository::rewind_dynamic_memory_suffix(
        database,
        lettuce_memory::DynamicMemorySuffixRewind {
            operation_id: OperationId::from_uuid(record.id.as_uuid()),
            conversation_id: broken.conversation_id,
            invalid_run_id: Some(earlier),
            expected_memory_revision: memory_revision(&harness, broken.conversation_id),
            invalidated_effect_ids: Vec::new(),
            at: harness.context.now(),
        },
    )
    .expect("a receipt that disagrees with the owed rewind");
    super::startup::complete_pending_rewinds(&harness.context)
        .await
        .expect("startup goes on");
    assert!(
        rewind_receipt(
            &harness,
            healthy.conversation_id,
            &healthy_owed.tombstone.operation
        )
        .is_some()
    );
    assert!(pending_of(&harness, healthy.conversation_id).is_empty());
    assert_eq!(pending_of(&harness, broken.conversation_id).len(), 1);
    assert!(
        PendingSuffixRewindRepository::pending_rewind_failure(database, broken.conversation_id)
            .expect("failure")
            .is_some()
    );
    assert!(
        PendingSuffixRewindRepository::pending_rewind_failure(database, healthy.conversation_id)
            .expect("failure")
            .is_none()
    );
    let still = memory_rewind_retry(
        &harness.context,
        dto::MemoryRewindRetryRequest {
            conversation_id: broken.conversation_id.to_string(),
        },
    )
    .await
    .expect("retry");
    assert_eq!(
        still,
        dto::MemoryRewindRetryOutcome::StillFailing {
            code: dto::MemoryRewindFailureCode::Inconsistent
        }
    );
    assert_eq!(
        open(&harness, broken.conversation_id).await.memory_blocked,
        Some(dto::MemoryBlockedReason::OwedRewindFailed {
            code: dto::MemoryRewindFailureCode::Inconsistent
        })
    );
    assert_eq!(
        open(&harness, healthy.conversation_id).await.memory_blocked,
        None
    );
    let error = messages_delete_after(
        &harness.context,
        delete_after_request(
            broken.conversation_id,
            broken.first,
            revision(&harness, broken.conversation_id),
            "owed-broken-2",
        ),
    )
    .await
    .expect_err("the owed rewind blocks the chat's next delete-after");
    assert_eq!(error.code, ApiErrorCode::Unavailable);
    assert_eq!(
        error.details,
        Some(ApiErrorDetails::PendingMemoryRewind {
            conversation_id: broken.conversation_id.to_string()
        })
    );
    let embedding = harness.context.embedding();
    let admission = harness
        .context
        .backend()
        .companion_memory_host(embedding.as_ref(), harness.context.inference())
        .after_turn(
            broken.conversation_id,
            lettuce_conversations::GenerationOperation::Send,
            WorkerId::new(),
            harness.context.now(),
            std::time::Duration::from_secs(60),
            &ResourceAvailability::all(),
        );
    assert!(matches!(
        admission,
        Err(crate::CompanionMemoryHostError::PendingRewind(
            crate::DynamicMemoryDeleteAfterError::OwedRewind { .. }
        ))
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_crash_after_the_rewind_before_the_owed_record_is_cleared_replays_the_receipt() {
    let harness = harness(Reply::Text("Hello."));
    let chat = memory_chat(&harness, "crash-after-rewind").await;
    let database = harness.context.backend().database();
    let token = delete_after_token(chat.conversation_id, chat.first, "crash-after-rewind-1");
    let command = crate::DeleteAfterMessages {
        conversation_id: chat.conversation_id,
        after_message_id: chat.first,
        expected_revision: Revision::new(revision(&harness, chat.conversation_id)),
        operation: token.clone(),
        summary_message_interval: 20,
    };
    let cancelled = JobStore::append_and_transition(
        database,
        JobMutation::RequestCancellation {
            id: chat.job_id,
            reason: lettuce_jobs::CancellationReason::User,
            at: harness.context.now(),
        },
    )
    .expect("request cancellation");
    JobStore::append_and_transition(
        database,
        JobMutation::RequestCleanup {
            claim: chat.claim.clone(),
            at: cancelled.updated_at,
        },
    )
    .expect("cleanup");
    JobStore::append_and_transition(
        database,
        JobMutation::FinishCancellation {
            claim: chat.claim.clone(),
            at: cancelled.updated_at,
        },
    )
    .expect("finish");
    crate::DynamicMemoryDeleteAfterCoordinator::new(database, database)
        .rewind_without_clearing(&command, chat.second, harness.context.now())
        .expect("rewind without clearing");
    let receipt = rewind_receipt(&harness, chat.conversation_id, &token).expect("receipt");
    let memory = memory_revision(&harness, chat.conversation_id);
    assert_eq!(pending_of(&harness, chat.conversation_id).len(), 1);
    super::startup::complete_pending_rewinds(&harness.context)
        .await
        .expect("startup");
    assert!(pending_of(&harness, chat.conversation_id).is_empty());
    assert_eq!(
        rewind_receipt(&harness, chat.conversation_id, &token),
        Some(receipt)
    );
    assert_eq!(memory_revision(&harness, chat.conversation_id), memory);
}

#[tokio::test(flavor = "multi_thread")]
async fn admission_and_the_api_path_never_rewind_twice() {
    let harness = harness(Reply::Text("Hello."));
    let chat = memory_chat(&harness, "no-double-rewind").await;
    let pending = tombstone_without_rewind(&harness, &chat, "no-double-rewind-1");
    let embedding = harness.context.embedding();
    harness
        .context
        .backend()
        .companion_memory_host(embedding.as_ref(), harness.context.inference())
        .after_turn(
            chat.conversation_id,
            lettuce_conversations::GenerationOperation::Send,
            WorkerId::new(),
            harness.context.now(),
            std::time::Duration::from_secs(60),
            &ResourceAvailability::all(),
        )
        .expect("admission finished the owed rewind");
    let receipt = rewind_receipt(&harness, chat.conversation_id, &pending.tombstone.operation)
        .expect("receipt");
    let memory = memory_revision(&harness, chat.conversation_id);
    let replay = messages_delete_after(
        &harness.context,
        delete_after_request(
            chat.conversation_id,
            chat.first,
            revision(&harness, chat.conversation_id),
            "no-double-rewind-1",
        ),
    )
    .await
    .expect("the api path completes against the finished rewind");
    assert_eq!(removed(&replay), vec![chat.second.to_string()]);
    assert_eq!(
        rewind_receipt(&harness, chat.conversation_id, &pending.tombstone.operation),
        Some(receipt)
    );
    assert_eq!(memory_revision(&harness, chat.conversation_id), memory);
    assert!(pending_of(&harness, chat.conversation_id).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_search_cursor_survives_a_delete_after_between_pages() {
    let harness = harness(Reply::Text("Hello."));
    let conversation_id = launch_with(&harness, harness.character_id, "search-between").await;
    let mut ids = Vec::new();
    for index in 0..5 {
        ids.push(append(
            &harness,
            conversation_id,
            &format!("search-between-{index}"),
            text(&format!("needle {index}")),
        ));
    }
    let first = search(&harness, conversation_id, "needle", None, Some(2)).await;
    assert_eq!(hit_texts(&first), vec!["needle 0", "needle 1"]);
    let cursor = first.next_cursor.clone().expect("more hits");
    messages_delete_after(
        &harness.context,
        delete_after_request(
            conversation_id,
            ids[0],
            revision(&harness, conversation_id),
            "search-between-delete",
        ),
    )
    .await
    .expect("delete after the first hit");
    let second = search(&harness, conversation_id, "needle", Some(cursor), Some(2)).await;
    assert!(second.items.is_empty());
    assert_eq!(second.next_cursor, None);
}

/// A second memory run of the chat whose source is `source`, on its own job.
fn extra_memory_run(
    harness: &Harness,
    chat: &MemoryChat,
    source: MessageId,
    key: &str,
) -> DynamicMemoryRunId {
    let database = harness.context.backend().database();
    let memory = MemoryRepository::get_for_conversation(database, chat.conversation_id)
        .expect("memory")
        .expect("space");
    let job = JobStore::create_or_get(
        database,
        JobSpec::new(
            JobKind::MemoryExtraction,
            JobSubject::new(
                SubjectKind::Conversation,
                format!("{}-{key}", chat.conversation_id),
            )
            .expect("subject"),
            OutcomeRef::Conversation(chat.conversation_id),
        )
        .with_resources(vec![ResourceClass::Cpu]),
    )
    .expect("job")
    .job;
    let item = ConversationOverviewReader::timeline_anchor(
        database,
        chat.conversation_id,
        active_branch_of(harness, chat.conversation_id),
        source,
    )
    .expect("source")
    .item;
    let run_id = DynamicMemoryRunId::new();
    database
        .admit_dynamic_memory_run_attempt(NewDynamicMemoryRunAttempt {
            run_id,
            attempt_id: DynamicMemoryAttemptId::new(),
            conversation_id: chat.conversation_id,
            branch_id: item.message.branch_id,
            space_id: memory.id,
            cycle_start_change: None,
            starting_memory: memory,
            source_messages: vec![DynamicMemorySourceMessage {
                message_id: source,
                role: item.message.role,
                render_source: item.message.active_render_source,
                effective_time: item.message.effective_time,
            }],
            profile: crate::companion::companion_memory_run::tests::profile(),
            time_awareness_enabled: false,
            supersession_enabled: false,
            structured_fallback_format: lettuce_memory::DynamicMemoryStructuredFallbackFormat::Xml,
            summary_window: lettuce_memory::DynamicMemorySummaryWindow {
                message_interval: 1,
                start: 0,
                end: 1,
            },
            tool_request: lettuce_memory::dynamic_memory_tool_request_for_run(
                lettuce_memory::DynamicMemoryToolOptions {
                    group: false,
                    supersession_enabled: false,
                    require_source_message_id: false,
                },
                &|key| key.to_owned(),
            ),
            job_id: job.id,
            now: harness.context.now(),
        })
        .expect("extra memory run");
    run_id
}

fn items_on(
    harness: &Harness,
    conversation_id: ConversationId,
    branch_id: lettuce_types::ConversationBranchId,
) -> Vec<lettuce_conversations::TimelineItem> {
    let database = harness.context.backend().database();
    let mut cursor = None;
    let mut newest_first = Vec::new();
    loop {
        let page = ConversationReader::timeline_page(
            database,
            conversation_id,
            branch_id,
            &lettuce_types::PageRequest {
                cursor,
                limit: lettuce_types::PageLimit::new(50),
            },
        )
        .expect("timeline");
        newest_first.extend(page.items.iter().cloned());
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    newest_first.reverse();
    newest_first
}

fn shown_parts_of(item: &lettuce_conversations::TimelineItem) -> Vec<MessagePart> {
    item.active_revision
        .as_ref()
        .map(|revision| revision.parts.clone())
        .or_else(|| {
            item.active_candidate
                .as_ref()
                .map(|candidate| candidate.parts.clone())
        })
        .expect("a render")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_branched_delete_copies_media_pin_author_time_and_the_shown_variant() {
    let harness = harness(Reply::Text("Copied reply."));
    let conversation_id = launch_with(&harness, harness.character_id, "fidelity").await;
    let database = harness.context.backend().database();
    append(&harness, conversation_id, "fidelity-first", text("First"));
    let middle = append(&harness, conversation_id, "fidelity-middle", text("Middle"));
    let asset = image_asset(database, "ab");
    let with_image = append(
        &harness,
        conversation_id,
        "fidelity-image",
        vec![
            MessagePart::Text {
                text: "Look".into(),
            },
            MessagePart::MediaAsset {
                asset_id: asset,
                role: MediaAssetRole::Attachment,
            },
        ],
    );
    message_pin(
        &harness.context,
        dto::MessagePinRequest {
            conversation_id: conversation_id.to_string(),
            message_id: with_image.to_string(),
            expected_revision: revision(&harness, conversation_id),
            pinned: true,
            client_operation_id: "fidelity-pin".into(),
        },
    )
    .await
    .expect("pin");
    send(
        &harness,
        &conversation_id.to_string(),
        "fidelity-send",
        "Question",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    assert!(
        ConversationGenerationWorker::new(harness.context.clone())
            .run_once()
            .await
            .expect("reply")
    );
    let root = active_branch_of(&harness, conversation_id);
    fork_at(&harness, conversation_id, root, middle, "fidelity-fork");
    select(&harness, conversation_id, root, "fidelity-back");
    let before = items_on(&harness, conversation_id, root);
    let position = before
        .iter()
        .position(|item| item.message.id == middle)
        .expect("middle");
    let originals = before[position + 1..].to_vec();
    assert_eq!(originals.len(), 3);
    assert!(originals[2].active_candidate.is_some());
    let deleted = ConversationRepository::delete_message(
        database,
        &TombstoneMessage {
            conversation_id,
            message_id: middle,
            expected_revision: Revision::new(revision(&harness, conversation_id)),
            operation: crate::conversation::edit_operation("fidelity-delete".into(), &[b"d"])
                .expect("token"),
            descendants: DescendantPolicy::Preserve,
        },
        harness.context.now(),
    )
    .expect("delete");
    let lettuce_conversations::DeleteMessageOutcome::Branched(branched) = deleted.value else {
        panic!("another branch shows the message");
    };
    assert!(deleted.outbox.iter().all(|record| !matches!(
        record.event,
        lettuce_conversations::ConversationOutboxEvent::AssetReferencesChanged { .. }
    )));
    let after = items_on(&harness, conversation_id, branched.branch.id);
    let copies = after[after.len() - 3..].to_vec();
    for (copy, original) in copies.iter().zip(&originals) {
        assert_ne!(copy.message.id, original.message.id);
        assert_eq!(
            copy.message.author_participant_id,
            original.message.author_participant_id
        );
        assert_eq!(copy.message.role, original.message.role);
        assert_eq!(copy.message.created_at, original.message.created_at);
        assert_eq!(copy.message.effective_time, original.message.effective_time);
        assert_eq!(copy.message.pinned, original.message.pinned);
        assert_eq!(copy.message.scene_edited, original.message.scene_edited);
        assert_eq!(copy.message.visibility, original.message.visibility);
        assert_eq!(shown_parts_of(copy), shown_parts_of(original));
    }
    assert!(copies[0].message.pinned);
    assert!(copies[2].active_candidate.is_none());
    assert!(copies[2].active_revision.is_some());
    assert_eq!(
        items_on(&harness, conversation_id, root).len(),
        before.len(),
        "the original branch is unchanged"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_a_root_message_another_branch_shows_is_refused() {
    let harness = harness(Reply::Text("Hello."));
    let conversation_id = launch_with(&harness, harness.character_id, "root-delete").await;
    let first = append(&harness, conversation_id, "root-delete-1", text("Root"));
    let second = append(&harness, conversation_id, "root-delete-2", text("Second"));
    let root = active_branch_of(&harness, conversation_id);
    fork_at(&harness, conversation_id, root, second, "root-delete-fork");
    let branches = branch_ids(&harness, conversation_id);
    let error = message_delete(
        &harness.context,
        delete_after_request(
            conversation_id,
            first,
            revision(&harness, conversation_id),
            "root-delete-x",
        ),
    )
    .await
    .expect_err("no branch can start before the first message");
    assert_eq!(error.code, ApiErrorCode::Unsupported);
    assert_eq!(branch_ids(&harness, conversation_id), branches);
    let shown = visible_ids(&open(&harness, conversation_id).await.messages.items);
    assert!(shown.contains(&first.to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_owed_rewind_shows_on_the_view_and_list_and_a_retry_clears_it() {
    let harness = harness(Reply::Text("Hello."));
    let blocked = memory_chat(&harness, "blocked").await;
    let other = memory_chat(&harness, "unblocked").await;
    let database = harness.context.backend().database();
    let owed = tombstone_without_rewind(&harness, &blocked, "blocked-1");
    let before = ConversationChangeFeed::conversation_change_position(database).expect("position");
    PendingSuffixRewindRepository::fail_pending_suffix_rewind(
        database,
        &owed,
        lettuce_memory::OwedRewindFailure::Conflict,
    )
    .expect("record the failure");
    let recorded =
        ConversationChangeFeed::conversation_change_position(database).expect("position");
    assert!(
        recorded > before,
        "recording the failure reaches the change feed"
    );
    let expected = Some(dto::MemoryBlockedReason::OwedRewindFailed {
        code: dto::MemoryRewindFailureCode::Conflict,
    });
    assert_eq!(
        open(&harness, blocked.conversation_id).await.memory_blocked,
        expected
    );
    assert_eq!(
        open(&harness, other.conversation_id).await.memory_blocked,
        None
    );
    let list = conversations_list(
        &harness.context,
        dto::ConversationsListRequest {
            kind: None,
            character_id: None,
            source_group_id: None,
            lifecycle: None,
            cursor: None,
            limit: None,
        },
    )
    .await
    .expect("list");
    for summary in &list.items {
        let wanted = if summary.id == blocked.conversation_id.to_string() {
            &expected
        } else {
            &None
        };
        assert_eq!(&summary.memory_blocked, wanted);
    }
    let retry = memory_rewind_retry(
        &harness.context,
        dto::MemoryRewindRetryRequest {
            conversation_id: blocked.conversation_id.to_string(),
        },
    )
    .await
    .expect("retry");
    assert_eq!(retry, dto::MemoryRewindRetryOutcome::Completed);
    assert_eq!(
        open(&harness, blocked.conversation_id).await.memory_blocked,
        None
    );
    assert!(pending_of(&harness, blocked.conversation_id).is_empty());
    assert!(
        ConversationChangeFeed::conversation_change_position(database).expect("position")
            > recorded
    );
    let idle = memory_rewind_retry(
        &harness.context,
        dto::MemoryRewindRetryRequest {
            conversation_id: other.conversation_id.to_string(),
        },
    )
    .await
    .expect("retry of a chat that owes nothing");
    assert_eq!(idle, dto::MemoryRewindRetryOutcome::NothingOwed);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failing_owed_rewind_refuses_every_operation_that_begins_a_turn() {
    let harness = harness(Reply::Text("Hello."));
    let broken = memory_chat(&harness, "owed-turns").await;
    let owed = tombstone_without_rewind(&harness, &broken, "owed-turns-1");
    let database = harness.context.backend().database();
    let record = ConversationReader::operation_record(
        database,
        broken.conversation_id,
        OperationKind::Tombstone,
        &owed.tombstone.operation,
    )
    .expect("record")
    .expect("tombstone record");
    let earlier = extra_memory_run(&harness, &broken, broken.first, "owed-turns-extra");
    DynamicMemorySuffixRewindRepository::rewind_dynamic_memory_suffix(
        database,
        lettuce_memory::DynamicMemorySuffixRewind {
            operation_id: OperationId::from_uuid(record.id.as_uuid()),
            conversation_id: broken.conversation_id,
            invalid_run_id: Some(earlier),
            expected_memory_revision: memory_revision(&harness, broken.conversation_id),
            invalidated_effect_ids: Vec::new(),
            at: harness.context.now(),
        },
    )
    .expect("a receipt that disagrees with the owed rewind");
    super::startup::complete_pending_rewinds(&harness.context)
        .await
        .expect("startup goes on");
    assert_eq!(pending_of(&harness, broken.conversation_id).len(), 1);

    let chat = broken.conversation_id.to_string();
    let stream = || Arc::new(RecordingStream::default());
    let revision = revision(&harness, broken.conversation_id);
    let sent = send(&harness, &chat, "owed-turns-send", "Hello", stream())
        .await
        .expect_err("send");
    let regenerated = conversation_regenerate(
        &harness.context,
        dto::ConversationRegenerateRequest {
            conversation_id: chat.clone(),
            message_id: broken.first.to_string(),
            expected_revision: revision,
            client_operation_id: "owed-turns-regen".into(),
            guidance: None,
            model_profile_id: None,
            forced_speaker_participant_id: None,
            swap_places: false,
        },
        stream(),
    )
    .await
    .expect_err("regenerate");
    let continued = conversation_continue(
        &harness.context,
        dto::ConversationContinueRequest {
            conversation_id: chat.clone(),
            expected_revision: revision,
            client_operation_id: "owed-turns-continue".into(),
            forced_speaker_participant_id: None,
            swap_places: false,
        },
        stream(),
    )
    .await
    .expect_err("continue");
    let retried = conversation_retry(
        &harness.context,
        dto::ConversationRetryRequest {
            conversation_id: chat.clone(),
            turn_id: uuid::Uuid::new_v4().to_string(),
            client_operation_id: "owed-turns-retry".into(),
        },
        stream(),
    )
    .await
    .expect_err("retry");
    for error in [sent, regenerated, continued, retried] {
        assert_eq!(error.code, ApiErrorCode::Unavailable, "{error:?}");
        assert_eq!(
            error.details,
            Some(ApiErrorDetails::PendingMemoryRewind {
                conversation_id: chat.clone()
            })
        );
    }
    assert_eq!(pending_of(&harness, broken.conversation_id).len(), 1);
    assert!(
        ConversationOverviewReader::live_turn(database, broken.conversation_id)
            .expect("live turn")
            .is_none()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_owed_rewind_a_crash_left_finishes_before_a_turn_begins() {
    let harness = harness(Reply::Text("Hello."));
    let chat = memory_chat(&harness, "owed-before-turn").await;
    let owed = tombstone_without_rewind(&harness, &chat, "owed-before-turn-1");
    assert!(rewind_receipt(&harness, chat.conversation_id, &owed.tombstone.operation).is_none());
    let text = "Hello again";
    send(
        &harness,
        &chat.conversation_id.to_string(),
        "owed-before-turn-send",
        text,
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("the owed rewind finishes, then the send begins");
    assert!(rewind_receipt(&harness, chat.conversation_id, &owed.tombstone.operation).is_some());
    assert!(pending_of(&harness, chat.conversation_id).is_empty());
}
