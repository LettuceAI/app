use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_types::{PersonaId, Revision};

use super::ApiContext;
use super::error::{IntoApiError, api_error, parse_id};
use super::lorebooks::Failure;

pub async fn persona_default_set(
    context: &ApiContext,
    request: dto::PersonaDefaultSetRequest,
) -> Result<dto::PersonaDefaultView, ApiError> {
    let id: PersonaId = parse_id(&request.persona_id, "persona_id")?;
    let expected = super::lorebooks::revision(request.expected_revision, "expected_revision")?;
    let digest = super::jobs::local::digest(&request)?;
    write(
        context,
        "persona_default_set",
        Some(id),
        expected,
        request.client_operation_id,
        digest,
    )
    .await
}

pub async fn persona_default_clear(
    context: &ApiContext,
    request: dto::PersonaDefaultClearRequest,
) -> Result<dto::PersonaDefaultView, ApiError> {
    let expected = super::lorebooks::revision(request.expected_revision, "expected_revision")?;
    let digest = super::jobs::local::digest(&request)?;
    write(
        context,
        "persona_default_clear",
        None,
        expected,
        request.client_operation_id,
        digest,
    )
    .await
}

async fn write(
    context: &ApiContext,
    command: &'static str,
    id: Option<PersonaId>,
    expected: Revision,
    key: String,
    digest: String,
) -> Result<dto::PersonaDefaultView, ApiError> {
    context
        .blocking(move |context| {
            if context.shutdown_token().is_cancelled() {
                return Err(api_error(
                    ApiErrorCode::Cancelled,
                    "shutdown interrupted default persona selection",
                ));
            }
            context
                .backend()
                .database()
                .commit_api_operation(command, &key, &digest, context.now(), |scope| {
                    let state = scope
                        .set_library_default_persona(id, expected, context.now())
                        .map_err(|error| Failure(error.into_api_error()))?;
                    Ok(dto::PersonaDefaultView {
                        persona_id: state.persona_id.map(|id| id.to_string()),
                        revision: state.revision.get(),
                        created_at: state.created_at.get(),
                        updated_at: state.updated_at.get(),
                    })
                })
                .map_err(|failure: Failure| failure.0)
        })
        .await
}
