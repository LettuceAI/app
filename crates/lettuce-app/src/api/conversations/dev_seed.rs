use std::collections::BTreeMap;

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_transfer::{ChatJsonl, ChatJsonlMessage, ChatJsonlRole};
use lettuce_types::{CharacterId, GroupId};

use crate::api::ApiContext;
use crate::api::error::{IntoApiError, api_error, invalid_field, parse_id};
use crate::api::lorebooks::Failure;

fn internal(error: impl std::fmt::Display) -> ApiError {
    api_error(ApiErrorCode::Internal, error.to_string())
}

enum Target {
    Direct(CharacterId),
    Group(GroupId),
}

/// Creates a chat with the given history through the chat import path, so
/// it is stored exactly like an imported transcript. Refused unless
/// developer mode is on. Repeating the call with the same operation id
/// returns the same conversation.
pub async fn dev_conversation_seed(
    context: &ApiContext,
    request: dto::DevConversationSeedRequest,
) -> Result<dto::DevConversationSeeded, ApiError> {
    let digest = crate::api::jobs::local::digest(&request)?;
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let developer = lettuce_settings::GlobalSettingsStore::load(database)
                .map_err(internal)?
                .settings
                .developer_mode_enabled;
            if !developer {
                return Err(api_error(
                    ApiErrorCode::Unsupported,
                    "test chats need developer mode",
                ));
            }
            let (target, chat, speakers) = seed_chat(context, &request)?;
            let files = context.backend().chat_files();
            let mut prepared = match target {
                Target::Direct(character_id) => {
                    files.prepare_direct(chat, None, Some(character_id), context.now())
                }
                Target::Group(group_id) => {
                    files.prepare_existing_group(chat, &speakers, group_id, context.now())
                }
            }
            .map_err(IntoApiError::into_api_error)?;
            if let Some(relationship) = request.relationship {
                let Some((_, state)) = prepared.companion.as_mut() else {
                    return Err(invalid_field(
                        "relationship",
                        "a relationship needs a companion character",
                    ));
                };
                let values = &mut state.relationship_state;
                values.closeness = relationship.closeness;
                values.trust = relationship.trust;
                values.affection = relationship.affection;
                values.tension = relationship.tension;
                values.stability = relationship.stability;
            }
            let conversation_id = prepared.record.history.aggregate.conversation.id;
            database
                .commit_api_operation(
                    "dev_conversation_seed",
                    &request.client_operation_id,
                    &digest,
                    context.now(),
                    |scope| {
                        scope
                            .import_chat(
                                prepared.group,
                                prepared.record,
                                prepared.companion,
                                context.now(),
                            )
                            .map_err(|error| Failure(internal(error)))?;
                        Ok::<_, Failure>(dto::DevConversationSeeded {
                            conversation_id: conversation_id.to_string(),
                        })
                    },
                )
                .map_err(|failure| failure.0)
        })
        .await
}

/// The validated target and the request's lines as a transcript whose
/// assistant speakers are named by character id, with that naming.
fn seed_chat(
    context: &ApiContext,
    request: &dto::DevConversationSeedRequest,
) -> Result<(Target, ChatJsonl, BTreeMap<String, CharacterId>), ApiError> {
    if request.client_operation_id.trim().is_empty() {
        return Err(invalid_field(
            "client_operation_id",
            "client_operation_id is required",
        ));
    }
    let title = request.title.trim();
    if title.is_empty() {
        return Err(invalid_field("title", "the title is blank"));
    }
    if request.lines.is_empty() {
        return Err(invalid_field("lines", "at least one line is required"));
    }
    let (target, members) = match &request.target {
        dto::DevSeedTarget::Direct { character_id } => {
            let id = parse_id::<CharacterId>(character_id, "target.character_id")?;
            (Target::Direct(id), vec![id])
        }
        dto::DevSeedTarget::Group { group_id } => {
            let id = parse_id::<GroupId>(group_id, "target.group_id")?;
            let group = lettuce_characters::GroupRepository::get(context.backend().database(), id)
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(|| api_error(ApiErrorCode::NotFound, "group not found"))?;
            let members = group
                .group
                .members
                .iter()
                .map(|member| member.character_id)
                .collect::<Vec<_>>();
            (Target::Group(id), members)
        }
    };
    if let Some(relationship) = &request.relationship {
        if matches!(target, Target::Group(_)) {
            return Err(invalid_field(
                "relationship",
                "a relationship applies only to a direct chat",
            ));
        }
        let bipolar = [
            relationship.closeness,
            relationship.trust,
            relationship.affection,
        ]
        .into_iter()
        .all(|value| value.is_finite() && (-1.0..=1.0).contains(&value));
        let unipolar = [relationship.tension, relationship.stability]
            .into_iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(&value));
        if !bipolar || !unipolar {
            return Err(invalid_field(
                "relationship",
                "closeness, trust and affection must be within -1 and 1, tension and stability within 0 and 1",
            ));
        }
    }
    let now = context.now().get();
    let mut previous = u32::MAX;
    let mut speakers = BTreeMap::new();
    let mut messages = Vec::with_capacity(request.lines.len());
    for line in &request.lines {
        if line.text.trim().is_empty() {
            return Err(invalid_field("lines.text", "a line is blank"));
        }
        if line.minutes_ago > previous {
            return Err(invalid_field(
                "lines.minutes_ago",
                "lines must run from the oldest to the newest",
            ));
        }
        previous = line.minutes_ago;
        let name = match line.speaker_character_id.as_deref() {
            None => None,
            Some(id) => {
                let id = parse_id::<CharacterId>(id, "lines.speaker_character_id")?;
                if !members.contains(&id) {
                    return Err(invalid_field(
                        "lines.speaker_character_id",
                        format!("{id} is not part of this chat"),
                    ));
                }
                speakers.insert(id.to_string(), id);
                Some(id.to_string())
            }
        };
        messages.push(ChatJsonlMessage {
            role: if name.is_some() {
                ChatJsonlRole::Assistant
            } else {
                ChatJsonlRole::User
            },
            name,
            content: Some(line.text.clone()),
            created_at: now - i64::from(line.minutes_ago) * 60_000,
            swipes: Vec::new(),
            swipe_id: None,
        });
    }
    let chat = ChatJsonl {
        metadata: Some(serde_json::json!({ "character_name": title })),
        messages,
    };
    Ok((target, chat, speakers))
}
