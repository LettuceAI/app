use std::sync::Arc;

use lettuce_contracts::{self as dto, ApiErrorCode};

use crate::api::*;
use crate::api::tests::{Harness, RecordingStream, Reply, harness, send};
use super::branches_tests::copy_second_character;
use super::turns_tests::{open, replied_chat, run_generation};

async fn memories(harness: &Harness, chat: &str) -> dto::MemoryView {
    memory_get(&harness.context, dto::ConversationRequest { conversation_id: chat.into() })
        .await.expect("memories")
}

#[tokio::test]
async fn branch_targets_keep_memory_at_the_cutoff_and_isolate_later_changes() {
    let harness = harness(Reply::Text("The earlier reply"));
    let other = copy_second_character(&harness);
    let (chat, cutoff) = replied_chat(&harness, "memory-copy").await;
    let added = memory_add(&harness.context, dto::MemoryAddRequest {
        conversation_id: chat.clone(), text: "Before the branch".into(), category: None,
        observed_at: None, expected_revision: 1, client_operation_id: "before-cut".into(),
    }).await.expect("add before cut");
    let original_id = added.memory_id.expect("memory id");
    let current = open(&harness, &chat).await;
    conversation_add_user_message(&harness.context, dto::ConversationAddUserMessageRequest {
        conversation_id: chat.clone(), text: "Later event".into(), expected_revision: current.revision,
        client_operation_id: "later-event".into(),
    }).await.expect("later message");
    memory_update(&harness.context, dto::MemoryUpdateRequest {
        conversation_id: chat.clone(), memory_id: original_id.clone(), text: Some("Changed after the branch".into()),
        category: dto::MemoryCategoryChange::Keep, observed_at: dto::MemoryObservedAtChange::Keep,
        expected_revision: 2, client_operation_id: "later-memory".into(),
    }).await.expect("later memory");
    let source = open(&harness, &chat).await;
    for (index, mode) in [None, Some(dto::GroupChatMode::Conversation), Some(dto::GroupChatMode::Roleplay)].into_iter().enumerate() {
        let copied = if let Some(mode) = mode {
            conversation_branch_direct_to_group(&harness.context, dto::ConversationGroupCopyRequest {
                conversation_id: chat.clone(), message_id: cutoff.clone(),
                character_ids: vec![harness.character_id.to_string(), other.to_string()],
                chat_mode: Some(mode), title: Some("My branch".into()), expected_revision: Some(source.revision),
                client_operation_id: format!("group-{mode:?}"),
            }).await.expect("group copy")
        } else {
            conversation_branch_direct_to_character(&harness.context, dto::ConversationCharacterCopyRequest {
                conversation_id: chat.clone(), message_id: cutoff.clone(), character_id: other.to_string(),
                expected_revision: Some(source.revision), client_operation_id: "character-copy".into(),
            }).await.expect("character copy")
        };
        let target = open(&harness, &copied.conversation_id).await;
        assert_eq!(target.origin_conversation_id.as_deref(), Some(chat.as_str()));
        assert_eq!(target.origin_message_id.as_deref(), Some(cutoff.as_str()));
        assert_eq!(target.chat_mode, mode);
        assert_eq!(target.messages.items.len(), 2, "later messages must not be copied");
        let memory = memories(&harness, &target.id).await;
        assert_eq!(memory.items.len(), 1);
        assert_eq!(memory.items[0].text, "Before the branch");
        assert_ne!(memory.items[0].id, original_id);
        assert_eq!(memory.status.total_conversation_messages, 2);
        memory_delete(&harness.context, dto::MemoryDeleteRequest {
            conversation_id: target.id, memory_id: memory.items[0].id.clone(),
            expected_revision: memory.revision, client_operation_id: format!("delete-{index}"),
        }).await.expect("delete copied memory");
        assert_eq!(memories(&harness, &chat).await.items[0].text, "Changed after the branch");
    }
}

