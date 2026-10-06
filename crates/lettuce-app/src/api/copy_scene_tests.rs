use super::tests::{Reply, harness};
use super::*;
use lettuce_contracts as dto;

#[tokio::test]
async fn group_character_copies_replace_known_names_and_keep_current_scene_rules() {
    use lettuce_characters::{
        CreateGroupPlan, GroupMember, GroupProfile, GroupRepository, GroupStartingScene, Scene,
        SceneDocumentV1, SceneOwner, ScenePart,
    };
    let harness = harness(Reply::Text("reply"));
    let other = super::branches_tests::copy_second_character(&harness);
    let group_id = lettuce_types::GroupId::new();
    let scene = Scene::new(
        lettuce_types::SceneId::new(),
        SceneOwner::Group(group_id),
        0,
        SceneDocumentV1 {
            format_version: 1,
            parts: vec![ScenePart::Text {
                text: "Current group scene".into(),
            }],
        },
        harness.context.now(),
    )
    .expect("scene");
    let mut profile = GroupProfile::new(
        group_id,
        "Source Group".into(),
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
        harness.context.now(),
    )
    .expect("profile");
    profile.chat_mode = lettuce_characters::ChatMode::Roleplay;
    profile.starting_scene_id = Some(scene.id);
    GroupRepository::create(
        harness.context.backend().database(),
        CreateGroupPlan {
            group: profile,
            starting_scene: Some(GroupStartingScene {
                scene: scene.clone(),
                variants: Vec::new(),
            }),
        },
    )
    .expect("source group");
    let source = conversation_launch_group(
        &harness.context,
        dto::LaunchGroupRequest {
            group_id: group_id.to_string(),
            client_operation_id: "copy-source-group-launch".into(),
        },
    )
    .await
    .expect("group chat");
    let root = conversation_branches(
        &harness.context,
        dto::ConversationRequest {
            conversation_id: source.conversation_id.clone(),
        },
    )
    .await
    .expect("source root");
    let message = conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: source.conversation_id.clone(),
            text: "Hello {{@\"Other\"}} and {{@\"Unknown\"}}".into(),
            expected_revision: root.revision,
            client_operation_id: "copy-group-user".into(),
        },
    )
    .await
    .expect("user");
    let from_message = dto::ConversationCharacterCopyRequest {
        conversation_id: source.conversation_id.clone(),
        message_id: message.message.id.clone(),
        character_id: harness.character_id.to_string(),
        client_operation_id: "copy-group-prefix".into(),
    };
    let prefix =
        conversation_branch_to_character_from_message(&harness.context, from_message.clone())
            .await
            .expect("prefix copy");
    let whole_request = dto::ConversationGroupCharacterCopyRequest {
        conversation_id: source.conversation_id.clone(),
        character_id: harness.character_id.to_string(),
        client_operation_id: "copy-group-whole".into(),
    };
    let whole = conversation_branch_to_character(&harness.context, whole_request.clone())
        .await
        .expect("whole copy");
    for copy in [&prefix, &whole] {
        let target = lettuce_conversations::ConversationReader::get(
            harness.context.backend().database(),
            copy.conversation_id.parse().expect("target id"),
        )
        .expect("target");
        assert_eq!(target.conversation.title, "Source Group - Ada");
        assert_eq!(target.conversation.origin_conversation_id, None);
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
            timeline
                .items
                .iter()
                .find(|item| item.message.role == lettuce_conversations::MessageRole::User)
                .expect("user message")
                .active_revision
                .as_ref()
                .expect("revision")
                .parts,
            vec![lettuce_conversations::MessagePart::Text {
                text: "Hello Other and {{@\"Unknown\"}}".into()
            }]
        );
        let lettuce_conversations::ConversationKind::Direct(details) = &target.conversation.kind
        else {
            panic!("direct target");
        };
        if copy == &whole {
            assert!(
                matches!(&details.scene,lettuce_conversations::SnapshotSelection::Explicit(value) if value.source_id == scene.id)
            );
        } else {
            assert_eq!(
                details.scene,
                lettuce_conversations::SnapshotSelection::Disabled
            );
        }
    }
    assert_eq!(
        conversation_branch_to_character_from_message(&harness.context, from_message)
            .await
            .expect("prefix replay"),
        prefix
    );
    assert_eq!(
        conversation_branch_to_character(&harness.context, whole_request)
            .await
            .expect("whole replay"),
        whole
    );
}

