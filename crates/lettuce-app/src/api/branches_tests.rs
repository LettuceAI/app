use lettuce_contracts::{self as dto, ApiErrorCode};

use super::tests::{Reply, harness, launch};
use super::*;

#[tokio::test]
async fn branches_list_fork_rename_select_and_replay() {
    let harness = harness(Reply::Text("reply"));
    let conversation_id = launch(&harness, "branches-launch").await;
    let list_request = dto::ConversationRequest {
        conversation_id: conversation_id.clone(),
    };
    let initial = conversation_branches(&harness.context, list_request.clone())
        .await
        .expect("root");
    assert_eq!(initial.branches.len(), 1);
    let root = initial.branches[0].id.clone();
    let message = conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: conversation_id.clone(),
            text: "one".into(),
            expected_revision: initial.revision,
            client_operation_id: "branches-message".into(),
        },
    )
    .await
    .expect("message");
    let request = dto::ConversationBranchForkRequest {
        conversation_id: conversation_id.clone(),
        message_id: message.message.id.clone(),
        expected_revision: message.revision,
        client_operation_id: "branches-fork".into(),
    };
    let fork = conversation_branch_fork(&harness.context, request.clone())
        .await
        .expect("fork");
    assert_eq!(
        fork,
        conversation_branch_fork(&harness.context, request.clone())
            .await
            .expect("replay")
    );
    let mut changed = request.clone();
    changed.message_id = root.clone();
    assert_eq!(
        conversation_branch_fork(&harness.context, changed)
            .await
            .expect_err("digest conflict")
            .code,
        ApiErrorCode::Conflict
    );
    let list = conversation_branches(&harness.context, list_request.clone())
        .await
        .expect("list");
    assert_eq!(list.branches.len(), 2);
    assert_eq!(list.branches[0].id, root);
    assert_eq!(
        list.branches[1].parent_branch_id.as_deref(),
        Some(root.as_str())
    );
    assert!(list.branches[1].active);
    assert_eq!(list.branches[1].message_count, 1);
    let rename = dto::ConversationBranchRenameRequest {
        branch_id: fork.branch_id.clone(),
        label: "  fork label  ".into(),
        expected_revision: list.revision,
        client_operation_id: "branches-rename".into(),
    };
    let renamed = conversation_branch_rename(&harness.context, rename.clone())
        .await
        .expect("rename");
    assert_eq!(
        renamed,
        conversation_branch_rename(&harness.context, rename)
            .await
            .expect("rename replay")
    );
    let select = dto::ConversationBranchMutationRequest {
        branch_id: root.clone(),
        expected_revision: renamed.revision,
        client_operation_id: "branches-select".into(),
    };
    let selected = conversation_branch_select(&harness.context, select.clone())
        .await
        .expect("select root");
    assert_eq!(
        selected,
        conversation_branch_select(&harness.context, select)
            .await
            .expect("select replay")
    );
    assert_eq!(
        fork,
        conversation_branch_fork(&harness.context, request)
            .await
            .expect("fork replay after selection")
    );
    let final_list = conversation_branches(&harness.context, list_request)
        .await
        .expect("final list");
    assert!(final_list.branches[0].active);
    assert_eq!(final_list.branches[1].label, "fork label");
    let delete = dto::ConversationBranchMutationRequest {
        branch_id: fork.branch_id.clone(),
        expected_revision: final_list.revision,
        client_operation_id: "branches-delete".into(),
    };
    let deleted = conversation_branch_delete(&harness.context, delete.clone())
        .await
        .expect("delete inactive branch");
    assert_eq!(
        conversation_branch_delete(&harness.context, delete.clone())
            .await
            .expect("delete replay"),
        deleted
    );
    let list = conversation_branches(
        &harness.context,
        dto::ConversationRequest {
            conversation_id: conversation_id.clone(),
        },
    )
    .await
    .expect("after delete");
    assert_eq!(list.branches.len(), 1);
    let mut changed = delete;
    changed.branch_id = root;
    assert_eq!(
        conversation_branch_delete(&harness.context, changed)
            .await
            .expect_err("changed delete digest")
            .code,
        ApiErrorCode::Conflict
    );
}

