use lettuce_characters::{CreateGroupPlan, GroupDetails, GroupRepository};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_types::{GroupId, TimestampMillis};

use super::ApiContext;
use super::error::{IntoApiError, api_error, invalid_field, parse_id};
use super::lorebooks::Failure;

fn mapping_error() -> ApiError {
    api_error(
        ApiErrorCode::Internal,
        "group editor contract could not be mapped",
    )
}

fn view(context: &ApiContext, details: GroupDetails) -> Result<dto::GroupView, ApiError> {
    let group = &details.group;
    let mut assets = group.presentation.referenced_asset_ids();
    assets.extend(group.background_asset_id);
    if let Some(starting) = &details.starting_scene {
        assets.extend(starting.scene.assets.iter().map(|link| link.asset_id));
    }
    let mut value = serde_json::to_value(&details).map_err(|_| mapping_error())?;
    let mut root = value["group"]
        .as_object()
        .ok_or_else(mapping_error)?
        .clone();
    for key in [
        "id",
        "status",
        "revision",
        "created_at",
        "updated_at",
        "starting_scene_id",
    ] {
        root.remove(key);
    }
    if !value["starting_scene"].is_null() {
        let starting = value["starting_scene"]
            .as_object_mut()
            .ok_or_else(mapping_error)?;
        let scene = starting
            .get_mut("scene")
            .ok_or_else(mapping_error)?
            .as_object_mut()
            .ok_or_else(mapping_error)?;
        for key in ["owner", "status", "revision", "created_at", "updated_at"] {
            scene.remove(key);
        }
        for variant in starting
            .get_mut("variants")
            .ok_or_else(mapping_error)?
            .as_array_mut()
            .ok_or_else(mapping_error)?
        {
            let variant = variant.as_object_mut().ok_or_else(mapping_error)?;
            for key in ["revision", "created_at", "updated_at"] {
                variant.remove(key);
            }
        }
    }
    root.insert("starting_scene".into(), value["starting_scene"].take());
    let draft =
        serde_json::from_value(serde_json::Value::Object(root)).map_err(|_| mapping_error())?;
    Ok(dto::GroupView {
        id: group.id.to_string(),
        archived: group.status == lettuce_characters::LifecycleStatus::Archived,
        draft,
        assets: assets.into_iter().map(|id| context.asset_ref(id)).collect(),
        revision: group.revision.get(),
        created_at: group.created_at.get(),
        updated_at: group.updated_at.get(),
    })
}

fn plan_from(
    draft: dto::GroupDraft,
    id: GroupId,
    now: TimestampMillis,
) -> Result<CreateGroupPlan, ApiError> {
    let mut value = serde_json::to_value(draft).map_err(|_| mapping_error())?;
    let root = value.as_object_mut().ok_or_else(mapping_error)?;
    let mut starting = root.remove("starting_scene").ok_or_else(mapping_error)?;
    let mut scene_id = serde_json::Value::Null;
    if !starting.is_null() {
        let starting = starting.as_object_mut().ok_or_else(mapping_error)?;
        let scene = starting
            .get_mut("scene")
            .ok_or_else(mapping_error)?
            .as_object_mut()
            .ok_or_else(mapping_error)?;
        scene_id = scene.get("id").ok_or_else(mapping_error)?.clone();
        scene.insert(
            "owner".into(),
            serde_json::json!({"kind": "group", "id": id.to_string()}),
        );
        scene.insert("status".into(), "active".into());
        scene.insert("revision".into(), 1.into());
        scene.insert("created_at".into(), now.get().into());
        scene.insert("updated_at".into(), now.get().into());
        for variant in starting
            .get_mut("variants")
            .ok_or_else(mapping_error)?
            .as_array_mut()
            .ok_or_else(mapping_error)?
        {
            let variant = variant.as_object_mut().ok_or_else(mapping_error)?;
            variant.insert("revision".into(), 1.into());
            variant.insert("created_at".into(), now.get().into());
            variant.insert("updated_at".into(), now.get().into());
        }
    }
    root.insert("starting_scene_id".into(), scene_id);
    root.insert("id".into(), id.to_string().into());
    root.insert("status".into(), "active".into());
    root.insert("revision".into(), 1.into());
    root.insert("created_at".into(), now.get().into());
    root.insert("updated_at".into(), now.get().into());
    let details: GroupDetails =
        serde_json::from_value(serde_json::json!({"group": value, "starting_scene": starting}))
            .map_err(|_| {
                invalid_field(
                    "draft",
                    "group draft contains an invalid identifier or value",
                )
            })?;
    Ok(CreateGroupPlan {
        group: details.group,
        starting_scene: details.starting_scene,
    })
}

pub async fn group_get(
    context: &ApiContext,
    request: dto::GroupGetRequest,
) -> Result<dto::GroupView, ApiError> {
    let id: GroupId = parse_id(&request.group_id, "group_id")?;
    context
        .blocking(move |context| {
            let details = GroupRepository::get(context.backend().database(), id)
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(|| api_error(ApiErrorCode::NotFound, "group not found"))?;
            view(context, details)
        })
        .await
}

pub async fn group_save(
    context: &ApiContext,
    request: dto::GroupSaveRequest,
) -> Result<dto::GroupView, ApiError> {
    let id: Option<GroupId> = request
        .group_id
        .as_deref()
        .map(|id| parse_id(id, "group_id"))
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
    let plan = plan_from(request.draft, id.unwrap_or_default(), context.now())?;
    context
        .blocking(move |context| {
            if context.shutdown_token().is_cancelled() {
                return Err(api_error(
                    ApiErrorCode::Cancelled,
                    "shutdown interrupted group save",
                ));
            }
            context
                .backend()
                .database()
                .commit_api_operation(
                    "group_save",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        let details = scope
                            .save_group(plan, expected, context.now())
                            .map_err(|error| Failure(error.into_api_error()))?;
                        view(context, details).map_err(Failure)
                    },
                )
                .map_err(|failure: Failure| failure.0)
        })
        .await
}

pub async fn groups_list(
    context: &ApiContext,
    request: dto::GroupsListRequest,
) -> Result<dto::GroupPage, ApiError> {
    context
        .blocking(move |context| {
            let page = GroupRepository::search(
                context.backend().database(),
                lettuce_characters::GroupSearch {
                    text: request.query.unwrap_or_default(),
                    include_archived: false,
                    sort: match request.sort {
                        None | Some(dto::LibrarySort::UpdatedDesc) => {
                            lettuce_characters::LibrarySort::UpdatedDesc
                        }
                        Some(dto::LibrarySort::CreatedAsc) => {
                            lettuce_characters::LibrarySort::CreatedAsc
                        }
                        Some(dto::LibrarySort::Name) => lettuce_characters::LibrarySort::Name,
                    },
                },
                lettuce_types::PageRequest {
                    cursor: request.cursor,
                    limit: super::mapping::page_limit(Some(request.limit)),
                },
            )
            .map_err(IntoApiError::into_api_error)?;
            Ok(dto::GroupPage {
                items: page
                    .items
                    .into_iter()
                    .map(|item| {
                        Ok(dto::GroupListItem {
                            group: view(context, item.details)?,
                            last_activity_at: item.last_activity_at.get(),
                        })
                    })
                    .collect::<Result<Vec<_>, ApiError>>()?,
                next_cursor: page.next_cursor,
            })
        })
        .await
}
