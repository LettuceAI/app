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

#[tokio::test]
async fn companion_duplicate_copies_state_and_relationship_then_advances_independently() {
    use lettuce_companions::{
        CompanionStateOwner, CompanionStateReplacement, CompanionStateRepository,
    };
    let harness = harness(Reply::Text("reply"));
    let database = harness.context.backend().database();
    let character_id = super::tests::create_character(
        database,
        "Companion",
        lettuce_characters::CharacterDefaults {
            interaction_mode: lettuce_characters::InteractionMode::Companion,
            companion_soul: Some(lettuce_companions::CompanionSoulConfig::default()),
            memory_policy: lettuce_characters::MemoryPolicy::Dynamic,
            ..lettuce_characters::CharacterDefaults::default()
        },
    );
    let source = conversation_launch_direct(
        &harness.context,
        dto::LaunchDirectRequest {
            character_id: character_id.to_string(),
            title: None,
            scene_id: None,
            starter_id: None,
            client_operation_id: "companion-copy-source".into(),
        },
    )
    .await
    .expect("source companion");
    let owner = CompanionStateOwner {
        conversation_id: source.conversation_id.parse().expect("source id"),
        character_id,
        persona_id: None,
    };
    let before = CompanionStateRepository::get(database, owner)
        .expect("source state")
        .expect("source state exists");
    let mut state = before.state;
    state.emotional_state.felt.warmth = 0.73;
    state.relationship_state.closeness = 0.47;
    CompanionStateRepository::replace(
        database,
        owner,
        lettuce_types::OperationRecordId::new(),
        CompanionStateReplacement {
            expected_session_revision: before.session_revision,
            expected_relationship_revision: before.relationship_revision,
            state,
            applied_at: harness.context.now(),
        },
    )
    .expect("advance source before copy");
    let source_state = CompanionStateRepository::get(database, owner)
        .expect("source state")
        .expect("source");
    let source_episode =
        CompanionStateRepository::get_continuity_episode(database, owner.conversation_id)
            .expect("source continuity")
            .expect("episode");
    for with_messages in [false, true] {
        let request = dto::ConversationDuplicateRequest {
            conversation_id: source.conversation_id.clone(),
            title: None,
            with_messages,
            client_operation_id: format!("companion-copy-{with_messages}"),
        };
        let copied = conversation_duplicate(&harness.context, request.clone())
            .await
            .expect("duplicate companion");
        let target_owner = CompanionStateOwner {
            conversation_id: copied.conversation_id.parse().expect("copy id"),
            ..owner
        };
        let target = CompanionStateRepository::get(database, target_owner)
            .expect("target state")
            .expect("target");
        assert_eq!(target.state, source_state.state);
        assert_eq!(target.session_revision, lettuce_types::Revision::INITIAL);
        assert_eq!(
            target.relationship_revision,
            lettuce_types::Revision::INITIAL
        );
        assert_eq!(
            CompanionStateRepository::get_continuity_episode(database, owner.conversation_id)
                .expect("source episode"),
            Some(source_episode.clone())
        );
        let episode = CompanionStateRepository::get_continuity_episode(
            database,
            target_owner.conversation_id,
        )
        .expect("target episode")
        .expect("episode");
        assert_eq!(episode.started_at, source_episode.started_at);
        assert_eq!(episode.ended_at, source_episode.ended_at);
        let mut changed = target.state;
        changed.emotional_state.felt.warmth = 0.11;
        changed.relationship_state.closeness = -0.31;
        CompanionStateRepository::replace(
            database,
            target_owner,
            lettuce_types::OperationRecordId::new(),
            CompanionStateReplacement {
                expected_session_revision: target.session_revision,
                expected_relationship_revision: target.relationship_revision,
                state: changed.clone(),
                applied_at: harness.context.now(),
            },
        )
        .expect("advance copy");
        assert_eq!(
            CompanionStateRepository::get(database, owner)
                .expect("source after copy update")
                .expect("source")
                .state,
            source_state.state
        );
        assert_eq!(
            CompanionStateRepository::get(database, target_owner)
                .expect("copy after update")
                .expect("copy")
                .state,
            changed
        );
        assert_eq!(
            conversation_duplicate(&harness.context, request)
                .await
                .expect("stable duplicate replay"),
            copied
        );
    }
}

