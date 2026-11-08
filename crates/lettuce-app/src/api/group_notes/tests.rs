use crate::api::library::groups_tests::draft;
use crate::api::tests::{Reply, harness};
use crate::api::*;
use lettuce_contracts::{self as dto, ApiErrorCode};

async fn saved_group(harness: &crate::api::tests::Harness) -> dto::GroupView {
    let mut draft = draft(&harness.context, harness.character_id.to_string()).await;
    draft.time_awareness = true;
    draft.disable_character_notes = true;
    draft.companion_mode = true;
    draft.companion_personal_memories = false;
    draft.companion_memory_write_back = false;
    group_save(
        &harness.context,
        dto::GroupSaveRequest {
            group_id: None,
            draft,
            expected_revision: None,
            client_operation_id: "notes-group".into(),
        },
    )
    .await
    .expect("group")
}

fn note(group: &dto::GroupView, visible_to: Vec<String>, key: &str) -> dto::GroupNoteUpsertRequest {
    dto::GroupNoteUpsertRequest {
        group_id: group.id.clone(),
        note_id: None,
        visible_to,
        label: "  Supplies  ".into(),
        content: "  Food is running low.  ".into(),
        available_at: 1_000,
        expires_at: Some(5_000),
        recurrence: dto::CharacterNoteRecurrence::Weekly,
        recurrence_window_ms: Some(2_000),
        enabled: true,
        client_operation_id: key.into(),
    }
}

#[tokio::test]
async fn the_group_keeps_its_time_default_and_character_notes_switch() {
    let harness = harness(Reply::Text("reply"));
    let group = saved_group(&harness).await;
    assert!(
        group.draft.time_awareness
            && group.draft.disable_character_notes
            && group.draft.companion_mode
    );
    assert!(!group.draft.companion_personal_memories && !group.draft.companion_memory_write_back);
    let fetched = group_get(&harness.context, dto::GroupGetRequest { group_id: group.id.clone() })
        .await
        .expect("get");
    assert!(
        fetched.draft.time_awareness
            && fetched.draft.disable_character_notes
            && fetched.draft.companion_mode
    );
    assert!(
        !fetched.draft.companion_personal_memories && !fetched.draft.companion_memory_write_back
    );
}

#[tokio::test]
async fn group_notes_get_api_assigned_ids_and_timestamps_and_can_be_edited_and_deleted() {
    let harness = harness(Reply::Text("reply"));
    let group = saved_group(&harness).await;
    let member = group.draft.members[0].character_id.clone();
    let created = group_notes_upsert(&harness.context, note(&group, vec![member.clone()], "note-create"))
        .await
        .expect("create");
    assert!(uuid::Uuid::parse_str(&created.id).is_ok());
    assert_eq!((created.label.as_str(), created.content.as_str()), ("Supplies", "Food is running low."));
    assert_eq!(created.visible_to, vec![member.clone()]);
    assert_eq!(created.created_at, created.updated_at);
    assert_eq!(
        group_notes_upsert(&harness.context, note(&group, vec![member.clone()], "note-create"))
            .await
            .expect("replay"),
        created
    );
    let mut changed = note(&group, Vec::new(), "note-create");
    changed.content = "Another note".into();
    assert_eq!(
        group_notes_upsert(&harness.context, changed).await.expect_err("changed request").code,
        ApiErrorCode::Conflict
    );

    let mut update = note(&group, Vec::new(), "note-update");
    update.note_id = Some(created.id.clone());
    update.content = "Updated content".into();
    let updated = group_notes_upsert(&harness.context, update).await.expect("update");
    assert_eq!(updated.id, created.id);
    assert_eq!(updated.created_at, created.created_at);
    assert!(updated.visible_to.is_empty());
    let listed = group_notes_list(&harness.context, dto::GroupNotesRequest { group_id: group.id.clone() })
        .await
        .expect("list");
    assert_eq!(listed, vec![updated]);

    let delete = dto::GroupNoteDeleteRequest { note_id: created.id.clone(), client_operation_id: "note-delete".into() };
    group_notes_delete(&harness.context, delete.clone()).await.expect("delete");
    group_notes_delete(&harness.context, dto::GroupNoteDeleteRequest { client_operation_id: "note-delete-again".into(), ..delete })
        .await
        .expect("a missing note is not an error");
    assert!(group_notes_list(&harness.context, dto::GroupNotesRequest { group_id: group.id.clone() })
        .await
        .expect("list")
        .is_empty());
}

#[tokio::test]
async fn group_notes_reject_non_members_unknown_groups_and_bad_requests() {
    let harness = harness(Reply::Text("reply"));
    let group = saved_group(&harness).await;
    let outsider = lettuce_types::CharacterId::new().to_string();
    assert_eq!(
        group_notes_upsert(&harness.context, note(&group, vec![outsider], "outsider"))
            .await
            .expect_err("non-member")
            .code,
        ApiErrorCode::InvalidInput
    );
    let mut lost = note(&group, Vec::new(), "lost");
    lost.group_id = lettuce_types::GroupId::new().to_string();
    assert_eq!(
        group_notes_upsert(&harness.context, lost).await.expect_err("unknown group").code,
        ApiErrorCode::NotFound
    );
    assert_eq!(
        group_notes_list(&harness.context, dto::GroupNotesRequest { group_id: lettuce_types::GroupId::new().to_string() })
            .await
            .expect_err("unknown group")
            .code,
        ApiErrorCode::NotFound
    );
    let mut blank = note(&group, Vec::new(), "blank");
    blank.content = "   ".into();
    assert_eq!(group_notes_upsert(&harness.context, blank).await.expect_err("blank").code, ApiErrorCode::InvalidInput);
    let mut backwards = note(&group, Vec::new(), "backwards");
    backwards.expires_at = Some(500);
    assert_eq!(group_notes_upsert(&harness.context, backwards).await.expect_err("backwards").code, ApiErrorCode::InvalidInput);
    let mut missing = note(&group, Vec::new(), "missing");
    missing.note_id = Some(uuid::Uuid::new_v4().to_string());
    assert_eq!(group_notes_upsert(&harness.context, missing).await.expect_err("missing note").code, ApiErrorCode::NotFound);
}

#[tokio::test]
async fn deleting_a_group_removes_its_notes() {
    let harness = harness(Reply::Text("reply"));
    let group = saved_group(&harness).await;
    group_notes_upsert(&harness.context, note(&group, Vec::new(), "to-be-removed")).await.expect("note");
    group_delete(
        &harness.context,
        dto::GroupDeleteRequest {
            group_id: group.id.clone(),
            expected_revision: group.revision,
            client_operation_id: "delete-group".into(),
        },
    )
    .await
    .expect("delete group");
    assert_eq!(
        group_notes_list(&harness.context, dto::GroupNotesRequest { group_id: group.id.clone() })
            .await
            .expect_err("the group is gone")
            .code,
        ApiErrorCode::NotFound
    );
}
