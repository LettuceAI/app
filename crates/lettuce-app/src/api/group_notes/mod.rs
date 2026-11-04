#[cfg(test)]
mod tests;

use lettuce_companions::{
    GroupScheduledNote, GroupScheduledNoteError, GroupScheduledNoteRepository,
    ScheduledNoteRecurrence,
};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_types::{CharacterId, GroupId, TimestampMillis};
use uuid::Uuid;

use super::ApiContext;
use super::error::{api_error, invalid_field, parse_id};
use super::jobs::local::stable_uuid;
use super::memory::EditFailure;
use super::memory::control::controlled_atomic;

fn note_error(error: GroupScheduledNoteError) -> ApiError {
    api_error(
        match error {
            GroupScheduledNoteError::GroupNotFound => ApiErrorCode::NotFound,
            GroupScheduledNoteError::Invalid { .. } | GroupScheduledNoteError::NotAMember => {
                ApiErrorCode::InvalidInput
            }
            GroupScheduledNoteError::Failure { .. } | GroupScheduledNoteError::Corrupt { .. } => {
                ApiErrorCode::Internal
            }
        },
        error.to_string(),
    )
}

fn recurrence(value: dto::CharacterNoteRecurrence) -> ScheduledNoteRecurrence {
    match value {
        dto::CharacterNoteRecurrence::None => ScheduledNoteRecurrence::None,
        dto::CharacterNoteRecurrence::Daily => ScheduledNoteRecurrence::Daily,
        dto::CharacterNoteRecurrence::Weekly => ScheduledNoteRecurrence::Weekly,
        dto::CharacterNoteRecurrence::Monthly => ScheduledNoteRecurrence::Monthly,
        dto::CharacterNoteRecurrence::Yearly => ScheduledNoteRecurrence::Yearly,
    }
}

fn note_view(note: &GroupScheduledNote) -> dto::GroupNoteView {
    dto::GroupNoteView {
        id: note.id.to_string(),
        group_id: note.group_id.to_string(),
        visible_to: note.visible_to.iter().map(ToString::to_string).collect(),
        label: note.label.clone(),
        content: note.content.clone(),
        available_at: note.available_at.get(),
        expires_at: note.expires_at.map(TimestampMillis::get),
        recurrence: match note.recurrence {
            ScheduledNoteRecurrence::None => dto::CharacterNoteRecurrence::None,
            ScheduledNoteRecurrence::Daily => dto::CharacterNoteRecurrence::Daily,
            ScheduledNoteRecurrence::Weekly => dto::CharacterNoteRecurrence::Weekly,
            ScheduledNoteRecurrence::Monthly => dto::CharacterNoteRecurrence::Monthly,
            ScheduledNoteRecurrence::Yearly => dto::CharacterNoteRecurrence::Yearly,
        },
        recurrence_window_ms: note.recurrence_window_ms,
        enabled: note.enabled,
        created_at: note.created_at.get(),
        updated_at: note.updated_at.get(),
    }
}

/// The group's scheduled notes in the order they become available.
pub async fn group_notes_list(
    context: &ApiContext,
    request: dto::GroupNotesRequest,
) -> Result<Vec<dto::GroupNoteView>, ApiError> {
    let group_id: GroupId = parse_id(&request.group_id, "group_id")?;
    context
        .blocking(move |context| {
            GroupScheduledNoteRepository::list_group_notes(
                context.backend().database(),
                group_id,
            )
            .map(|notes| notes.iter().map(note_view).collect())
            .map_err(note_error)
        })
        .await
}

/// Creates a note, or updates the one `note_id` names. The API assigns the
/// id and the timestamps: a retry of the same request keeps the id it first
/// assigned, and an update keeps the note's creation time. A character the
/// note is visible to must be a current member of the group.
pub async fn group_notes_upsert(
    context: &ApiContext,
    request: dto::GroupNoteUpsertRequest,
) -> Result<dto::GroupNoteView, ApiError> {
    let group_id: GroupId = parse_id(&request.group_id, "group_id")?;
    let note_id: Option<Uuid> = request
        .note_id
        .as_deref()
        .map(|value| parse_id(value, "note_id"))
        .transpose()?;
    let visible_to = request
        .visible_to
        .iter()
        .map(|value| parse_id::<CharacterId>(value, "visible_to"))
        .collect::<Result<Vec<_>, _>>()?;
    if request.content.trim().is_empty() {
        return Err(invalid_field("content", "the note needs content"));
    }
    if request
        .expires_at
        .is_some_and(|expires| expires <= request.available_at)
    {
        return Err(invalid_field(
            "expires_at",
            "the end must be after the start",
        ));
    }
    let key = request.client_operation_id.clone();
    let action_key = key.clone();
    let action_request = request.clone();
    controlled_atomic(
        context,
        "group_notes_upsert",
        &key,
        &request,
        move |context| {
            let request = action_request;
            let database = context.backend().database();
            let existing = GroupScheduledNoteRepository::list_group_notes(database, group_id)
                .map_err(note_error)?;
            let id = match note_id {
                Some(id) => {
                    if existing.iter().all(|note| note.id != id) {
                        return Err(api_error(ApiErrorCode::NotFound, "the note was not found"));
                    }
                    id
                }
                None => stable_uuid(&["group-scheduled-note", &action_key]),
            };
            let now = context.now();
            let created_at = existing
                .iter()
                .find(|note| note.id == id)
                .map_or(now, |note| note.created_at);
            Ok(GroupScheduledNote {
                id,
                group_id,
                visible_to,
                label: request.label,
                content: request.content,
                available_at: TimestampMillis::new(request.available_at),
                expires_at: request.expires_at.map(TimestampMillis::new),
                recurrence: recurrence(request.recurrence),
                recurrence_window_ms: request.recurrence_window_ms,
                enabled: request.enabled,
                created_at,
                updated_at: now,
            })
        },
        move |scope, note| {
            scope
                .upsert_group_note(note)
                .map(|stored| note_view(&stored))
                .map_err(|error| EditFailure(note_error(error)))
        },
    )
    .await
}

/// Deletes a note; a note that does not exist is not an error.
pub async fn group_notes_delete(
    context: &ApiContext,
    request: dto::GroupNoteDeleteRequest,
) -> Result<(), ApiError> {
    let note_id: Uuid = parse_id(&request.note_id, "note_id")?;
    let key = request.client_operation_id.clone();
    controlled_atomic(
        context,
        "group_notes_delete",
        &key,
        &request,
        |_| Ok(()),
        move |scope, ()| {
            scope
                .delete_group_note(note_id)
                .map_err(|error| EditFailure(note_error(error)))
        },
    )
    .await
}