#[tokio::test]
async fn companion_duplicate_private_state_validates_for_backup_and_syncs_to_a_peer() {
    use lettuce_companions::{
        CompanionStateOwner, CompanionStateReplacement, CompanionStateRepository,
    };
    let harness = harness(Reply::Text("reply"));
    let database = harness.context.backend().database();
    let character_id = super::tests::create_character(
        database,
        "Companion",
        lettuce_characters::CharacterDefaults {
            interaction_mode: lettuce_characters::InteractionMode::Companion,
            companion_soul: Some(lettuce_companions::CompanionSoulConfig::default()),
            memory_policy: lettuce_characters::MemoryPolicy::Dynamic,
            ..lettuce_characters::CharacterDefaults::default()
        },
    );
    let source = conversation_launch_direct(
        &harness.context,
        dto::LaunchDirectRequest {
            character_id: character_id.to_string(),
            title: None,
            scene_id: None,
            starter_id: None,
            client_operation_id: "companion-sync-source".into(),
        },
    )
    .await
    .expect("source companion");
    let copied = conversation_duplicate(
        &harness.context,
        dto::ConversationDuplicateRequest {
            conversation_id: source.conversation_id.clone(),
            title: None,
            with_messages: false,
            client_operation_id: "companion-sync-copy".into(),
        },
    )
    .await
    .expect("duplicate companion");
    let owner = CompanionStateOwner {
        conversation_id: copied.conversation_id.parse().expect("copy id"),
        character_id,
        persona_id: None,
    };
    let current = CompanionStateRepository::get(database, owner)
        .expect("copy state")
        .expect("copy");
    let mut changed = current.state.clone();
    changed.relationship_state.closeness = -0.42;
    CompanionStateRepository::replace(
        database,
        owner,
        lettuce_types::OperationRecordId::new(),
        CompanionStateReplacement {
            expected_session_revision: current.session_revision,
            expected_relationship_revision: current.relationship_revision,
            state: changed.clone(),
            applied_at: harness.context.now(),
        },
    )
    .expect("advance copy");
    let mut graph = lettuce_transfer::ProviderBackupSource::read_provider_backup_graph(database)
        .expect("graph");
    lettuce_transfer::canonicalize_and_validate(&mut graph)
        .expect("graph with private relationship validates");
    let session = graph
        .companion_state
        .sessions
        .iter()
        .find(|session| session.owner == owner)
        .expect("copy session in backup");
    assert!(session.private_relationships.is_some());
    let source_session = graph
        .companion_state
        .sessions
        .iter()
        .find(|session| session.owner.conversation_id.to_string() == source.conversation_id)
        .expect("source session in backup");
    assert!(source_session.private_relationships.is_none());
    let peer =
        crate::AppBackend::open_in_memory(lettuce_types::TimestampMillis::new(1)).expect("peer");
    crate::launch::tests::sync_prompts(database, peer.database(), 1_000);
    let on_peer = CompanionStateRepository::get(peer.database(), owner)
        .expect("peer state")
        .expect("peer copy");
    assert_eq!(on_peer.state, changed);
    let source_owner = CompanionStateOwner {
        conversation_id: source.conversation_id.parse().expect("source id"),
        ..owner
    };
    assert_ne!(
        CompanionStateRepository::get(peer.database(), source_owner)
            .expect("peer source")
            .expect("source")
            .state
            .relationship_state,
        changed.relationship_state
    );
}