#[tokio::test]
async fn duplicate_copies_messages_and_replays_after_source_deletion() {
    let harness = harness(Reply::Text("reply"));
    let conversation_id = launch(&harness, "duplicate-launch").await;
    let root = conversation_branches(
        &harness.context,
        dto::ConversationRequest {
            conversation_id: conversation_id.clone(),
        },
    )
    .await
    .expect("source root");
    let message = conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: conversation_id.clone(),
            text: "Copied message".into(),
            expected_revision: root.revision,
            client_operation_id: "duplicate-message".into(),
        },
    )
    .await
    .expect("source message");
    let request = dto::ConversationDuplicateRequest {
        conversation_id: conversation_id.clone(),
        with_messages: true,
        title: None,
        client_operation_id: "duplicate-api".into(),
    };
    let copied = conversation_duplicate(&harness.context, request.clone())
        .await
        .expect("duplicate");
    assert_ne!(copied.conversation_id, conversation_id);
    assert_eq!(copied.revision, 1);
    let target = lettuce_conversations::ConversationReader::get(
        harness.context.backend().database(),
        copied.conversation_id.parse().expect("target id"),
    )
    .expect("target");
    assert_eq!(target.conversation.origin_conversation_id, None);
    assert_eq!(target.conversation.origin_message_id, None);
    let page = lettuce_conversations::ConversationReader::timeline_page(
        harness.context.backend().database(),
        target.conversation.id,
        target.conversation.active_branch_id,
        &lettuce_types::PageRequest {
            cursor: None,
            limit: lettuce_types::PageLimit::new(20),
        },
    )
    .expect("target messages");
    assert_eq!(page.items.len(), 1);
    assert_ne!(page.items[0].message.id.to_string(), message.message.id);
    harness
        .context
        .backend()
        .database()
        .purge_conversation(
            conversation_id.parse().expect("source id"),
            harness.context.now(),
        )
        .expect("delete source");
    assert_eq!(
        conversation_duplicate(&harness.context, request.clone())
            .await
            .expect("replay"),
        copied
    );
    let mut changed = request;
    changed.with_messages = false;
    assert_eq!(
        conversation_duplicate(&harness.context, changed)
            .await
            .expect_err("changed digest")
            .code,
        ApiErrorCode::Conflict
    );
}

#[tokio::test]
async fn direct_character_copy_keeps_persona_lineage_and_replays_after_source_purge() {
    let harness = harness(Reply::Text("reply"));
    let source_id = launch(&harness, "direct-copy-source").await;
    let initial = conversation_branches(
        &harness.context,
        dto::ConversationRequest {
            conversation_id: source_id.clone(),
        },
    )
    .await
    .expect("source root");
    let message = conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: source_id.clone(),
            text: "Copied text".into(),
            expected_revision: initial.revision,
            client_operation_id: "direct-copy-message".into(),
        },
    )
    .await
    .expect("source message");
    let request = dto::ConversationCharacterCopyRequest {
        conversation_id: source_id.clone(),
        message_id: message.message.id.clone(),
        character_id: harness.character_id.to_string(),
        client_operation_id: "direct-copy".into(),
    };
    let copied = conversation_branch_direct_to_character(&harness.context, request.clone())
        .await
        .expect("copy");
    let target = lettuce_conversations::ConversationReader::get(
        harness.context.backend().database(),
        copied.conversation_id.parse().expect("target id"),
    )
    .expect("target");
    assert_eq!(
        target
            .conversation
            .origin_conversation_id
            .map(|id| id.to_string()),
        Some(source_id.clone())
    );
    assert_eq!(
        target
            .conversation
            .origin_message_id
            .map(|id| id.to_string()),
        Some(message.message.id)
    );
    assert_eq!(
        target
            .conversation
            .current_settings
            .as_ref()
            .and_then(|settings| settings.background),
        Some(lettuce_conversations::ConversationBackground::Hidden)
    );
    harness
        .context
        .backend()
        .database()
        .purge_conversation(source_id.parse().expect("source id"), harness.context.now())
        .expect("purge source");
    assert_eq!(
        conversation_branch_direct_to_character(&harness.context, request.clone())
            .await
            .expect("replay"),
        copied
    );
    let mut changed = request;
    changed.character_id = lettuce_types::CharacterId::new().to_string();
    assert_eq!(
        conversation_branch_direct_to_character(&harness.context, changed)
            .await
            .expect_err("changed digest")
            .code,
        ApiErrorCode::Conflict
    );
}

pub(super) fn copy_second_character(harness: &super::tests::Harness) -> lettuce_types::CharacterId {
    use lettuce_characters::CharacterRepository;
    let database = harness.context.backend().database();
    let mut details = CharacterRepository::get(database, harness.character_id)
        .expect("owner")
        .expect("owner character");
    details.character.id = lettuce_types::CharacterId::new();
    details.character.profile.name = "Other".into();
    let id = details.character.id;
    CharacterRepository::create(
        database,
        lettuce_characters::CreateCharacterPlan {
            character: details.character,
            scenes: Vec::new(),
            variants: Vec::new(),
            starters: Vec::new(),
        },
    )
    .expect("second character");
    id
}

