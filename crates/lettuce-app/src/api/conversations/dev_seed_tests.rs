use std::sync::Arc;

use lettuce_characters::CharacterDefaults;
use lettuce_contracts::{self as dto, ApiErrorCode, ApiErrorDetails};
use lettuce_conversations::{
    Conversation, ConversationKind, ConversationReader, MessagePart, MessageRenderSource,
    MessageRole, ParticipantSource,
};
use lettuce_types::{CharacterId, ConversationId, TimestampMillis};

use crate::api::tests::{Harness, Reply, create_character, harness_in};
use crate::api::*;
use crate::launch::tests::{member, seed_group, seed_named_character};

const NOW: i64 = 1_800_000_000_000;

fn seeded_harness() -> Harness {
    harness_in(
        Reply::Text("Hello."),
        Arc::new(lettuce_jobs::FakeClock::new(TimestampMillis::new(NOW))),
        None,
        None,
        Arc::new(crate::api::app::inspect_tests::AllModels),
    )
}

async fn developer_mode(harness: &Harness, enabled: bool) {
    let view = settings_get(&harness.context).await.expect("settings");
    settings_update(
        &harness.context,
        dto::SettingsUpdateRequest {
            expected_revision: view.revision,
            patch: dto::SettingsPatch::General {
                developer_mode_enabled: Some(enabled),
                pure_mode: None,
                analytics_enabled: None,
                update_checks_enabled: None,
                auto_download_character_card_avatars: None,
                manual_mode_context_window: None,
                lorebook_scan_depth: None,
            },
        },
    )
    .await
    .expect("developer mode");
}

fn line(speaker: Option<CharacterId>, text: &str, minutes_ago: u32) -> dto::DevSeedLine {
    dto::DevSeedLine {
        speaker_character_id: speaker.map(|id| id.to_string()),
        text: text.into(),
        minutes_ago,
    }
}

fn direct(
    character_id: CharacterId,
    lines: Vec<dto::DevSeedLine>,
) -> dto::DevConversationSeedRequest {
    dto::DevConversationSeedRequest {
        target: dto::DevSeedTarget::Direct {
            character_id: character_id.to_string(),
        },
        title: "Dev chat".into(),
        lines,
        relationship: None,
        client_operation_id: "dev-seed".into(),
    }
}