#[tokio::test]
async fn companion_duplicate_clones_conversation_soul_only_when_growth_is_not_shared() {
    use lettuce_companions::{SoulFact, SoulFactKind, SoulFactPolicy, SoulOwner, SoulRepository};
    for shared in [false, true] {
        let memory_id = lettuce_types::MemoryId::new();
        let harness = harness(Reply::Text("reply"));
        let database = harness.context.backend().database();
        let fact = SoulFact {
            id: "authored-fact".into(),
            category: lettuce_companions::SoulCategory::Traits,
            value: "Dry humor".into(),
            kind: SoulFactKind::Authored,
            policy: SoulFactPolicy::Current,
            slot: "traits".into(),
            confidence: 1.0,
            evidence_count: 1,
            weight: 1.0,
            valid_from: lettuce_types::TimestampMillis::new(1),
            valid_until: None,
            locked: false,
            source_memory_ids: vec![memory_id.to_string(), "pool-memory".into()],
            created_at: lettuce_types::TimestampMillis::new(1),
            supersedes: vec!["older-fact".into()],
            superseded_by: None,
            superseded_at: None,
        };
        let character_id = super::tests::create_character(
            database,
            "Companion",
            lettuce_characters::CharacterDefaults {
                interaction_mode: lettuce_characters::InteractionMode::Companion,
                companion_soul: Some(lettuce_companions::CompanionSoulConfig {
                    authored_facts: vec![fact],
                    share_soul_growth_across_chats: shared,
                    share_memory_across_chats: false,
                    ..lettuce_companions::CompanionSoulConfig::default()
                }),
                memory_policy: lettuce_characters::MemoryPolicy::Dynamic,
                ..lettuce_characters::CharacterDefaults::default()
            },
        );
        let source = conversation_launch_direct(
            &harness.context,
            dto::LaunchDirectRequest {
                character_id: character_id.to_string(),
                title: None,
                scene_id: None,
                starter_id: None,
                client_operation_id: format!("soul-copy-source-{shared}"),
            },
        )
        .await
        .expect("source companion");
        let copied = conversation_duplicate(
            &harness.context,
            dto::ConversationDuplicateRequest {
                conversation_id: source.conversation_id.clone(),
                title: None,
                with_messages: false,
                client_operation_id: format!("soul-copy-{shared}"),
            },
        )
        .await
        .expect("duplicate companion");
        let owner_of = |id: &str| SoulOwner::Conversation {
            character_id,
            conversation_id: id.parse().expect("conversation id"),
        };
        let target =
            SoulRepository::get(database, owner_of(&copied.conversation_id)).expect("target soul");
        if shared {
            assert!(target.is_none());
            continue;
        }
        let source_soul = SoulRepository::get(database, owner_of(&source.conversation_id))
            .expect("source soul")
            .expect("source has own soul");
        let target = target.expect("target has own soul");
        assert_eq!(target.facts.len(), source_soul.facts.len());
        for (copy, original) in target.facts.iter().zip(&source_soul.facts) {
            assert_ne!(copy.id, original.id);
            assert_eq!(copy.value, original.value);
            assert_eq!(copy.source_memory_ids, original.source_memory_ids);
            assert_eq!(copy.source_memory_ids[0], memory_id.to_string());
            assert_eq!(copy.supersedes.len(), original.supersedes.len());
            assert!(
                copy.supersedes
                    .iter()
                    .all(|id| !original.supersedes.contains(id))
            );
        }
        let source_conversation: lettuce_types::ConversationId =
            source.conversation_id.parse().expect("source id");
        let source_branch =
            lettuce_conversations::ConversationReader::get(database, source_conversation)
                .expect("source aggregate")
                .conversation
                .active_branch_id;
        let source_space = lettuce_memory::MemoryRepository::get_for_branch(
            database,
            source_conversation,
            source_branch,
        )
        .expect("source memory")
        .expect("source space");
        lettuce_memory::MemoryRepository::compare_and_apply(
            database,
            lettuce_memory::MemoryChangeSet {
                space_id: source_space.id,
                expected_revision: source_space.revision,
                items: vec![lettuce_memory::MemoryItem::written(
                    memory_id,
                    lettuce_memory::MemoryShortId::derived(memory_id),
                    "Likes tea".into(),
                    lettuce_types::TimestampMillis::new(1),
                )],
            },
        )
        .expect("source memory item");
        let aggregate =
            lettuce_conversations::ConversationReader::get(database, source_conversation)
                .expect("source aggregate");
        conversation_add_user_message(
            &harness.context,
            dto::ConversationAddUserMessageRequest {
                conversation_id: source.conversation_id.clone(),
                text: "hello".into(),
                expected_revision: aggregate.conversation.revision.get(),
                client_operation_id: format!("soul-copy-message-{shared}"),
            },
        )
        .await
        .expect("source message");
        let with_messages = conversation_duplicate(
            &harness.context,
            dto::ConversationDuplicateRequest {
                conversation_id: source.conversation_id.clone(),
                title: None,
                with_messages: true,
                client_operation_id: format!("soul-copy-messages-{shared}"),
            },
        )
        .await
        .expect("duplicate with messages");
        let copied_conversation: lettuce_types::ConversationId =
            with_messages.conversation_id.parse().expect("copy id");
        let copied_branch =
            lettuce_conversations::ConversationReader::get(database, copied_conversation)
                .expect("copy aggregate")
                .conversation
                .active_branch_id;
        let copied_item = lettuce_memory::MemoryRepository::get_for_branch(
            database,
            copied_conversation,
            copied_branch,
        )
        .expect("copy memory")
        .expect("copy space")
        .items
        .into_iter()
        .find(|item| item.text == "Likes tea")
        .expect("copied memory item");
        assert_ne!(copied_item.id, memory_id);
        let remapped = SoulRepository::get(database, owner_of(&with_messages.conversation_id))
            .expect("copy soul")
            .expect("copy has own soul");
        for fact in &remapped.facts {
            assert_eq!(
                fact.source_memory_ids,
                vec![copied_item.id.to_string(), "pool-memory".to_owned()]
            );
        }
    }
}