#[tokio::test]
async fn branch_targets_reject_stale_sources_and_replay_completed_operations() {
    let harness = harness(Reply::Text("reply"));
    let other = copy_second_character(&harness);
    let (chat, cutoff) = replied_chat(&harness, "revision-copy").await;
    let current = open(&harness, &chat).await;
    let mut request = dto::ConversationGroupCopyRequest {
        conversation_id: chat.clone(), message_id: cutoff, character_ids: vec![harness.character_id.to_string(), other.to_string()],
        chat_mode: Some(dto::GroupChatMode::Conversation), title: None,
        expected_revision: Some(current.revision - 1), client_operation_id: "stale-group-copy".into(),
    };
    assert_eq!(conversation_branch_direct_to_group(&harness.context, request.clone()).await.expect_err("stale").code, ApiErrorCode::Conflict);
    request.expected_revision = Some(current.revision);
    let copy = conversation_branch_direct_to_group(&harness.context, request.clone()).await.expect("copy");
    conversation_add_user_message(&harness.context, dto::ConversationAddUserMessageRequest {
        conversation_id: chat, text: "The source changed".into(), expected_revision: current.revision,
        client_operation_id: "change-after-copy".into(),
    }).await.expect("change source");
    assert_eq!(conversation_branch_direct_to_group(&harness.context, request).await.expect("replay"), copy);
}

#[tokio::test]
async fn branch_targets_preserve_historical_speakers_through_group_and_character_copies() {
    let harness = harness(Reply::Text("A recorded reply"));
    let other = copy_second_character(&harness);
    let outsider = copy_second_character(&harness);
    let (chat, cutoff) = replied_chat(&harness, "speaker-copy").await;
    let current = open(&harness, &chat).await;
    let group = conversation_branch_direct_to_group(&harness.context, dto::ConversationGroupCopyRequest {
        conversation_id: chat.clone(), message_id: cutoff,
        character_ids: vec![harness.character_id.to_string(), other.to_string()],
        chat_mode: Some(dto::GroupChatMode::Conversation), title: None,
        expected_revision: Some(current.revision), client_operation_id: "speaker-group".into(),
    }).await.expect("group");
    let group_view = open(&harness, &group.conversation_id).await;
    let other_participant = group_view.participants.iter().find(|participant| participant.character_id.as_deref() == Some(other.to_string().as_str())).expect("other participant");
    let mut next = super::turns_tests::continue_request(&harness, &group.conversation_id, "other-reply");
    next.forced_speaker_participant_id = Some(other_participant.id.clone());
    conversation_continue(&harness.context, next, Arc::new(RecordingStream::default())).await.expect("continue group");
    run_generation(&harness).await;
    let group_view = open(&harness, &group.conversation_id).await;
    let cutoff = group_view.messages.items.last().expect("last reply").id.clone();
    let copied = conversation_branch_to_character_from_message(&harness.context, dto::ConversationCharacterCopyRequest {
        conversation_id: group_view.id.clone(), message_id: cutoff, character_id: outsider.to_string(),
        expected_revision: Some(group_view.revision), client_operation_id: "outsider-copy".into(),
    }).await.expect("copy to a character outside the group");
    let target = open(&harness, &copied.conversation_id).await;
    assert_eq!(target.participants.len(), 2);
    let speakers: Vec<_> = target.messages.items.iter().flat_map(|message| message.parts.iter()).filter_map(|part| match part {
        dto::MessagePartView::HistoricalSpeaker { character_id, name } => Some((character_id.as_str(), name.as_str())),
        _ => None,
    }).collect();
    assert!(speakers.contains(&(harness.character_id.to_string().as_str(), "Ada")));
    assert!(speakers.contains(&(other.to_string().as_str(), "Other")));
    send(&harness, &target.id, "after-copy", "Continue our story", Arc::new(RecordingStream::default())).await.expect("send to destination");
    run_generation(&harness).await;
    {
    let requests = harness.provider.requests.lock().expect("requests");
    let request = requests.last().expect("destination request");
    let history = request.context.messages.iter().flat_map(|message| &message.parts).filter_map(|part| match part {
        lettuce_conversations::ProviderContextPart::Text { text } => Some(text.as_str()),
        _ => None,
    }).collect::<Vec<_>>().join("\n");
    assert!(history.contains("Ada: "));
    assert!(history.contains("Other: "));
    }
    let completed = open(&harness, &target.id).await;
    assert!(completed.can_send);
    assert_eq!(completed.messages.items.last().expect("new reply").parts.iter().filter(|part| matches!(part, dto::MessagePartView::HistoricalSpeaker { .. })).count(), 0);
}

