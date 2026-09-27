use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use lettuce_characters::StarterRole;
use lettuce_contracts::{self as dto, ApiErrorCode, ApiErrorDetails, ApiEvent};
use lettuce_conversations::{
    ArchiveConversation, ConversationChangeFeed, ConversationReader, ConversationRepository,
    OperationToken,
};
use lettuce_types::{CharacterId, ContentHash, ConversationId, TimestampMillis};

use super::conversation_feed::ConversationFeed;
use super::tests::{Harness, Reply, api_events, harness};
use super::{
    GenerationEventSink, conversation_launch_direct, conversation_launch_group, conversation_open,
    conversation_send, conversations_latest_by_character, conversations_latest_by_group,
    conversations_list,
};
use crate::launch::tests::{message, seed_character, starter_with, text_scene, two_member_group};

struct NoStream;

impl GenerationEventSink for NoStream {
    fn emit(&self, _event: dto::GenerationEvent) {}
}

async fn launch_with(
    harness: &Harness,
    character_id: CharacterId,
    key: &str,
    title: Option<&str>,
    scene_id: Option<String>,
    starter_id: Option<String>,
) -> Result<String, dto::ApiError> {
    conversation_launch_direct(
        &harness.context,
        dto::LaunchDirectRequest {
            character_id: character_id.to_string(),
            title: title.map(str::to_owned),
            scene_id,
            starter_id,
            client_operation_id: key.into(),
        },
    )
    .await
    .map(|launched| launched.conversation_id)
}

async fn launch(harness: &Harness, key: &str) -> String {
    launch_with(harness, harness.character_id, key, None, None, None)
        .await
        .expect("launch")
}

fn archive(harness: &Harness, conversation_id: &str, key: &str) {
    let database = harness.context.backend().database();
    let conversation_id: ConversationId = conversation_id.parse().expect("id");
    let conversation = ConversationReader::get(database, conversation_id)
        .expect("conversation")
        .conversation;
    ConversationRepository::archive(
        database,
        &ArchiveConversation {
            conversation_id,
            expected_revision: conversation.revision,
            operation: OperationToken {
                key: lettuce_jobs::IdempotencyKey::new(key).expect("key"),
                request_digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
            },
        },
        TimestampMillis::new(conversation.updated_at.get() + 1),
    )
    .expect("archive");
}

async fn list(
    harness: &Harness,
    request: dto::ConversationsListRequest,
) -> Vec<dto::ConversationSummary> {
    conversations_list(&harness.context, request)
        .await
        .expect("list")
        .items
}

fn ids(items: &[dto::ConversationSummary]) -> Vec<String> {
    items.iter().map(|item| item.id.clone()).collect()
}