#[tokio::test]
async fn private_duplicate_persona_switch_never_touches_the_shared_relationship() {
    use lettuce_companions::{
        CompanionStateOwner, CompanionStateReplacement, CompanionStateRepository,
    };
    let harness = harness(Reply::Text("reply"));
    let database = harness.context.backend().database();
    let character_id = super::tests::create_character(
        database,
        "Companion",
        lettuce_characters::CharacterDefaults {
            interaction_mode: lettuce_characters::InteractionMode::Companion,
            companion_soul: Some(lettuce_companions::CompanionSoulConfig::default()),
            memory_policy: lettuce_characters::MemoryPolicy::Dynamic,
            ..lettuce_characters::CharacterDefaults::default()
        },
    );
    let launch_companion = |key: &'static str| {
        let context = harness.context.clone();
        async move {
            conversation_launch_direct(
                &context,
                dto::LaunchDirectRequest {
                    character_id: character_id.to_string(),
                    title: None,
                    scene_id: None,
                    starter_id: None,
                    client_operation_id: key.into(),
                },
            )
            .await
            .expect("launch companion")
        }
    };
    let source = launch_companion("persona-source").await;
    let other = launch_companion("persona-other").await;
    let duplicate = conversation_duplicate(
        &harness.context,
        dto::ConversationDuplicateRequest {
            conversation_id: source.conversation_id.clone(),
            title: None,
            with_messages: false,
            client_operation_id: "persona-duplicate".into(),
        },
    )
    .await
    .expect("duplicate companion");
    let shared_persona = lettuce_types::PersonaId::new();
    let fresh_persona = lettuce_types::PersonaId::new();
    let owner_of = |id: &str, persona_id| CompanionStateOwner {
        conversation_id: id.parse().expect("conversation id"),
        character_id,
        persona_id,
    };
    let advance = |owner: CompanionStateOwner, closeness: f64| {
        let current = CompanionStateRepository::get(database, owner)
            .expect("state")
            .expect("state exists");
        let mut state = current.state;
        state.relationship_state.closeness = closeness;
        CompanionStateRepository::replace(
            database,
            owner,
            lettuce_types::OperationRecordId::new(),
            CompanionStateReplacement {
                expected_session_revision: current.session_revision,
                expected_relationship_revision: current.relationship_revision,
                state,
                applied_at: harness.context.now(),
            },
        )
    };
    let shared_relationships = |persona_id| {
        let graph = lettuce_transfer::ProviderBackupSource::read_provider_backup_graph(database)
            .expect("graph");
        graph
            .companion_state
            .relationships
            .into_iter()
            .find(|row| row.character_id == character_id && row.persona_id == persona_id)
    };
    advance(owner_of(&source.conversation_id, None), 0.9).expect("source moves its own row");
    advance(owner_of(&other.conversation_id, Some(shared_persona)), 0.61)
        .expect("other chat claims the shared persona row");
    let shared_before = shared_relationships(Some(shared_persona)).expect("shared row");
    assert!(shared_relationships(Some(fresh_persona)).is_none());

    advance(
        owner_of(&duplicate.conversation_id, Some(shared_persona)),
        -0.27,
    )
    .expect("duplicate switches to a persona another chat already uses");
    assert_eq!(
        shared_relationships(Some(shared_persona)).expect("shared row"),
        shared_before
    );
    let switched = CompanionStateRepository::get(
        database,
        owner_of(&duplicate.conversation_id, Some(shared_persona)),
    )
    .expect("duplicate state")
    .expect("duplicate");
    assert!((switched.state.relationship_state.closeness + 0.27).abs() < 1e-9);
    advance(
        owner_of(&duplicate.conversation_id, Some(shared_persona)),
        -0.35,
    )
    .expect("later state write succeeds");

    advance(
        owner_of(&duplicate.conversation_id, Some(fresh_persona)),
        0.19,
    )
    .expect("duplicate switches to a persona with no shared row");
    let config = lettuce_companions::CompanionSoulConfig::default();
    let defaults = lettuce_companions::initial_runtime_state(
        &config.soul.baseline_affect,
        &config.soul.regulation_style,
        &config.relationship_defaults,
    )
    .relationship_state;
    let anchor = shared_relationships(Some(fresh_persona)).expect("anchor row");
    assert_eq!(anchor.state, defaults);
    assert_eq!(anchor.revision, lettuce_types::Revision::INITIAL);
    let fresh_chat = launch_companion("persona-fresh").await;
    let fresh = CompanionStateRepository::get(
        database,
        owner_of(&fresh_chat.conversation_id, Some(fresh_persona)),
    )
    .expect("fresh chat state")
    .expect("fresh chat");
    assert_eq!(fresh.state.relationship_state, defaults);
    assert_eq!(
        shared_relationships(Some(shared_persona)).expect("shared row"),
        shared_before
    );
}