/// The active branch oldest first: role, author, text and effective time.
fn timeline(
    harness: &Harness,
    conversation_id: &str,
) -> (
    Conversation,
    Vec<(
        MessageRole,
        Option<lettuce_types::ConversationParticipantId>,
        String,
        i64,
    )>,
) {
    let database = harness.context.backend().database();
    let id: ConversationId = conversation_id.parse().expect("conversation id");
    let conversation = ConversationReader::get(database, id)
        .expect("conversation")
        .conversation;
    let page = ConversationReader::timeline_page(
        database,
        id,
        conversation.active_branch_id,
        &lettuce_types::PageRequest {
            cursor: None,
            limit: lettuce_types::PageLimit::new(50),
        },
    )
    .expect("timeline");
    let mut items = page
        .items
        .into_iter()
        .map(|item| {
            let parts = match item.message.active_render_source {
                MessageRenderSource::Candidate(_) => item
                    .active_candidate
                    .as_ref()
                    .map(|candidate| candidate.parts.as_slice()),
                MessageRenderSource::Revision(_) => item
                    .active_revision
                    .as_ref()
                    .map(|revision| revision.parts.as_slice()),
            };
            let text = parts
                .unwrap_or_default()
                .iter()
                .filter_map(|part| match part {
                    MessagePart::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<String>();
            (
                item.message.role,
                item.message.author_participant_id,
                text,
                item.message.effective_time.get(),
            )
        })
        .collect::<Vec<_>>();
    items.reverse();
    (conversation, items)
}

fn field(error: &dto::ApiError) -> Option<&str> {
    match &error.details {
        Some(ApiErrorDetails::InvalidField { field }) => Some(field.as_str()),
        _ => None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn seeding_is_refused_without_developer_mode() {
    let harness = seeded_harness();
    let error = dev_conversation_seed(
        &harness.context,
        direct(harness.character_id, vec![line(None, "Hi", 1)]),
    )
    .await
    .expect_err("developer mode is off");
    assert_eq!(error.code, ApiErrorCode::Unsupported);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_direct_seed_keeps_the_lines_in_order_with_their_authors_and_times() {
    let harness = seeded_harness();
    developer_mode(&harness, true).await;
    let ada = harness.character_id;
    let request = direct(
        ada,
        vec![
            line(None, "Morning.", 90),
            line(Some(ada), "Morning to you.", 89),
            line(None, "Coffee?", 30),
            line(Some(ada), "Always.", 30),
        ],
    );
    let seeded = dev_conversation_seed(&harness.context, request.clone())
        .await
        .expect("seed");
    let (conversation, lines) = timeline(&harness, &seeded.conversation_id);
    assert_eq!(conversation.title, "Dev chat");
    let ConversationKind::Direct(details) = &conversation.kind else {
        panic!("a direct chat");
    };
    assert_eq!(details.character.source_id, ada);
    let character = conversation
        .participants
        .iter()
        .find(|participant| participant.source == ParticipantSource::Character(ada))
        .map(|participant| participant.id)
        .expect("character participant");
    assert_eq!(
        lines,
        vec![
            (
                MessageRole::User,
                None,
                "Morning.".to_owned(),
                NOW - 90 * 60_000
            ),
            (
                MessageRole::Assistant,
                Some(character),
                "Morning to you.".to_owned(),
                NOW - 89 * 60_000
            ),
            (
                MessageRole::User,
                None,
                "Coffee?".to_owned(),
                NOW - 30 * 60_000
            ),
            (
                MessageRole::Assistant,
                Some(character),
                "Always.".to_owned(),
                NOW - 30 * 60_000
            ),
        ]
    );
    let replay = dev_conversation_seed(&harness.context, request.clone())
        .await
        .expect("replay");
    assert_eq!(replay, seeded);
    let mut changed = request;
    changed.title = "Another".into();
    let conflict = dev_conversation_seed(&harness.context, changed)
        .await
        .expect_err("same key, different request");
    assert_eq!(conflict.code, ApiErrorCode::Conflict);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_group_seed_belongs_to_the_existing_group_and_maps_each_speaker() {
    let harness = seeded_harness();
    developer_mode(&harness, true).await;
    let database = harness.context.backend().database();
    let ada = harness.character_id;
    let ben = seed_named_character(database, "Ben");
    let group = seed_group(
        database,
        vec![member(ada, 0), member(ben, 1)],
        None,
        |group| {
            group.companion_mode = true;
        },
    );
    let seeded = dev_conversation_seed(
        &harness.context,
        dto::DevConversationSeedRequest {
            target: dto::DevSeedTarget::Group {
                group_id: group.to_string(),
            },
            title: "Dev crew".into(),
            lines: vec![
                line(None, "Hello both.", 20),
                line(Some(ada), "Hi.", 19),
                line(Some(ben), "Hey.", 18),
                line(Some(ada), "Ben, say more.", 10),
            ],
            relationship: None,
            client_operation_id: "dev-seed-group".into(),
        },
    )
    .await
    .expect("seed");
    let (conversation, lines) = timeline(&harness, &seeded.conversation_id);
    let ConversationKind::Group(details) = &conversation.kind else {
        panic!("a group chat");
    };
    assert_eq!(details.group.source_id, group);
    assert_eq!(conversation.title, "Dev crew");
    let participant = |character: CharacterId| {
        conversation
            .participants
            .iter()
            .find(|participant| participant.source == ParticipantSource::Character(character))
            .map(|participant| participant.id)
            .expect("member participant")
    };
    assert_eq!(
        lines
            .iter()
            .map(|(role, author, text, _)| (*role, *author, text.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (MessageRole::User, None, "Hello both."),
            (MessageRole::Assistant, Some(participant(ada)), "Hi."),
            (MessageRole::Assistant, Some(participant(ben)), "Hey."),
            (
                MessageRole::Assistant,
                Some(participant(ada)),
                "Ben, say more."
            ),
        ]
    );
    assert!(
        crate::generation::live_sources::live_companion_mode(database, &conversation)
            .expect("companion mode"),
        "the chat follows the group's companion toggle"
    );
    let groups = lettuce_characters::GroupRepository::search(
        database,
        lettuce_characters::GroupSearch {
            text: String::new(),
            include_archived: true,
            sort: lettuce_characters::LibrarySort::Name,
        },
        lettuce_types::PageRequest::default(),
    )
    .expect("groups");
    assert_eq!(groups.items.len(), 1, "no new group is created");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_relationship_is_applied_to_a_companion_and_refused_otherwise() {
    let harness = seeded_harness();
    developer_mode(&harness, true).await;
    let database = harness.context.backend().database();
    let companion = create_character(
        database,
        "Mira",
        CharacterDefaults {
            interaction_mode: lettuce_characters::InteractionMode::Companion,
            companion_soul: Some(lettuce_companions::CompanionSoulConfig::default()),
            ..Default::default()
        },
    );
    let relationship = dto::DevSeedRelationship {
        closeness: 0.45,
        trust: 0.6,
        affection: 0.35,
        tension: 0.15,
        stability: 0.7,
    };
    let mut request = direct(
        companion,
        vec![line(None, "Hey you.", 5), line(Some(companion), "Hey.", 4)],
    );
    request.relationship = Some(relationship);
    request.client_operation_id = "dev-seed-companion".into();
    let seeded = dev_conversation_seed(&harness.context, request)
        .await
        .expect("seed");
    let state = companion_state_get(
        &harness.context,
        dto::CompanionStateGetRequest {
            conversation_id: seeded.conversation_id,
        },
    )
    .await
    .expect("state")
    .expect("companion state");
    assert_eq!(
        [
            state.relationship.closeness,
            state.relationship.trust,
            state.relationship.affection,
            state.relationship.tension,
            state.relationship.stability,
        ],
        [0.45, 0.6, 0.35, 0.15, 0.7]
    );
    assert_eq!(state.relationship.interaction_count, 0);

    let mut roleplay = direct(harness.character_id, vec![line(None, "Hi", 1)]);
    roleplay.relationship = Some(relationship);
    roleplay.client_operation_id = "dev-seed-roleplay".into();
    let error = dev_conversation_seed(&harness.context, roleplay)
        .await
        .expect_err("not a companion");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
    assert_eq!(field(&error), Some("relationship"));

    let mut out_of_range = direct(companion, vec![line(None, "Hi", 1)]);
    out_of_range.relationship = Some(dto::DevSeedRelationship {
        tension: -0.1,
        ..relationship
    });
    out_of_range.client_operation_id = "dev-seed-range".into();
    let error = dev_conversation_seed(&harness.context, out_of_range)
        .await
        .expect_err("tension below zero");
    assert_eq!(field(&error), Some("relationship"));
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_lines_are_refused_with_their_field() {
    let harness = seeded_harness();
    developer_mode(&harness, true).await;
    let database = harness.context.backend().database();
    let ada = harness.character_id;
    let stranger = seed_named_character(database, "Stranger");
    let cases = [
        (
            direct(ada, vec![line(Some(stranger), "Who am I?", 1)]),
            "lines.speaker_character_id",
        ),
        (direct(ada, vec![line(None, "  ", 1)]), "lines.text"),
        (
            direct(ada, vec![line(None, "Later", 1), line(None, "Earlier", 5)]),
            "lines.minutes_ago",
        ),
        (direct(ada, Vec::new()), "lines"),
        (
            dto::DevConversationSeedRequest {
                title: " ".into(),
                ..direct(ada, vec![line(None, "Hi", 1)])
            },
            "title",
        ),
    ];
    for (request, expected) in cases {
        let error = dev_conversation_seed(&harness.context, request)
            .await
            .expect_err("invalid request");
        assert_eq!(error.code, ApiErrorCode::InvalidInput, "{expected}");
        assert_eq!(field(&error), Some(expected));
    }
    let ben = seed_named_character(database, "Ben");
    let group = seed_group(database, vec![member(ada, 0), member(ben, 1)], None, |_| {});
    let error = dev_conversation_seed(
        &harness.context,
        dto::DevConversationSeedRequest {
            target: dto::DevSeedTarget::Group {
                group_id: group.to_string(),
            },
            title: "Dev crew".into(),
            lines: vec![line(Some(stranger), "Not a member", 1)],
            relationship: None,
            client_operation_id: "dev-seed-stranger".into(),
        },
    )
    .await
    .expect_err("speaker outside the group");
    assert_eq!(field(&error), Some("lines.speaker_character_id"));
}