async fn send(harness: &Harness, conversation_id: &str, key: &str, text: &str) {
    conversation_send(
        &harness.context,
        dto::ConversationSendRequest {
            conversation_id: conversation_id.into(),
            text: text.into(),
            client_operation_id: key.into(),
        },
        Arc::new(NoStream),
    )
    .await
    .expect("send");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_send_restores_an_archived_chat_to_the_default_list() {
    let harness = harness(Reply::Text("Hello."));
    let conversation_id = launch(&harness, "archived-launch").await;
    archive(&harness, &conversation_id, "archive-it");
    assert!(
        list(&harness, dto::ConversationsListRequest::default())
            .await
            .is_empty()
    );
    let archived = list(
        &harness,
        dto::ConversationsListRequest {
            lifecycle: Some(dto::LifecycleFilter::Archived),
            ..dto::ConversationsListRequest::default()
        },
    )
    .await;
    assert_eq!(ids(&archived), vec![conversation_id.clone()]);
    assert!(archived[0].archived);
    let view = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: conversation_id.clone(),
        },
    )
    .await
    .expect("an archived chat opens");
    assert!(view.archived);
    assert!(view.can_send);

    send(&harness, &conversation_id, "archived-send", "Back again").await;
    let active = list(&harness, dto::ConversationsListRequest::default()).await;
    assert_eq!(ids(&active), vec![conversation_id.clone()]);
    assert!(!active[0].archived);
    assert_eq!(active[0].message_count, 1);
    let view = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: conversation_id.clone(),
        },
    )
    .await
    .expect("open");
    assert!(!view.archived);
    assert!(!view.messages.items[0].pinned);
    assert!(view.revision > 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn lists_filter_by_character_and_lifecycle_and_cut_previews_at_400_characters() {
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let other = seed_character(database, Vec::new(), Vec::new(), Vec::new(), |_| {});
    let older = launch(&harness, "list-older").await;
    let newer = launch(&harness, "list-newer").await;
    let theirs = launch_with(&harness, other, "list-other", None, None, None)
        .await
        .expect("launch other");
    archive(&harness, &older, "list-archive");
    let long = "é".repeat(500);
    send(&harness, &newer, "list-send", &long).await;

    let mine = list(
        &harness,
        dto::ConversationsListRequest {
            character_id: Some(harness.character_id.to_string()),
            lifecycle: Some(dto::LifecycleFilter::All),
            ..dto::ConversationsListRequest::default()
        },
    )
    .await;
    assert_eq!(ids(&mine), vec![newer.clone(), older.clone()]);
    assert_eq!(
        mine[0].source,
        dto::ConversationSource::Direct {
            character_id: harness.character_id.to_string()
        }
    );
    let preview = mine[0].last_message_preview.clone().expect("preview");
    assert_eq!(preview.chars().count(), 400);
    assert!(preview.chars().all(|character| character == 'é'));
    assert_eq!(mine[0].chat_mode, None);
    assert_eq!(mine[0].missing_models, Vec::new());
    assert!(!ids(&mine).contains(&theirs));

    let groups = list(
        &harness,
        dto::ConversationsListRequest {
            kind: Some(dto::ConversationKind::Group),
            ..dto::ConversationsListRequest::default()
        },
    )
    .await;
    assert!(groups.is_empty());
    let error = conversations_list(
        &harness.context,
        dto::ConversationsListRequest {
            character_id: Some("not-an-id".into()),
            ..dto::ConversationsListRequest::default()
        },
    )
    .await
    .expect_err("a malformed character id");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_latest_chat_per_character_keeps_an_archived_one() {
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let other = seed_character(database, Vec::new(), Vec::new(), Vec::new(), |_| {});
    launch(&harness, "latest-older").await;
    let theirs = launch_with(&harness, other, "latest-other", None, None, None)
        .await
        .expect("launch other");
    let newest = launch(&harness, "latest-newest").await;
    archive(&harness, &newest, "latest-archive");
    let latest = conversations_latest_by_character(
        &harness.context,
        dto::LatestConversationsRequest::default(),
    )
    .await
    .expect("latest");
    assert_eq!(
        latest
            .items
            .iter()
            .map(|item| (item.source_id.clone(), item.conversation.id.clone()))
            .collect::<Vec<_>>(),
        vec![
            (harness.character_id.to_string(), newest.clone()),
            (other.to_string(), theirs),
        ]
    );
    assert!(latest.items[0].conversation.archived);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_group_launch_retried_with_its_key_returns_the_same_chat() {
    let harness = harness(Reply::Text("Hello."));
    let group_id = two_member_group(harness.context.backend().database());
    let request = dto::LaunchGroupRequest {
        group_id: group_id.to_string(),
        client_operation_id: "group-launch".into(),
    };
    let first = conversation_launch_group(&harness.context, request.clone())
        .await
        .expect("launch");
    let again = conversation_launch_group(&harness.context, request)
        .await
        .expect("retry");
    assert_eq!(again.conversation_id, first.conversation_id);
    let chats = list(
        &harness,
        dto::ConversationsListRequest {
            source_group_id: Some(group_id.to_string()),
            ..dto::ConversationsListRequest::default()
        },
    )
    .await;
    assert_eq!(ids(&chats), vec![first.conversation_id.clone()]);
    assert_eq!(chats[0].title, "Cast");
    assert_eq!(
        chats[0].source,
        dto::ConversationSource::Group {
            group_id: group_id.to_string()
        }
    );
    assert!(chats[0].chat_mode.is_some());
    let latest =
        conversations_latest_by_group(&harness.context, dto::LatestConversationsRequest::default())
            .await
            .expect("latest per group");
    assert_eq!(latest.items.len(), 1);
    assert_eq!(latest.items[0].source_id, group_id.to_string());
    let missing = conversation_launch_group(
        &harness.context,
        dto::LaunchGroupRequest {
            group_id: lettuce_types::GroupId::new().to_string(),
            client_operation_id: "group-missing".into(),
        },
    )
    .await
    .expect_err("an unknown group");
    assert_eq!(missing.code, ApiErrorCode::NotFound);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_direct_launch_takes_a_title_scene_and_starter_of_its_character_only() {
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let own_scene = text_scene(CharacterId::new(), 0, "A quiet library.");
    let own_starter = starter_with(
        CharacterId::new(),
        0,
        "Greeting",
        vec![message(StarterRole::Assistant, "Welcome back.")],
    );
    let (own_scene_id, own_starter_id) = (own_scene.id, own_starter.id);
    let character = seed_character(
        database,
        vec![own_scene],
        Vec::new(),
        vec![own_starter],
        |_| {},
    );
    let foreign_scene = text_scene(CharacterId::new(), 0, "Elsewhere.");
    let foreign_starter = starter_with(
        CharacterId::new(),
        0,
        "Other",
        vec![message(StarterRole::Assistant, "Not yours.")],
    );
    let (foreign_scene_id, foreign_starter_id) = (foreign_scene.id, foreign_starter.id);
    seed_character(
        database,
        vec![foreign_scene],
        Vec::new(),
        vec![foreign_starter],
        |_| {},
    );

    let error = launch_with(
        &harness,
        character,
        "foreign-scene",
        None,
        Some(foreign_scene_id.to_string()),
        None,
    )
    .await
    .expect_err("another character's scene");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
    assert_eq!(
        error.details,
        Some(ApiErrorDetails::InvalidField {
            field: "scene_id".into()
        })
    );
    let error = launch_with(
        &harness,
        character,
        "foreign-starter",
        None,
        None,
        Some(foreign_starter_id.to_string()),
    )
    .await
    .expect_err("another character's starter");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
    assert_eq!(
        error.details,
        Some(ApiErrorDetails::InvalidField {
            field: "starter_id".into()
        })
    );

    let titled = launch_with(
        &harness,
        character,
        "own-choices",
        Some("  Library night  "),
        Some(own_scene_id.to_string()),
        Some(own_starter_id.to_string()),
    )
    .await
    .expect("own scene and starter");
    let view = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: titled,
        },
    )
    .await
    .expect("open");
    assert_eq!(view.title, "Library night");
    assert_eq!(
        view.messages
            .items
            .iter()
            .map(|message| message.role)
            .collect::<Vec<_>>(),
        vec![dto::MessageRole::Assistant],
        "a template without a scene replaces the chosen scene"
    );
    let scened = launch_with(
        &harness,
        character,
        "own-scene",
        None,
        Some(own_scene_id.to_string()),
        None,
    )
    .await
    .expect("own scene");
    let view = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: scened,
        },
    )
    .await
    .expect("open");
    assert_eq!(
        view.messages
            .items
            .iter()
            .map(|message| message.role)
            .collect::<Vec<_>>(),
        vec![dto::MessageRole::Scene]
    );
    let untitled = launch_with(&harness, character, "blank-title", Some("   "), None, None)
        .await
        .expect("blank title");
    let view = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: untitled,
        },
    )
    .await
    .expect("open");
    assert_eq!(view.title, "Ada");
}

