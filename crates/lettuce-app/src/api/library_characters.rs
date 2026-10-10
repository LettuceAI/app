use lettuce_characters::{
    CharacterDetails, CharacterRepository, CreateCharacterPlan, InteractionMode,
};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_types::{CharacterId, TimestampMillis};

use super::ApiContext;
use super::error::{IntoApiError, api_error, invalid_field, parse_id};
use super::lorebooks::Failure;

fn mapping_error() -> ApiError {
    api_error(
        ApiErrorCode::Internal,
        "character editor contract could not be mapped",
    )
}

fn view(context: &ApiContext, details: CharacterDetails) -> Result<dto::CharacterView, ApiError> {
    let character = &details.character;
    let mut assets = std::collections::BTreeSet::new();
    assets.extend(character.media.links.iter().map(|link| link.asset_id));
    assets.extend(character.presentation.referenced_asset_ids());
    assets.extend(
        details
            .scenes
            .iter()
            .flat_map(|scene| scene.assets.iter().map(|link| link.asset_id)),
    );
    let mut value = serde_json::to_value(&details).map_err(|_| mapping_error())?;
    let mut root = value["character"]
        .as_object()
        .ok_or_else(mapping_error)?
        .clone();
    for key in ["id", "status", "revision", "created_at", "updated_at"] {
        root.remove(key);
    }
    root.insert("media".into(), value["character"]["media"]["links"].clone());
    root.get_mut("defaults")
        .ok_or_else(mapping_error)?
        .as_object_mut()
        .ok_or_else(mapping_error)?
        .insert(
            "companion_soul".into(),
            serde_json::to_value(
                character
                    .defaults
                    .companion_soul
                    .as_ref()
                    .map(super::companion::config_view),
            )
            .map_err(|_| mapping_error())?,
        );
    for key in ["scenes", "variants", "starters"] {
        let children = value[key].as_array_mut().ok_or_else(mapping_error)?;
        for child in children.iter_mut() {
            let child = child.as_object_mut().ok_or_else(mapping_error)?;
            for key in [
                "owner",
                "status",
                "character_id",
                "revision",
                "created_at",
                "updated_at",
            ] {
                child.remove(key);
            }
        }
        root.insert(
            key.into(),
            serde_json::Value::Array(std::mem::take(children)),
        );
    }
    let draft =
        serde_json::from_value(serde_json::Value::Object(root)).map_err(|_| mapping_error())?;
    Ok(dto::CharacterView {
        id: character.id.to_string(),
        archived: character.status == lettuce_characters::LifecycleStatus::Archived,
        draft,
        assets: assets.into_iter().map(|id| context.asset_ref(id)).collect(),
        revision: character.revision.get(),
        created_at: character.created_at.get(),
        updated_at: character.updated_at.get(),
    })
}

fn plan_from(
    draft: dto::CharacterDraft,
    id: CharacterId,
    now: TimestampMillis,
) -> Result<CreateCharacterPlan, ApiError> {
    let config = draft
        .defaults
        .companion_soul
        .as_ref()
        .map(super::companion::config_from)
        .transpose()?;
    let mut value = serde_json::to_value(draft).map_err(|_| mapping_error())?;
    let root = value.as_object_mut().ok_or_else(mapping_error)?;
    let mut graph = serde_json::Map::new();
    for key in ["scenes", "variants", "starters"] {
        let mut children = root.remove(key).ok_or_else(mapping_error)?;
        for child in children.as_array_mut().ok_or_else(mapping_error)? {
            let child = child.as_object_mut().ok_or_else(mapping_error)?;
            child.insert("revision".into(), 1.into());
            child.insert("created_at".into(), now.get().into());
            child.insert("updated_at".into(), now.get().into());
            match key {
                "scenes" => {
                    child.insert(
                        "owner".into(),
                        serde_json::json!({"kind": "character", "id": id.to_string()}),
                    );
                    child.insert("status".into(), "active".into());
                }
                "starters" => {
                    child.insert("character_id".into(), id.to_string().into());
                }
                _ => {}
            }
        }
        graph.insert(key.into(), children);
    }
    let media = root.remove("media").ok_or_else(mapping_error)?;
    root.insert("media".into(), serde_json::json!({"links": media}));
    root.insert("id".into(), id.to_string().into());
    root.insert("status".into(), "active".into());
    root.insert("revision".into(), 1.into());
    root.insert("created_at".into(), now.get().into());
    root.insert("updated_at".into(), now.get().into());
    root.get_mut("defaults")
        .ok_or_else(mapping_error)?
        .as_object_mut()
        .ok_or_else(mapping_error)?
        .insert(
            "companion_soul".into(),
            serde_json::to_value(config).map_err(|_| mapping_error())?,
        );
    graph.insert("character".into(), value);
    let details: CharacterDetails = serde_json::from_value(serde_json::Value::Object(graph))
        .map_err(|_| {
            invalid_field(
                "draft",
                "character draft contains an invalid identifier or value",
            )
        })?;
    Ok(CreateCharacterPlan {
        character: details.character,
        scenes: details.scenes,
        variants: details.variants,
        starters: details.starters,
    })
}