#[tokio::test]
async fn fork_at_a_message_whose_owning_branch_was_deleted_uses_the_branch_that_sees_it() {
    let harness = harness(Reply::Text("reply"));
    let conversation_id = launch(&harness, "orphan-fork-launch").await;
    let list_request = dto::ConversationRequest {
        conversation_id: conversation_id.clone(),
    };
    let initial = conversation_branches(&harness.context, list_request.clone())
        .await
        .expect("root");
    let root = initial.branches[0].id.clone();
    let first = conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: conversation_id.clone(),
            text: "one".into(),
            expected_revision: initial.revision,
            client_operation_id: "orphan-fork-first".into(),
        },
    )
    .await
    .expect("first message");
    let owner = conversation_branch_fork(
        &harness.context,
        dto::ConversationBranchForkRequest {
            conversation_id: conversation_id.clone(),
            message_id: first.message.id.clone(),
            expected_revision: first.revision,
            client_operation_id: "orphan-fork-owner".into(),
        },
    )
    .await
    .expect("owner branch");
    let shared = conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: conversation_id.clone(),
            text: "shared".into(),
            expected_revision: owner.revision,
            client_operation_id: "orphan-fork-shared".into(),
        },
    )
    .await
    .expect("shared message");
    let survivor = conversation_branch_fork(
        &harness.context,
        dto::ConversationBranchForkRequest {
            conversation_id: conversation_id.clone(),
            message_id: shared.message.id.clone(),
            expected_revision: shared.revision,
            client_operation_id: "orphan-fork-survivor".into(),
        },
    )
    .await
    .expect("survivor branch");
    let deleted = conversation_branch_delete(
        &harness.context,
        dto::ConversationBranchMutationRequest {
            branch_id: owner.branch_id.clone(),
            expected_revision: survivor.revision,
            client_operation_id: "orphan-fork-delete".into(),
        },
    )
    .await
    .expect("delete owner branch");
    let forked = conversation_branch_fork(
        &harness.context,
        dto::ConversationBranchForkRequest {
            conversation_id: conversation_id.clone(),
            message_id: shared.message.id.clone(),
            expected_revision: deleted.revision,
            client_operation_id: "orphan-fork-again".into(),
        },
    )
    .await
    .expect("fork at the surviving shared message");
    let list = conversation_branches(&harness.context, list_request)
        .await
        .expect("list");
    let created = list
        .branches
        .iter()
        .find(|branch| branch.id == forked.branch_id)
        .expect("new branch");
    assert_eq!(
        created.parent_branch_id.as_deref(),
        Some(owner.branch_id.as_str())
    );
    assert_eq!(
        created.fork_message_id.as_deref(),
        Some(shared.message.id.as_str())
    );
    assert!(created.active);
    assert_ne!(created.parent_branch_id.as_deref(), Some(root.as_str()));
    let replay = conversation_branch_fork(
        &harness.context,
        dto::ConversationBranchForkRequest {
            conversation_id,
            message_id: shared.message.id.clone(),
            expected_revision: deleted.revision,
            client_operation_id: "orphan-fork-again".into(),
        },
    )
    .await
    .expect("replay");
    assert_eq!(replay, forked);
}