fn conversation_events(events: &[ApiEvent]) -> Vec<ApiEvent> {
    events
        .iter()
        .filter(|event| {
            matches!(
                event,
                ApiEvent::ConversationChanged { .. } | ApiEvent::ConversationRemoved { .. }
            )
        })
        .cloned()
        .collect()
}

fn run_feed(
    context: &super::ApiContext,
    feed: ConversationFeed,
) -> (
    tokio::sync::oneshot::Sender<()>,
    tokio::task::JoinHandle<()>,
) {
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let context = context.clone();
    let running = tokio::spawn(async move {
        feed.run(context, async move {
            let _ = stopped.await;
        })
        .await;
    });
    (stop, running)
}

/// Lets an hour of paused time pass, far past any coalescing or retry delay.
async fn idle_for_an_hour() {
    tokio::time::sleep(Duration::from_secs(60 * 60)).await;
}

#[tokio::test(start_paused = true)]
async fn conversation_changes_are_published_once_per_commit_and_never_while_idle() {
    let harness = harness(Reply::Text("Hello."));
    let context = harness.context.clone();
    let feed = ConversationFeed::start(&context).await.expect("feed");
    let reads = feed.reads();
    let (stop, running) = run_feed(&context, feed);
    idle_for_an_hour().await;
    assert!(conversation_events(&api_events(&harness)).is_empty());
    assert_eq!(reads.load(Ordering::SeqCst), 0);

    let conversation_id = launch(&harness, "feed-launch").await;
    harness
        .events
        .until(|events| !conversation_events(events).is_empty())
        .await;
    idle_for_an_hour().await;
    assert_eq!(
        conversation_events(&api_events(&harness)),
        vec![ApiEvent::ConversationChanged {
            conversation_id: conversation_id.clone()
        }]
    );
    assert_eq!(reads.load(Ordering::SeqCst), 1);

    let refused = conversation_launch_direct(
        &context,
        dto::LaunchDirectRequest {
            character_id: CharacterId::new().to_string(),
            title: None,
            scene_id: None,
            starter_id: None,
            client_operation_id: "feed-refused".into(),
        },
    )
    .await
    .expect_err("an unknown character writes nothing");
    assert_eq!(refused.code, ApiErrorCode::NotFound);
    idle_for_an_hour().await;
    assert_eq!(conversation_events(&api_events(&harness)).len(), 1);
    assert_eq!(reads.load(Ordering::SeqCst), 1);

    let id: ConversationId = conversation_id.parse().expect("id");
    context
        .backend()
        .database()
        .purge_conversation(id, TimestampMillis::new(10_000))
        .expect("purge");
    harness
        .events
        .until(|events| conversation_events(events).len() == 2)
        .await;
    assert_eq!(
        conversation_events(&api_events(&harness))[1],
        ApiEvent::ConversationRemoved { conversation_id }
    );
    stop.send(()).expect("stop");
    running.await.expect("feed task");
}