#[tokio::test]
async fn branch_targets_copy_only_local_companion_memories_into_an_independent_space() {
    let harness = harness(Reply::Text("reply"));
    let database = harness.context.backend().database();
    let mut chats = Vec::new();
    let mut characters = Vec::new();
    for name in ["Source companion", "Target companion"] {
        let character = crate::api::tests::create_character(database, name, lettuce_characters::CharacterDefaults {
            interaction_mode: lettuce_characters::InteractionMode::Companion,
            companion_soul: Some(lettuce_companions::CompanionSoulConfig::default()),
            memory_policy: lettuce_characters::MemoryPolicy::Dynamic,
            ..Default::default()
        });
        characters.push(character);
        let count = if name == "Source companion" { 2 } else { 1 };
        for index in 0..count {
            let chat = conversation_launch_direct(&harness.context, dto::LaunchDirectRequest {
                character_id: character.to_string(), title: None, scene_id: None, starter_id: None,
                client_operation_id: format!("companion-{}-{index}", characters.len()),
            }).await.expect("companion chat").conversation_id;
            let current = open(&harness, &chat).await;
            conversation_add_user_message(&harness.context, dto::ConversationAddUserMessageRequest {
                conversation_id: chat.clone(), text: "An anchor".into(), expected_revision: current.revision,
                client_operation_id: format!("anchor-{}-{index}", characters.len()),
            }).await.expect("anchor");
            let current = memories(&harness, &chat).await;
            memory_add(&harness.context, dto::MemoryAddRequest {
                conversation_id: chat.clone(), text: format!("Memory from {name} chat {index}"),
                category: None, observed_at: None, expected_revision: current.revision,
                client_operation_id: format!("memory-{}-{index}", characters.len()),
            }).await.expect("shared pool memory");
            chats.push(chat);
        }
    }
    let source = open(&harness, &chats[0]).await;
    let copy = conversation_branch_direct_to_character(&harness.context, dto::ConversationCharacterCopyRequest {
        conversation_id: source.id.clone(), message_id: source.messages.items.last().expect("anchor").id.clone(),
        character_id: characters[1].to_string(), expected_revision: Some(source.revision),
        client_operation_id: "isolated-companion-copy".into(),
    }).await.expect("companion branch");
    let memory = memories(&harness, &copy.conversation_id).await;
    assert_eq!(memory.items.len(), 1);
    assert_eq!(memory.items[0].text, "Memory from Source companion chat 0");
    assert_eq!(memories(&harness, &chats[0]).await.items.len(), 2);
    assert_eq!(memories(&harness, &chats[2]).await.items.len(), 1);
    let target = open(&harness, &copy.conversation_id).await;
    let owner = lettuce_companions::CompanionStateOwner {
        conversation_id: target.id.parse().expect("target conversation"), character_id: characters[1],
        persona_id: None,
    };
    assert!(lettuce_companions::CompanionStateRepository::get(database, owner).expect("target state").is_some());
}

#[tokio::test]
async fn a_copy_to_a_time_aware_character_starts_time_aware_like_a_launch() {
    use lettuce_characters::CharacterRepository;
    let harness = harness(Reply::Text("The earlier reply"));
    let other = copy_second_character(&harness);
    let database = harness.context.backend().database();
    let character = CharacterRepository::get(database, other)
        .expect("character")
        .expect("exists")
        .character;
    let mut defaults = character.defaults.clone();
    defaults.time_awareness = true;
    CharacterRepository::update_defaults(
        database,
        other,
        character.revision,
        defaults,
        harness.context.now(),
    )
    .expect("make the character time aware");
    let (chat, cutoff) = replied_chat(&harness, "time-copy").await;
    let current = open(&harness, &chat).await;
    let copied = conversation_branch_direct_to_character(
        &harness.context,
        dto::ConversationCharacterCopyRequest {
            conversation_id: chat.clone(),
            message_id: cutoff,
            character_id: other.to_string(),
            expected_revision: Some(current.revision),
            client_operation_id: "time-copy-target".into(),
        },
    )
    .await
    .expect("copy to the time aware character");
    let target = lettuce_conversations::ConversationReader::get(
        database,
        copied.conversation_id.parse().expect("target id"),
    )
    .expect("target")
    .conversation;
    let clock = crate::companion::companion_clock::conversation_clock_context(database, &target)
        .expect("clock context");
    assert!(!clock.companion);
    assert!(clock.time_awareness_enabled());
    let source = lettuce_conversations::ConversationReader::get(
        database,
        chat.parse().expect("source id"),
    )
    .expect("source")
    .conversation;
    assert!(
        !crate::companion::companion_clock::conversation_clock_context(database, &source)
            .expect("source clock")
            .time_awareness_enabled()
    );
}