#[tokio::test]
async fn group_copies_follow_the_chat_snapshot_after_the_profile_changes() {
    use lettuce_characters::{
        CreateGroupPlan, GroupMember, GroupProfile, GroupRepository, GroupStartingScene, Scene,
        SceneDocumentV1, SceneOwner, ScenePart, Selection,
    };
    let harness = harness(Reply::Text("reply"));
    let database = harness.context.backend().database();
    let other = super::branches_tests::copy_second_character(&harness);
    let third = super::branches_tests::copy_second_character(&harness);
    let launch_persona = crate::launch::tests::seed_persona(database, "Launch persona");
    let edited_persona = crate::launch::tests::seed_persona(database, "Edited persona");
    let group_id = lettuce_types::GroupId::new();
    let scene_of = |text: &str| {
        Scene::new(
            lettuce_types::SceneId::new(),
            SceneOwner::Group(group_id),
            0,
            SceneDocumentV1 {
                format_version: 1,
                parts: vec![ScenePart::Text { text: text.into() }],
            },
            harness.context.now(),
        )
        .expect("scene")
    };
    let launch_scene = scene_of("Launch scene");
    let member = |character_id, ordinal| GroupMember {
        character_id,
        ordinal,
        muted: false,
        model_profile_override: None,
    };
    let mut profile = GroupProfile::new(
        group_id,
        "Launch Group".into(),
        vec![
            member(harness.character_id, 0),
            member(other, 1),
            member(third, 2),
        ],
        harness.context.now(),
    )
    .expect("profile");
    profile.chat_mode = lettuce_characters::ChatMode::Roleplay;
    profile.persona = Selection::Explicit(launch_persona);
    profile.starting_scene_id = Some(launch_scene.id);
    GroupRepository::create(
        database,
        CreateGroupPlan {
            group: profile,
            starting_scene: Some(GroupStartingScene {
                scene: launch_scene.clone(),
                variants: Vec::new(),
            }),
        },
    )
    .expect("group");
    let source = conversation_launch_group(
        &harness.context,
        dto::LaunchGroupRequest {
            group_id: group_id.to_string(),
            client_operation_id: "snapshot-copy-launch".into(),
        },
    )
    .await
    .expect("group chat");

    let revision = |database: &lettuce_database::Database| {
        GroupRepository::get(database, group_id)
            .expect("group")
            .expect("details")
            .group
            .revision
    };
    let now = harness.context.now();
    GroupRepository::rename(
        database,
        group_id,
        revision(database),
        "Renamed Group".into(),
        now,
    )
    .expect("rename");
    GroupRepository::set_persona(
        database,
        group_id,
        revision(database),
        Selection::Explicit(edited_persona),
        now,
    )
    .expect("persona");
    GroupRepository::set_starting_scene(
        database,
        group_id,
        revision(database),
        Some(GroupStartingScene {
            scene: scene_of("Edited scene"),
            variants: Vec::new(),
        }),
        now,
    )
    .expect("scene");
    GroupRepository::replace_members(
        database,
        group_id,
        revision(database),
        vec![member(harness.character_id, 0), member(third, 1)],
        now,
    )
    .expect("remove member");

    let copied = conversation_branch_to_character(
        &harness.context,
        dto::ConversationGroupCharacterCopyRequest {
            conversation_id: source.conversation_id.clone(),
            character_id: other.to_string(),
            client_operation_id: "snapshot-copy-whole".into(),
        },
    )
    .await
    .expect("copy to the member the profile no longer lists");
    let target = lettuce_conversations::ConversationReader::get(
        database,
        copied.conversation_id.parse().expect("target id"),
    )
    .expect("target");
    assert_eq!(target.conversation.title, "Launch Group - Other");
    let lettuce_conversations::ConversationKind::Direct(details) = &target.conversation.kind else {
        panic!("direct target");
    };
    assert!(matches!(
        &details.persona,
        lettuce_conversations::SnapshotSelection::Explicit(value)
            if value.source_id == launch_persona
    ));
    assert!(matches!(
        &details.scene,
        lettuce_conversations::SnapshotSelection::Explicit(value)
            if value.source_id == launch_scene.id
    ));
}
