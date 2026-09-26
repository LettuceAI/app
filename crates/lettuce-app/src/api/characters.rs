use lettuce_characters::CharacterRepository;
use lettuce_contracts::{self as dto, ApiError};
use lettuce_types::PageRequest;

use super::ApiContext;
use super::error::IntoApiError;
use super::mapping;

/// Active characters in library order, for the launch picker.
pub async fn characters_list(
    context: &ApiContext,
    request: dto::CharactersListRequest,
) -> Result<dto::CharacterPage, ApiError> {
    context
        .blocking(move |context| {
            let page = CharacterRepository::list(
                context.backend().database(),
                PageRequest {
                    cursor: request.cursor,
                    limit: mapping::page_limit(request.limit),
                },
                false,
            )
            .map_err(IntoApiError::into_api_error)?;
            Ok(dto::CharacterPage {
                items: page
                    .items
                    .iter()
                    .map(|character| dto::CharacterSummary {
                        id: character.id.to_string(),
                        name: character.profile.name.clone(),
                        avatar: mapping::character_avatar(character)
                            .map(|asset_id| context.asset_ref(asset_id)),
                    })
                    .collect(),
                next_cursor: page.next_cursor,
            })
        })
        .await
}
