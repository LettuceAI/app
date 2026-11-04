#[cfg(test)]
mod tests;

use lettuce_companions::{
    CompanionScheduledNote, CompanionScheduledNoteError, CompanionScheduledNoteRepository,
    ScheduledNoteRecurrence,
};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_types::{CharacterId, TimestampMillis};
use uuid::Uuid;

use super::ApiContext;
use super::error::{api_error, invalid_field, parse_id};
use super::jobs::local::stable_uuid;
use super::memory::control::controlled_atomic;
use super::memory::EditFailure;

fn note_error(error: CompanionScheduledNoteError) -> ApiError {
    api_error(
        match error {
            CompanionScheduledNoteError::NotFound
            | CompanionScheduledNoteError::CharacterNotFound => ApiErrorCode::NotFound,
            CompanionScheduledNoteError::Invalid { .. } => ApiErrorCode::InvalidInput,
            CompanionScheduledNoteError::Conflict => ApiErrorCode::Conflict,
            CompanionScheduledNoteError::Failure { .. } | CompanionScheduledNoteError::Corrupt { .. } => {
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

fn note_view(note: &CompanionScheduledNote) -> dto::CharacterNoteView {
    dto::CharacterNoteView {
        id: note.id.to_string(),
        character_id: note.character_id.to_string(),
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

/// The character's scheduled notes in the order they become available.
pub async fn character_notes_list(
    context: &ApiContext,
    request: dto::CharacterNotesRequest,
) -> Result<Vec<dto::CharacterNoteView>, ApiError> {
    let character_id: CharacterId = parse_id(&request.character_id, "character_id")?;
    context
        .blocking(move |context| {
            CompanionScheduledNoteRepository::list_scheduled_notes(
                context.backend().database(),
                character_id,
            )
            .map(|notes| notes.iter().map(note_view).collect())
            .map_err(note_error)
        })
        .await
}

/// Creates a note, or updates the one `note_id` names. The API assigns the
/// id and the timestamps: a retry of the same request keeps the id it first
/// assigned, and an update keeps the note's creation time.
pub async fn character_notes_upsert(
    context: &ApiContext,
    request: dto::CharacterNoteUpsertRequest,
) -> Result<dto::CharacterNoteView, ApiError> {
    let character_id: CharacterId = parse_id(&request.character_id, "character_id")?;
    let note_id: Option<Uuid> = request
        .note_id
        .as_deref()
        .map(|value| parse_id(value, "note_id"))
        .transpose()?;
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
        "character_notes_upsert",
        &key,
        &request,
        move |context| {
            let request = action_request;
            let database = context.backend().database();
            let existing =
                CompanionScheduledNoteRepository::list_scheduled_notes(database, character_id)
                    .map_err(note_error)?;
            let id = match note_id {
                Some(id) => {
                    if existing.iter().all(|note| note.id != id) {
                        return Err(api_error(ApiErrorCode::NotFound, "the note was not found"));
                    }
                    id
                }
                None => stable_uuid(&["companion-scheduled-note", &action_key]),
            };
            let now = context.now();
            let created_at = existing
                .iter()
                .find(|note| note.id == id)
                .map_or(now, |note| note.created_at);
            Ok(CompanionScheduledNote {
                    id,
                    character_id,
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
            scope.upsert_companion_note(note).map(|stored| note_view(&stored))
                .map_err(|error| EditFailure(note_error(error)))
        },
    )
    .await
}

/// Deletes a note; a note that does not exist is not an error.
pub async fn character_notes_delete(
    context: &ApiContext,
    request: dto::CharacterNoteDeleteRequest,
) -> Result<(), ApiError> {
    let note_id: Uuid = parse_id(&request.note_id, "note_id")?;
    let key = request.client_operation_id.clone();
    controlled_atomic(
        context, "character_notes_delete", &key, &request,
        |_| Ok(()),
        move |scope, ()| scope.delete_companion_note(note_id).map_err(|error| EditFailure(note_error(error))),
    )
    .await
}

/// The enabled notes that apply at `as_of`.
pub async fn character_notes_active_preview(
    context: &ApiContext,
    request: dto::CharacterNotesActivePreviewRequest,
) -> Result<Vec<dto::CharacterNoteView>, ApiError> {
    let character_id: CharacterId = parse_id(&request.character_id, "character_id")?;
    context
        .blocking(move |context| {
            let notes = CompanionScheduledNoteRepository::list_scheduled_notes(
                context.backend().database(),
                character_id,
            )
            .map_err(note_error)?;
            lettuce_companions::active_scheduled_notes(notes, TimestampMillis::new(request.as_of))
                .map(|notes| notes.iter().map(note_view).collect())
                .map_err(note_error)
        })
        .await
}