#[tokio::test]
async fn refusing_to_delete_the_root_or_selected_branch_says_which() {
    let harness = harness(Reply::Text("reply"));
    let conversation_id = launch(&harness, "refusal-launch").await;
    let initial = conversation_branches(
        &harness.context,
        dto::ConversationRequest {
            conversation_id: conversation_id.clone(),
        },
    )
    .await
    .expect("root");
    let root = initial.branches[0].id.clone();
    let message = conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: conversation_id.clone(),
            text: "one".into(),
            expected_revision: initial.revision,
            client_operation_id: "refusal-message".into(),
        },
    )
    .await
    .expect("message");
    let fork = conversation_branch_fork(
        &harness.context,
        dto::ConversationBranchForkRequest {
            conversation_id,
            message_id: message.message.id.clone(),
            expected_revision: message.revision,
            client_operation_id: "refusal-fork".into(),
        },
    )
    .await
    .expect("fork");
    let refuse = |branch_id: &str, key: &str| {
        let request = dto::ConversationBranchMutationRequest {
            branch_id: branch_id.to_owned(),
            expected_revision: fork.revision,
            client_operation_id: key.to_owned(),
        };
        let context = harness.context.clone();
        async move {
            conversation_branch_delete(&context, request)
                .await
                .expect_err("refused")
        }
    };
    let selected = refuse(&fork.branch_id, "refusal-selected").await;
    assert_eq!(selected.code, ApiErrorCode::Conflict);
    assert_eq!(
        selected.details,
        Some(dto::ApiErrorDetails::BranchDeleteRefused {
            reason: dto::BranchDeleteRefusal::SelectedBranch
        })
    );
    let root_refusal = refuse(&root, "refusal-root").await;
    assert_eq!(root_refusal.code, ApiErrorCode::Conflict);
    assert_eq!(
        root_refusal.details,
        Some(dto::ApiErrorDetails::BranchDeleteRefused {
            reason: dto::BranchDeleteRefusal::RootBranch
        })
    );
    let stale = conversation_branch_delete(
        &harness.context,
        dto::ConversationBranchMutationRequest {
            branch_id: root,
            expected_revision: 1,
            client_operation_id: "refusal-stale".into(),
        },
    )
    .await
    .expect_err("stale revision");
    assert_eq!(stale.code, ApiErrorCode::Conflict);
    assert_eq!(stale.details, None);
}