#[tokio::test(start_paused = true)]
async fn a_failed_change_read_is_retried_without_another_signal() {
    let harness = harness(Reply::Text("Hello."));
    let context = harness.context.clone();
    let failures = Arc::new(AtomicUsize::new(1));
    let remaining = Arc::clone(&failures);
    let feed = ConversationFeed::start_reading(
        &context,
        Arc::new(move |context: &super::ApiContext, after, limit| {
            if remaining
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                    left.checked_sub(1)
                })
                .is_ok()
            {
                return Err(dto::ApiError {
                    code: ApiErrorCode::Internal,
                    message: "injected read failure".into(),
                    details: None,
                });
            }
            context
                .backend()
                .database()
                .conversation_changes_since(after, limit)
                .map_err(|_| dto::ApiError {
                    code: ApiErrorCode::Internal,
                    message: "change read failed".into(),
                    details: None,
                })
        }),
    )
    .await
    .expect("feed");
    let reads = feed.reads();
    let (stop, running) = run_feed(&context, feed);

    let conversation_id = launch(&harness, "retry-launch").await;
    harness
        .events
        .until(|events| !conversation_events(events).is_empty())
        .await;
    assert_eq!(failures.load(Ordering::SeqCst), 0);
    assert_eq!(reads.load(Ordering::SeqCst), 2);
    assert_eq!(
        conversation_events(&api_events(&harness)),
        vec![ApiEvent::ConversationChanged { conversation_id }]
    );
    idle_for_an_hour().await;
    assert_eq!(
        reads.load(Ordering::SeqCst),
        2,
        "a successful read stops retrying"
    );
    stop.send(()).expect("stop");
    running.await.expect("feed task");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_launch_key_reused_for_another_request_conflicts() {
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let first_scene = text_scene(CharacterId::new(), 0, "A quiet library.");
    let second_scene = text_scene(CharacterId::new(), 1, "A busy market.");
    let starter = starter_with(
        CharacterId::new(),
        0,
        "Greeting",
        vec![message(StarterRole::Assistant, "Welcome back.")],
    );
    let (first_scene_id, second_scene_id, starter_id) =
        (first_scene.id, second_scene.id, starter.id);
    let character = seed_character(
        database,
        vec![first_scene, second_scene],
        Vec::new(),
        vec![starter],
        |_| {},
    );
    let foreign_scene = text_scene(CharacterId::new(), 0, "Elsewhere.");
    let foreign_scene_id = foreign_scene.id;
    seed_character(
        database,
        vec![foreign_scene],
        Vec::new(),
        Vec::new(),
        |_| {},
    );
    let group_id = two_member_group(database);

    let key = "reused-key";
    let launched = launch_with(
        &harness,
        character,
        key,
        Some("Library"),
        Some(first_scene_id.to_string()),
        None,
    )
    .await
    .expect("launch");
    assert_eq!(
        launch_with(
            &harness,
            character,
            key,
            Some("Library"),
            Some(first_scene_id.to_string()),
            None,
        )
        .await
        .expect("the same request replays"),
        launched
    );
    for (label, title, scene, starter) in [
        ("title", Some("Market"), Some(first_scene_id), None),
        ("scene", Some("Library"), Some(second_scene_id), None),
        (
            "foreign scene",
            Some("Library"),
            Some(foreign_scene_id),
            None,
        ),
        (
            "starter",
            Some("Library"),
            Some(first_scene_id),
            Some(starter_id),
        ),
    ] {
        let error = launch_with(
            &harness,
            character,
            key,
            title,
            scene.map(|id| id.to_string()),
            starter.map(|id| id.to_string()),
        )
        .await
        .expect_err(label);
        assert_eq!(error.code, ApiErrorCode::Conflict, "{label}");
    }
    let error = conversation_launch_group(
        &harness.context,
        dto::LaunchGroupRequest {
            group_id: group_id.to_string(),
            client_operation_id: key.into(),
        },
    )
    .await
    .expect_err("a group launch under a direct launch's key");
    assert_eq!(error.code, ApiErrorCode::Conflict);
    let error = conversation_launch_group(
        &harness.context,
        dto::LaunchGroupRequest {
            group_id: lettuce_types::GroupId::new().to_string(),
            client_operation_id: key.into(),
        },
    )
    .await
    .expect_err("an unpreparable group launch under a direct launch's key");
    assert_eq!(error.code, ApiErrorCode::Conflict);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_chat_whose_character_is_gone_needs_no_companion_model() {
    let harness = harness(Reply::Text("Hello."));
    let conversation_id = launch(&harness, "gone-character").await;
    let database = harness.context.backend().database();
    let mut conversation = ConversationReader::get(database, conversation_id.parse().expect("id"))
        .expect("conversation")
        .conversation;
    let lettuce_conversations::ConversationKind::Direct(details) = &mut conversation.kind else {
        panic!("a direct chat");
    };
    details.character.source_id = CharacterId::new();
    let settings = lettuce_settings::GlobalSettingsStore::load(database)
        .expect("settings")
        .settings;
    assert_eq!(
        super::models::required_models(database, &settings, &conversation).expect("required"),
        Vec::new()
    );
}

#[test]
fn a_tombstoned_chat_takes_no_send() {
    use lettuce_conversations::ConversationLifecycle;
    assert!(super::conversations::can_send(
        ConversationLifecycle::Active,
        false,
        true
    ));
    assert!(super::conversations::can_send(
        ConversationLifecycle::Archived,
        false,
        true
    ));
    assert!(!super::conversations::can_send(
        ConversationLifecycle::Tombstoned,
        false,
        true
    ));
    assert!(!super::conversations::can_send(
        ConversationLifecycle::Active,
        true,
        true
    ));
    assert!(!super::conversations::can_send(
        ConversationLifecycle::Active,
        false,
        false
    ));
}