#[tokio::test]
async fn direct_group_copy_creates_profile_and_chat_and_replays() {
    let harness = harness(Reply::Text("reply"));
    let other = copy_second_character(&harness);
    let database = harness.context.backend().database();
    let owner = lettuce_characters::CharacterRepository::get(database, harness.character_id)
        .expect("owner")
        .expect("owner character");
    let mut scene = lettuce_characters::Scene::new(
        lettuce_types::SceneId::new(),
        lettuce_characters::SceneOwner::Character(harness.character_id),
        0,
        lettuce_characters::SceneDocumentV1 {
            format_version: 1,
            parts: vec![lettuce_characters::ScenePart::Text {
                text: "Owner scene".into(),
            }],
        },
        harness.context.now(),
    )
    .expect("owner scene");
    scene.direction = Some("Owner direction".into());
    lettuce_characters::SceneRepository::add_scene(
        database,
        harness.character_id,
        owner.character.revision,
        scene.clone(),
        harness.context.now(),
    )
    .expect("add owner scene");
    let source_id = conversation_launch_direct(
        &harness.context,
        dto::LaunchDirectRequest {
            character_id: harness.character_id.to_string(),
            title: None,
            scene_id: Some(scene.id.to_string()),
            starter_id: None,
            client_operation_id: "group-copy-source".into(),
        },
    )
    .await
    .expect("source scene launch")
    .conversation_id;
    let initial = conversation_branches(
        &harness.context,
        dto::ConversationRequest {
            conversation_id: source_id.clone(),
        },
    )
    .await
    .expect("source root");
    let message = conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: source_id.clone(),
            text: "Group copy".into(),
            expected_revision: initial.revision,
            client_operation_id: "group-copy-message".into(),
        },
    )
    .await
    .expect("source message");
    let request = dto::ConversationGroupCopyRequest {
        conversation_id: source_id.clone(),
        message_id: message.message.id.clone(),
        character_ids: vec![harness.character_id.to_string(), other.to_string()],
        client_operation_id: "group-copy".into(),
    };
    let copied = conversation_branch_direct_to_group(&harness.context, request.clone())
        .await
        .expect("copy group");
    let target = lettuce_conversations::ConversationReader::get(
        harness.context.backend().database(),
        copied.conversation_id.parse().expect("target id"),
    )
    .expect("target");
    assert_eq!(target.conversation.origin_conversation_id, None);
    let lettuce_conversations::ConversationKind::Group(details) = &target.conversation.kind else {
        panic!("group target");
    };
    let group = lettuce_characters::GroupRepository::get(
        harness.context.backend().database(),
        details.group.source_id,
    )
    .expect("group")
    .expect("new group");
    assert_eq!(group.group.members.len(), 2);
    assert_eq!(
        group.group.chat_mode,
        lettuce_characters::ChatMode::Roleplay
    );
    assert!(group.group.name.ends_with(" Branch"));
    let timeline = lettuce_conversations::ConversationReader::timeline_page(
        harness.context.backend().database(),
        target.conversation.id,
        target.conversation.active_branch_id,
        &lettuce_types::PageRequest {
            cursor: None,
            limit: lettuce_types::PageLimit::new(20),
        },
    )
    .expect("timeline");
    assert_eq!(timeline.items.len(), 2);
    assert_eq!(
        group
            .starting_scene
            .as_ref()
            .expect("new group scene")
            .scene
            .direction,
        scene.direction
    );
    assert_ne!(
        group
            .starting_scene
            .as_ref()
            .expect("new group scene")
            .scene
            .id,
        scene.id
    );
    assert!(
        timeline
            .items
            .iter()
            .any(|item| item.message.role == lettuce_conversations::MessageRole::Scene)
    );
    assert_ne!(timeline.items[0].message.id.to_string(), message.message.id);
    assert_eq!(
        conversation_branch_direct_to_group(&harness.context, request.clone())
            .await
            .expect("replay"),
        copied
    );
    let mut changed = request;
    changed.character_ids.reverse();
    assert_eq!(
        conversation_branch_direct_to_group(&harness.context, changed)
            .await
            .expect_err("changed digest")
            .code,
        ApiErrorCode::Conflict
    );
}

