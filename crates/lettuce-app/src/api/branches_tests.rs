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
