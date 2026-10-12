use lettuce_characters::{Persona, PersonaDraftUpdate, PersonaMedia, PersonaRepository};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_types::PersonaId;

use crate::api::ApiContext;
use crate::api::error::{IntoApiError, api_error, invalid_field, parse_id};
use crate::api::lorebooks::Failure;

pub(in crate::api) fn view(context: &ApiContext, persona: Persona) -> dto::PersonaView {
    dto::PersonaView {
        id: persona.id.to_string(),
        archived: persona.status == lettuce_characters::LifecycleStatus::Archived,
        assets: persona
            .media
            .links
            .iter()
            .map(|link| context.asset_ref(link.asset_id))
            .collect(),
        draft: dto::PersonaDraft {
            title: persona.title,
            description: persona.description,
            nickname: persona.nickname,
            design_description: persona.design_description,
            avatar_crop: persona.avatar_crop.map(|crop| dto::LibraryCrop {
                x: crop.x,
                y: crop.y,
                scale: crop.scale,
            }),
            image_recommendation: persona.image_recommendation.map(|value| {
                dto::LibraryImageRecommendation {
                    artifact_id: value.artifact_id.map(|id| id.to_string()),
                    unresolved_legacy_name: value.unresolved_legacy_name,
                    strength: value.strength,
                }
            }),
            media: persona
                .media
                .links
                .into_iter()
                .map(|link| dto::PersonaMediaLink {
                    asset_id: link.asset_id.to_string(),
                    ordinal: link.ordinal,
                    slot: match link.slot {
                        lettuce_characters::PersonaMediaSlot::Avatar => {
                            dto::PersonaMediaSlot::Avatar
                        }
                        lettuce_characters::PersonaMediaSlot::DesignReference => {
                            dto::PersonaMediaSlot::DesignReference
                        }
                    },
                })
                .collect(),
        },
        revision: persona.revision.get(),
        created_at: persona.created_at.get(),
        updated_at: persona.updated_at.get(),
    }
}

pub async fn persona_get(
    context: &ApiContext,
    request: dto::PersonaGetRequest,
) -> Result<dto::PersonaView, ApiError> {
    let id: PersonaId = parse_id(&request.persona_id, "persona_id")?;
    context
        .blocking(move |context| {
            let persona = PersonaRepository::get(context.backend().database(), id)
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(|| api_error(ApiErrorCode::NotFound, "persona not found"))?;
            Ok(view(context, persona))
        })
        .await
}

pub async fn persona_save(
    context: &ApiContext,
    request: dto::PersonaSaveRequest,
) -> Result<dto::PersonaView, ApiError> {
    let id: Option<PersonaId> = request
        .persona_id
        .as_deref()
        .map(|id| parse_id(id, "persona_id"))
        .transpose()?;
    let expected = request
        .expected_revision
        .map(|revision| crate::api::lorebooks::revision(revision, "expected_revision"))
        .transpose()?;
    if id.is_some() != expected.is_some() {
        return Err(invalid_field(
            "expected_revision",
            "updates require a revision; creation omits it",
        ));
    }
    let digest = crate::api::jobs::local::digest(&request)?;
    let draft = PersonaDraftUpdate {
        title: request.draft.title,
        description: request.draft.description,
        nickname: request.draft.nickname,
        design_description: request.draft.design_description,
        avatar_crop: request
            .draft
            .avatar_crop
            .map(|crop| lettuce_characters::Crop {
                x: crop.x,
                y: crop.y,
                scale: crop.scale,
            }),
        image_recommendation: request
            .draft
            .image_recommendation
            .map(|value| {
                Ok::<_, ApiError>(lettuce_characters::ImageRecommendation {
                    artifact_id: value
                        .artifact_id
                        .as_deref()
                        .map(|id| parse_id(id, "artifact_id"))
                        .transpose()?,
                    unresolved_legacy_name: value.unresolved_legacy_name,
                    strength: value.strength,
                })
            })
            .transpose()?,
    };
    let media = PersonaMedia {
        links: request
            .draft
            .media
            .into_iter()
            .map(|link| {
                Ok::<_, ApiError>(lettuce_characters::PersonaMediaLink {
                    asset_id: parse_id(&link.asset_id, "asset_id")?,
                    ordinal: link.ordinal,
                    slot: match link.slot {
                        dto::PersonaMediaSlot::Avatar => {
                            lettuce_characters::PersonaMediaSlot::Avatar
                        }
                        dto::PersonaMediaSlot::DesignReference => {
                            lettuce_characters::PersonaMediaSlot::DesignReference
                        }
                    },
                })
            })
            .collect::<Result<_, _>>()?,
    };
    context
        .blocking(move |context| {
            if context.shutdown_token().is_cancelled() {
                return Err(api_error(
                    ApiErrorCode::Cancelled,
                    "shutdown interrupted persona save",
                ));
            }
            context
                .backend()
                .database()
                .commit_api_operation(
                    "persona_save",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        let persona = scope
                            .save_persona(id, expected, draft, media, context.now())
                            .map_err(|error| Failure(error.into_api_error()))?;
                        Ok(view(context, persona))
                    },
                )
                .map_err(|failure: Failure| failure.0)
        })
        .await
}

pub async fn personas_list(
    context: &ApiContext,
    request: dto::PersonasListRequest,
) -> Result<dto::PersonaPage, ApiError> {
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let page = PersonaRepository::search(
                database,
                lettuce_characters::PersonaSearch {
                    text: request.query.unwrap_or_default(),
                    include_archived: false,
                    sort: crate::api::mapping::library_sort(request.sort),
                    search_nickname: request.search_nickname,
                },
                lettuce_types::PageRequest {
                    cursor: request.cursor,
                    limit: crate::api::mapping::page_limit(request.limit),
                },
            )
            .map_err(IntoApiError::into_api_error)?;
            let state = PersonaRepository::get_default_snapshot(database)
                .map_err(IntoApiError::into_api_error)?
                .state;
            Ok(dto::PersonaPage {
                items: page
                    .items
                    .into_iter()
                    .map(|persona| dto::PersonaSummary {
                        id: persona.id.to_string(),
                        title: persona.title,
                        nickname: persona.nickname,
                        avatar: persona
                            .media
                            .links
                            .iter()
                            .find(|link| link.slot == lettuce_characters::PersonaMediaSlot::Avatar)
                            .map(|link| context.asset_ref(link.asset_id)),
                    })
                    .collect(),
                next_cursor: page.next_cursor,
                default_persona: dto::PersonaDefaultView {
                    persona_id: state.persona_id.map(|id| id.to_string()),
                    revision: state.revision.get(),
                    created_at: state.created_at.get(),
                    updated_at: state.updated_at.get(),
                },
            })
        })
        .await
}