pub async fn character_get(
    context: &ApiContext,
    request: dto::CharacterGetRequest,
) -> Result<dto::CharacterView, ApiError> {
    let id: CharacterId = parse_id(&request.character_id, "character_id")?;
    context
        .blocking(move |context| {
            let details = CharacterRepository::get(context.backend().database(), id)
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(|| api_error(ApiErrorCode::NotFound, "character not found"))?;
            view(context, details)
        })
        .await
}

pub async fn character_save(
    context: &ApiContext,
    request: dto::CharacterSaveRequest,
) -> Result<dto::CharacterView, ApiError> {
    let id: Option<CharacterId> = request
        .character_id
        .as_deref()
        .map(|id| parse_id(id, "character_id"))
        .transpose()?;
    let expected = request
        .expected_revision
        .map(|value| super::lorebooks::revision(value, "expected_revision"))
        .transpose()?;
    if id.is_some() != expected.is_some() {
        return Err(invalid_field(
            "expected_revision",
            "updates require a revision; creation omits it",
        ));
    }
    let digest = super::jobs::local::digest(&request)?;
    let mut plan = plan_from(request.draft, id.unwrap_or_default(), context.now())?;
    let installed = context
        .blocking(|context| Ok(super::models::installed_models(context)))
        .await?;
    let missing_models = super::models::missing_models(
        &[dto::RequiredModel::Embedding, dto::RequiredModel::Emotion],
        &installed,
    );
    context
        .blocking(move |context| {
            if context.shutdown_token().is_cancelled() {
                return Err(api_error(
                    ApiErrorCode::Cancelled,
                    "shutdown interrupted character save",
                ));
            }
            let default_rules = if id.is_none() && plan.character.profile.rules.is_empty() {
                let level =
                    lettuce_settings::GlobalSettingsStore::load(context.backend().database())
                        .map(|snapshot| snapshot.settings.pure_mode)
                        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()));
                level.and_then(|level| {
                    crate::generation::pure_mode_rules::default_character_rules(
                        context.backend().database(),
                        level,
                    )
                    .map_err(|_| super::prompts::runtime_text_error())
                })
            } else {
                Ok(Vec::new())
            };
            context
                .backend()
                .database()
                .commit_api_operation(
                    "character_save",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        let creating_companion = plan.character.defaults.interaction_mode
                            == InteractionMode::Companion
                            && match id {
                                None => true,
                                Some(id) => {
                                    scope
                                        .library_character(id)
                                        .map_err(|error| Failure(error.into_api_error()))?
                                        .defaults
                                        .interaction_mode
                                        != InteractionMode::Companion
                                }
                            };
                        if creating_companion && !missing_models.is_empty() {
                            let mut error = api_error(
                                ApiErrorCode::ModelRequired,
                                "companion creation requires installed models",
                            );
                            error.details =
                                Some(dto::ApiErrorDetails::MissingModels { missing_models });
                            return Err(Failure(error));
                        }
                        if id.is_none() && plan.character.profile.rules.is_empty() {
                            plan.character.profile.rules = default_rules.map_err(Failure)?;
                        }
                        let details = scope
                            .save_character(plan, expected, context.now())
                            .map_err(|error| Failure(error.into_api_error()))?;
                        view(context, details).map_err(Failure)
                    },
                )
                .map_err(|failure: Failure| failure.0)
        })
        .await
}