async fn lazy_peer_race(peer_advances: bool) {
    use lettuce_companions::{CompanionStateOwner, CompanionStateRepository};
    use lettuce_sync::{
        IncomingBatchState, IncomingChangeRepository, LocalChangeJournal, SyncDeviceId,
    };
    let harness = harness(Reply::Text("reply"));
    let database = harness.context.backend().database();
    let character_id = super::tests::create_character(
        database,
        "Companion",
        lettuce_characters::CharacterDefaults {
            interaction_mode: lettuce_characters::InteractionMode::Companion,
            companion_soul: Some(lettuce_companions::CompanionSoulConfig::default()),
            memory_policy: lettuce_characters::MemoryPolicy::Dynamic,
            ..lettuce_characters::CharacterDefaults::default()
        },
    );
    let source = conversation_launch_direct(
        &harness.context,
        dto::LaunchDirectRequest {
            character_id: character_id.to_string(),
            title: None,
            scene_id: None,
            starter_id: None,
            client_operation_id: "lazy-race-source".into(),
        },
    )
    .await
    .expect("source companion");
    let copied = conversation_duplicate(
        &harness.context,
        dto::ConversationDuplicateRequest {
            conversation_id: source.conversation_id.clone(),
            title: None,
            with_messages: false,
            client_operation_id: "lazy-race-copy".into(),
        },
    )
    .await
    .expect("duplicate companion");
    let owner = CompanionStateOwner {
        conversation_id: copied.conversation_id.parse().expect("copy id"),
        character_id,
        persona_id: None,
    };
    let peer =
        crate::AppBackend::open_in_memory(lettuce_types::TimestampMillis::new(1)).expect("peer");
    let apply = |changes: Vec<lettuce_sync::CanonicalChange>, at: i64| {
        let id = lettuce_types::OperationId::new();
        peer.database()
            .stage_incoming_batch(
                SyncDeviceId::new(),
                id,
                &lettuce_sync::canonical_batch_hash(&changes),
                &changes,
                lettuce_types::TimestampMillis::new(at),
            )
            .expect("stage");
        peer.database()
            .apply_incoming_batch(id, lettuce_types::TimestampMillis::new(at))
            .map(|result| result.state)
    };
    let outbound = |at: i64| {
        database
            .journal_current_state(lettuce_types::TimestampMillis::new(at))
            .expect("scan source");
        database
            .outbound_changes(
                &peer.database().local_frontier().expect("frontier"),
                lettuce_sync::MAX_OUTBOUND_CHANGES,
                lettuce_sync::MAX_OUTBOUND_PAYLOAD_BYTES,
            )
            .expect("outbound")
            .changes
    };
    let (sessions, rest): (Vec<_>, Vec<_>) = outbound(1_000)
        .into_iter()
        .partition(|change| change.entity().kind() == lettuce_sync::COMPANION_SESSION_SYNC_KIND);
    assert!(!sessions.is_empty());
    assert_eq!(apply(rest, 1_001), Ok(IncomingBatchState::Committed));
    let config = lettuce_companions::CompanionSoulConfig::default();
    CompanionStateRepository::create(
        peer.database(),
        owner,
        lettuce_companions::initial_runtime_state(
            &config.soul.baseline_affect,
            &config.soul.regulation_style,
            &config.relationship_defaults,
        ),
        lettuce_types::TimestampMillis::new(400),
    )
    .expect("peer lazily creates the state");
    if peer_advances {
        let local = CompanionStateRepository::get(peer.database(), owner)
            .expect("peer state")
            .expect("peer copy");
        let mut moved = local.state.clone();
        moved.relationship_state.closeness = 0.77;
        CompanionStateRepository::replace(
            peer.database(),
            owner,
            lettuce_types::OperationRecordId::new(),
            lettuce_companions::CompanionStateReplacement {
                expected_session_revision: local.session_revision,
                expected_relationship_revision: local.relationship_revision,
                state: moved,
                applied_at: harness.context.now(),
            },
        )
        .expect("peer advances its own state");
    }
    peer.database()
        .journal_current_state(lettuce_types::TimestampMillis::new(500))
        .expect("scan peer");
    let current = CompanionStateRepository::get(database, owner)
        .expect("copy state")
        .expect("copy");
    let mut advanced = current.state.clone();
    advanced.relationship_state.closeness = -0.42;
    CompanionStateRepository::replace(
        database,
        owner,
        lettuce_types::OperationRecordId::new(),
        lettuce_companions::CompanionStateReplacement {
            expected_session_revision: current.session_revision,
            expected_relationship_revision: current.relationship_revision,
            state: advanced.clone(),
            applied_at: harness.context.now(),
        },
    )
    .expect("advance the duplicate");
    let applied = apply(outbound(2_001), 2_002);
    let private_on_peer = || {
        lettuce_transfer::ProviderBackupSource::read_provider_backup_graph(peer.database())
            .expect("peer graph")
            .companion_state
            .sessions
            .iter()
            .find(|session| session.owner == owner)
            .expect("peer session")
            .private_relationships
            .is_some()
    };
    if peer_advances {
        assert!(applied.is_err());
        assert!(!private_on_peer());
        return;
    }
    assert_eq!(applied, Ok(IncomingBatchState::Committed));
    let on_peer = CompanionStateRepository::get(peer.database(), owner)
        .expect("peer state")
        .expect("peer copy");
    assert_eq!(on_peer.state, advanced);
    assert!(private_on_peer());
}

#[tokio::test]
async fn private_duplicate_sync_wins_over_a_peer_that_lazily_created_the_state() {
    lazy_peer_race(false).await;
}

#[tokio::test]
async fn private_duplicate_sync_conflicts_with_a_peer_state_that_already_advanced() {
    lazy_peer_race(true).await;
}