#[tokio::test]
async fn new_group_copy_rolls_back_profile_messages_and_receipt_together() {
    use lettuce_characters::{GroupDetails, GroupMember, GroupProfile, GroupRepository};
    use lettuce_conversations::ConversationReader;
    let harness = harness(Reply::Text("reply"));
    let other = copy_second_character(&harness);
    let source_id = launch(&harness, "copy-fault-source").await;
    let initial = conversation_branches(
        &harness.context,
        dto::ConversationRequest {
            conversation_id: source_id.clone(),
        },
    )
    .await
    .expect("source root");
    let message = conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: source_id.clone(),
            text: "Atomic copy".into(),
            expected_revision: initial.revision,
            client_operation_id: "copy-fault-user".into(),
        },
    )
    .await
    .expect("source user");
    let database = harness.context.backend().database();
    let source =
        ConversationReader::get(database, source_id.parse().expect("source id")).expect("source");
    let now = harness.context.now();
    let group_id = lettuce_types::GroupId::new();
    let mut group = GroupProfile::new(
        group_id,
        "Atomic Branch".into(),
        vec![
            GroupMember {
                character_id: harness.character_id,
                ordinal: 0,
                muted: false,
                model_profile_override: None,
            },
            GroupMember {
                character_id: other,
                ordinal: 1,
                muted: false,
                model_profile_override: None,
            },
        ],
        now,
    )
    .expect("group");
    group.chat_mode = lettuce_characters::ChatMode::Roleplay;
    let request = crate::GroupConversationLaunchRequest {
        format_version: crate::GROUP_LAUNCH_REQUEST_FORMAT_V1,
        title: group.name.clone(),
        user: crate::DirectUserParticipant {
            display_name: "User".into(),
            authored_description: None,
        },
        group_id,
        persona: crate::LaunchSelection::Disabled,
        operation_key: lettuce_conversations::IdempotencyKey::new("copy-fault-target-launch")
            .expect("key"),
    };
    let target_id = crate::launch_conversation_id(&request.operation_key);
    let prepare = || lettuce_database::ConversationCopyLaunch {
        launch: crate::ConversationLaunchPlanner::new(database)
            .prepare_new_group(
                &request,
                GroupDetails {
                    group: group.clone(),
                    starting_scene: None,
                },
                now,
            )
            .expect("prepared new group"),
        source_conversation_id: source.conversation.id,
        source_revision: source.conversation.revision,
        source_branch_id: source.conversation.active_branch_id,
        source_group_revision: None,
        source_character_revision: None,
        through_message_id: Some(message.message.id.parse().expect("source message id")),
        kind: lettuce_conversations::SelectedConversationCopyKind::DirectToGroup,
        companion: None,
        initialize_companion: false,
        new_group: Some(lettuce_characters::CreateGroupPlan {
            group: group.clone(),
            starting_scene: None,
        }),
    };
    let draft = prepare();
    let failed: Result<dto::ConversationCopyResult, lettuce_database::ApiOperationError> = database
        .commit_api_operation("copy-fault", "copy-fault", "ab", now, |scope| {
            scope
                .create_conversation_copy(draft, now)
                .map_err(|_| lettuce_database::ApiOperationError::Storage)?;
            Err(lettuce_database::ApiOperationError::Storage)
        });
    assert_eq!(failed, Err(lettuce_database::ApiOperationError::Storage));
    assert!(
        GroupRepository::get(database, group_id)
            .expect("group query")
            .is_none()
    );
    assert_eq!(
        ConversationReader::get(database, target_id),
        Err(lettuce_conversations::ConversationRepositoryError::NotFound)
    );
    assert!(
        database
            .lookup_api_operation("copy-fault", "copy-fault")
            .expect("receipt query")
            .is_none()
    );
    let draft = prepare();
    let copied: dto::ConversationCopyResult = database
        .commit_api_operation("copy-fault", "copy-fault", "ab", now, |scope| {
            let commit = scope
                .create_conversation_copy(draft, now)
                .map_err(|_| lettuce_database::ApiOperationError::Storage)?;
            Ok::<_, lettuce_database::ApiOperationError>(dto::ConversationCopyResult {
                conversation_id: commit.value.conversation.id.to_string(),
                revision: commit.value.conversation.revision.get(),
            })
        })
        .expect("successful retry");
    assert_eq!(copied.conversation_id, target_id.to_string());
    assert!(
        GroupRepository::get(database, group_id)
            .expect("new group query")
            .is_some()
    );
    let replay: dto::ConversationCopyResult = database
        .commit_api_operation(
            "copy-fault",
            "copy-fault",
            "ab",
            now,
            |_| -> Result<dto::ConversationCopyResult, lettuce_database::ApiOperationError> {
                panic!("exact replay must not create another group")
            },
        )
        .expect("exact replay");
    assert_eq!(replay, copied);
    let changed: Result<dto::ConversationCopyResult, lettuce_database::ApiOperationError> =
        database.commit_api_operation("copy-fault", "copy-fault", "ac", now, |_| {
            panic!("changed digest must not stage writes")
        });
    assert_eq!(changed, Err(lettuce_database::ApiOperationError::Conflict));
}
