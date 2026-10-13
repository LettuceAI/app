use lettuce_characters::{InteractionMode, ProfileDuplicateRequest};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_types::CharacterId;

use crate::api::ApiContext;
use crate::api::error::{IntoApiError, api_error, parse_id};
use crate::api::lorebooks::Failure;
use crate::api::mapping;

pub async fn character_duplicate(
    context: &ApiContext,
    request: dto::CharacterDuplicateRequest,
) -> Result<dto::CharacterSummary, ApiError> {
    let id: CharacterId = parse_id(&request.character_id, "character_id")?;
    let digest = crate::api::jobs::local::digest(&request)?;
    let installed = context
        .blocking(|context| Ok(crate::api::models::installed_models(context)))
        .await?;
    let missing_models = crate::api::models::missing_models(
        &[dto::RequiredModel::Embedding, dto::RequiredModel::Emotion],
        &installed,
    );
    context
        .blocking(move |context| {
            if context.shutdown_token().is_cancelled() {
                return Err(api_error(
                    ApiErrorCode::Cancelled,
                    "shutdown interrupted duplication",
                ));
            }
            context
                .backend()
                .database()
                .commit_api_operation(
                    "character_duplicate",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        let source = scope
                            .library_character(id)
                            .map_err(|error| Failure(error.into_api_error()))?;
                        if source.defaults.interaction_mode == InteractionMode::Companion
                            && !missing_models.is_empty()
                        {
                            let mut error = api_error(
                                ApiErrorCode::ModelRequired,
                                "companion creation requires installed models",
                            );
                            error.details =
                                Some(dto::ApiErrorDetails::MissingModels { missing_models });
                            return Err(Failure(error));
                        }
                        let copied = scope
                            .duplicate_character(ProfileDuplicateRequest {
                                source_character_id: id,
                                destination_character_id: CharacterId::new(),
                                destination_name: Some(format!("{} (Copy)", source.profile.name)),
                                now: context.now(),
                            })
                            .map_err(|error| Failure(error.into_api_error()))?;
                        let character = scope
                            .library_character(copied.character_id)
                            .map_err(|error| Failure(error.into_api_error()))?;
                        Ok(mapping::character_summary(context, &character))
                    },
                )
                .map_err(|failure: Failure| failure.0)
        })
        .await
}
