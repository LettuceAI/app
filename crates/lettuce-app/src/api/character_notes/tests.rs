use crate::api::tests::{Reply, harness};
use crate::api::*;
use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_types::CharacterId;

fn note(character: CharacterId, key: &str) -> dto::CharacterNoteUpsertRequest {
    dto::CharacterNoteUpsertRequest {
        character_id: character.to_string(),
        note_id: None,
        label: "  Birthday  ".into(),
        content: "  Her birthday is today.  ".into(),
        available_at: 1_000,
        expires_at: Some(5_000),
        recurrence: dto::CharacterNoteRecurrence::Yearly,
        recurrence_window_ms: Some(2_000),
        enabled: true,
        client_operation_id: key.into(),
    }
}

#[tokio::test]
async fn notes_of_a_roleplay_character_get_api_assigned_ids_and_timestamps() {
    let harness = harness(Reply::Text("reply"));
    let character = harness.character_id;
    let created = character_notes_upsert(&harness.context, note(character, "note-create"))
        .await
        .expect("create");
    assert!(uuid::Uuid::parse_str(&created.id).is_ok());
    assert_eq!(created.label, "Birthday");
    assert_eq!(created.content, "Her birthday is today.");
    assert_eq!(created.created_at, created.updated_at);
    assert!(created.created_at > 0);
    assert_eq!(
        character_notes_upsert(&harness.context, note(character, "note-create"))
            .await
            .expect("replay"),
        created
    );
    let mut changed = note(character, "note-create");
    changed.content = "Another note".into();
    assert_eq!(
        character_notes_upsert(&harness.context, changed)
            .await
            .expect_err("changed request")
            .code,
        ApiErrorCode::Conflict
    );
    let mut update = note(character, "note-update");
    update.note_id = Some(created.id.clone());
    update.content = "Updated content".into();
    let updated = character_notes_upsert(&harness.context, update)
        .await
        .expect("update");
    assert_eq!(updated.id, created.id);
    assert_eq!(updated.created_at, created.created_at);
    assert!(updated.updated_at >= created.updated_at);
    assert_eq!(updated.content, "Updated content");
    let listed = character_notes_list(
        &harness.context,
        dto::CharacterNotesRequest {
            character_id: character.to_string(),
        },
    )
    .await
    .expect("list");
    assert_eq!(listed, vec![updated.clone()]);

    let mut unknown = note(character, "note-unknown");
    unknown.note_id = Some(uuid::Uuid::new_v4().to_string());
    assert_eq!(
        character_notes_upsert(&harness.context, unknown)
            .await
            .expect_err("the API assigns ids")
            .code,
        ApiErrorCode::NotFound
    );
    let missing = character_notes_upsert(
        &harness.context,
        note(CharacterId::new(), "note-no-character"),
    )
    .await
    .expect_err("character not found");
    assert_eq!(missing.code, ApiErrorCode::NotFound);
    let mut backwards = note(character, "note-backwards");
    backwards.expires_at = Some(500);
    assert_eq!(
        character_notes_upsert(&harness.context, backwards)
            .await
            .expect_err("expires before it starts")
            .code,
        ApiErrorCode::InvalidInput
    );
    let mut empty = note(character, "note-empty");
    empty.content = "   ".into();
    assert_eq!(
        character_notes_upsert(&harness.context, empty)
            .await
            .expect_err("empty content")
            .code,
        ApiErrorCode::InvalidInput
    );
    let preview = |as_of| {
        let context = harness.context.clone();
        async move {
            character_notes_active_preview(
                &context,
                dto::CharacterNotesActivePreviewRequest {
                    character_id: character.to_string(),
                    as_of,
                },
            )
            .await
            .expect("preview")
        }
    };
    assert_eq!(preview(1_500).await.len(), 1);
    assert!(preview(500).await.is_empty());
    assert!(preview(6_000).await.is_empty());
    let delete = dto::CharacterNoteDeleteRequest {
        note_id: created.id.clone(),
        client_operation_id: "note-delete".into(),
    };
    character_notes_delete(&harness.context, delete.clone())
        .await
        .expect("delete");
    character_notes_delete(&harness.context, delete)
        .await
        .expect("replay");
    character_notes_delete(
        &harness.context,
        dto::CharacterNoteDeleteRequest {
            note_id: created.id,
            client_operation_id: "note-delete-missing".into(),
        },
    )
    .await
    .expect("a missing note deletes silently");
    assert!(
        character_notes_list(
            &harness.context,
            dto::CharacterNotesRequest {
                character_id: character.to_string(),
            },
        )
        .await
        .expect("list")
        .is_empty()
    );
}

#[tokio::test]
async fn notes_before_1970_round_trip_with_the_exact_instant() {
    use chrono::TimeZone;
    let local = |month, day, hour| {
        chrono::Local
            .with_ymd_and_hms(1890, month, day, hour, 0, 0)
            .earliest()
            .expect("local 1890 time")
            .timestamp_millis()
    };
    let harness = harness(Reply::Text("reply"));
    let character = harness.character_id;
    let mut request = note(character, "note-1890");
    request.available_at = local(3, 14, 10);
    request.expires_at = Some(local(9, 1, 10));
    request.recurrence = dto::CharacterNoteRecurrence::None;
    request.recurrence_window_ms = None;
    let created = character_notes_upsert(&harness.context, request)
        .await
        .expect("create an 1890 note");
    assert_eq!(created.available_at, local(3, 14, 10));
    assert_eq!(created.expires_at, Some(local(9, 1, 10)));
    let listed = character_notes_list(
        &harness.context,
        dto::CharacterNotesRequest {
            character_id: character.to_string(),
        },
    )
    .await
    .expect("list");
    assert_eq!(listed, vec![created.clone()]);
    let active = character_notes_active_preview(
        &harness.context,
        dto::CharacterNotesActivePreviewRequest {
            character_id: character.to_string(),
            as_of: local(5, 1, 12),
        },
    )
    .await
    .expect("preview at an 1890 time");
    assert_eq!(active, vec![created]);
    let mut same = note(character, "note-1890-same");
    same.available_at = local(3, 14, 10);
    same.expires_at = Some(local(3, 14, 10));
    assert_eq!(
        character_notes_upsert(&harness.context, same)
            .await
            .expect_err("expires when it starts")
            .code,
        ApiErrorCode::InvalidInput
    );
}
