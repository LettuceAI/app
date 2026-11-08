#[cfg(test)]
mod tests;

use lettuce_characters::{CharacterDetails, CharacterRepository, InteractionMode};
use lettuce_companions::{
    CompanionSoulConfig, CompanionStateRepository, CompanionStateSnapshot,
    GroupCompanionEffectReason, GroupCompanionRepository, GroupCompanionTarget, RelationshipState,
    SoulFact, SoulOwner, SoulRepository, SoulRepositoryError, initial_group_member_state,
    initial_group_relationship,
};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_conversations::{
    Conversation, ConversationKind, ConversationLifecycle, ConversationOverviewReader,
    ConversationParticipant, ConversationReader, MessageRole, MessageVisibility,
    ParticipantSource,
};
use lettuce_settings::GlobalSettingsStore;
use lettuce_types::{
    CharacterId, ConversationId, MessageId, ModelProfileId, PageLimit, PageRequest, RequestId,
    TimestampMillis,
};

use super::ApiContext;
use crate::companion::companion_clock::companion_state_owner;
use super::error::{IntoApiError, api_error, invalid_field, parse_id};
use super::jobs::local::stable_uuid;
use super::memory::control::controlled_atomic;
use super::memory::EditFailure;

fn soul_error(error: SoulRepositoryError) -> ApiError {
    api_error(
        match error {
            SoulRepositoryError::NotFound => ApiErrorCode::NotFound,
            SoulRepositoryError::Conflict
            | SoulRepositoryError::AlreadyExists
            | SoulRepositoryError::OperationMismatch => ApiErrorCode::Conflict,
            SoulRepositoryError::Invalid(_)
            | SoulRepositoryError::Corrupt { .. }
            | SoulRepositoryError::Failure { .. } => ApiErrorCode::Internal,
        },
        format!("the Soul could not be updated: {error:?}"),
    )
}

fn character_with_config(
    database: &lettuce_database::Database,
    id: CharacterId,
) -> Result<(CharacterDetails, CompanionSoulConfig), ApiError> {
    let details = CharacterRepository::get(database, id)
        .map_err(IntoApiError::into_api_error)?
        .ok_or_else(|| api_error(ApiErrorCode::NotFound, "the character was not found"))?;
    let config = details
        .character
        .defaults
        .companion_soul
        .clone()
        .unwrap_or_default();
    Ok((details, config))
}

fn require_companion(details: &CharacterDetails) -> Result<(), ApiError> {
    if details.character.defaults.interaction_mode != InteractionMode::Companion {
        return Err(api_error(
            ApiErrorCode::Unsupported,
            "the character is not a companion",
        ));
    }
    Ok(())
}

struct Target {
    character_id: CharacterId,
    conversation_id: Option<ConversationId>,
    owner: SoulOwner,
    config: CompanionSoulConfig,
}

fn target(
    database: &lettuce_database::Database,
    character_id: &str,
    conversation_id: Option<&str>,
) -> Result<Target, ApiError> {
    let character_id: CharacterId = parse_id(character_id, "character_id")?;
    let (details, config) = character_with_config(database, character_id)?;
    let conversation_id: Option<ConversationId> = conversation_id
        .map(|value| parse_id(value, "conversation_id"))
        .transpose()?;
    let owner = match conversation_id {
        None => {
            require_companion(&details)?;
            SoulOwner::Character(character_id)
        }
        Some(conversation_id) => {
            let aggregate = ConversationReader::get(database, conversation_id)
                .map_err(IntoApiError::into_api_error)?;
            if aggregate.conversation.lifecycle == ConversationLifecycle::Tombstoned {
                return Err(api_error(
                    ApiErrorCode::NotFound,
                    "the conversation was not found",
                ));
            }
            let belongs = match &aggregate.conversation.kind {
                ConversationKind::Direct(direct) => {
                    require_companion(&details)?;
                    direct.character.source_id == character_id
                }
                ConversationKind::Group(_) => aggregate
                    .conversation
                    .participants
                    .iter()
                    .any(|participant| {
                        participant.source == ParticipantSource::Character(character_id)
                    }),
            };
            if !belongs {
                return Err(invalid_field(
                    "conversation_id",
                    "the conversation is not a chat with this character",
                ));
            }
            SoulOwner::for_conversation(
                character_id,
                conversation_id,
                config.share_soul_growth_across_chats,
            )
        }
    };
    Ok(Target {
        character_id,
        conversation_id,
        owner,
        config,
    })
}

fn category(value: lettuce_companions::SoulCategory) -> dto::SoulCategory {
    use dto::SoulCategory as D;
    use lettuce_companions::SoulCategory as S;
    match value {
        S::Essence => D::Essence,
        S::Traits => D::Traits,
        S::Backstory => D::Backstory,
        S::Appearance => D::Appearance,
        S::Goals => D::Goals,
        S::Likes => D::Likes,
        S::Voice => D::Voice,
        S::RelationalStyle => D::RelationalStyle,
        S::Vulnerabilities => D::Vulnerabilities,
        S::Fears => D::Fears,
        S::Habits => D::Habits,
        S::Boundaries => D::Boundaries,
    }
}

fn fact_view(fact: &SoulFact) -> dto::SoulFactView {
    dto::SoulFactView {
        id: fact.id.clone(),
        category: category(fact.category),
        value: fact.value.clone(),
        kind: match fact.kind {
            lettuce_companions::SoulFactKind::Add => dto::SoulFactKind::Add,
            lettuce_companions::SoulFactKind::Adjust => dto::SoulFactKind::Adjust,
            lettuce_companions::SoulFactKind::Authored => dto::SoulFactKind::Authored,
            lettuce_companions::SoulFactKind::Consolidated => dto::SoulFactKind::Consolidated,
        },
        policy: match fact.policy {
            lettuce_companions::SoulFactPolicy::Current => dto::SoulFactPolicy::Current,
            lettuce_companions::SoulFactPolicy::Adaptive => dto::SoulFactPolicy::Adaptive,
            lettuce_companions::SoulFactPolicy::Historical => dto::SoulFactPolicy::Historical,
        },
        slot: fact.slot.clone(),
        confidence: fact.confidence,
        evidence_count: fact.evidence_count,
        weight: fact.weight,
        valid_from: fact.valid_from.get(),
        valid_until: fact.valid_until.map(TimestampMillis::get),
        locked: fact.locked,
        created_at: fact.created_at.get(),
        source_memory_ids: fact.source_memory_ids.clone(),
        supersedes: fact.supersedes.clone(),
        superseded_by: fact.superseded_by.clone(),
        superseded_at: fact.superseded_at.map(TimestampMillis::get),
    }
}

fn emotion_view(value: &lettuce_companions::EmotionVector) -> dto::CompanionEmotionVector {
    dto::CompanionEmotionVector {
        warmth: value.warmth,
        trust: value.trust,
        calm: value.calm,
        vulnerability: value.vulnerability,
        longing: value.longing,
        hurt: value.hurt,
        tension: value.tension,
        irritation: value.irritation,
        affection_intensity: value.affection_intensity,
        reassurance_need: value.reassurance_need,
    }
}

fn emotion_from(value: &dto::CompanionEmotionVector) -> lettuce_companions::EmotionVector {
    lettuce_companions::EmotionVector {
        warmth: value.warmth,
        trust: value.trust,
        calm: value.calm,
        vulnerability: value.vulnerability,
        longing: value.longing,
        hurt: value.hurt,
        tension: value.tension,
        irritation: value.irritation,
        affection_intensity: value.affection_intensity,
        reassurance_need: value.reassurance_need,
    }
}

fn regulation_view(value: &lettuce_companions::RegulationStyle) -> dto::CompanionRegulationStyle {
    dto::CompanionRegulationStyle {
        suppression: value.suppression,
        volatility: value.volatility,
        recovery_speed: value.recovery_speed,
        conflict_avoidance: value.conflict_avoidance,
        reassurance_seeking: value.reassurance_seeking,
        protest_behavior: value.protest_behavior,
        emotional_transparency: value.emotional_transparency,
        attachment_activation: value.attachment_activation,
        pride: value.pride,
    }
}

fn regulation_from(value: &dto::CompanionRegulationStyle) -> lettuce_companions::RegulationStyle {
    lettuce_companions::RegulationStyle {
        suppression: value.suppression,
        volatility: value.volatility,
        recovery_speed: value.recovery_speed,
        conflict_avoidance: value.conflict_avoidance,
        reassurance_seeking: value.reassurance_seeking,
        protest_behavior: value.protest_behavior,
        emotional_transparency: value.emotional_transparency,
        attachment_activation: value.attachment_activation,
        pride: value.pride,
    }
}

fn identity_view(value: &lettuce_companions::CompanionSoulIdentity) -> dto::CompanionSoulIdentity {
    dto::CompanionSoulIdentity {
        essence: value.essence.clone(),
        traits: value.traits.clone(),
        backstory: value.backstory.clone(),
        appearance: value.appearance.clone(),
        goals: value.goals.clone(),
        likes: value.likes.clone(),
        voice: value.voice.clone(),
        relational_style: value.relational_style.clone(),
        vulnerabilities: value.vulnerabilities.clone(),
        fears: value.fears.clone(),
        habits: value.habits.clone(),
        boundaries: value.boundaries.clone(),
        baseline_affect: emotion_view(&value.baseline_affect),
        regulation_style: regulation_view(&value.regulation_style),
    }
}

fn identity_from(value: &dto::CompanionSoulIdentity) -> lettuce_companions::CompanionSoulIdentity {
    lettuce_companions::CompanionSoulIdentity {
        essence: value.essence.clone(),
        traits: value.traits.clone(),
        backstory: value.backstory.clone(),
        appearance: value.appearance.clone(),
        goals: value.goals.clone(),
        likes: value.likes.clone(),
        voice: value.voice.clone(),
        relational_style: value.relational_style.clone(),
        vulnerabilities: value.vulnerabilities.clone(),
        fears: value.fears.clone(),
        habits: value.habits.clone(),
        boundaries: value.boundaries.clone(),
        baseline_affect: emotion_from(&value.baseline_affect),
        regulation_style: regulation_from(&value.regulation_style),
    }
}

fn defaults_view(
    value: &lettuce_companions::RelationshipDefaults,
) -> dto::CompanionRelationshipDefaults {
    dto::CompanionRelationshipDefaults {
        closeness: value.closeness,
        trust: value.trust,
        affection: value.affection,
        tension: value.tension,
    }
}

fn defaults_from(
    value: &dto::CompanionRelationshipDefaults,
) -> lettuce_companions::RelationshipDefaults {
    lettuce_companions::RelationshipDefaults {
        closeness: value.closeness,
        trust: value.trust,
        affection: value.affection,
        tension: value.tension,
    }
}

fn fact_from(view: &dto::SoulFactView) -> SoulFact {
    use lettuce_companions as c;
    SoulFact {
        id: view.id.clone(),
        category: match view.category {
            dto::SoulCategory::Essence => c::SoulCategory::Essence,
            dto::SoulCategory::Traits => c::SoulCategory::Traits,
            dto::SoulCategory::Backstory => c::SoulCategory::Backstory,
            dto::SoulCategory::Appearance => c::SoulCategory::Appearance,
            dto::SoulCategory::Goals => c::SoulCategory::Goals,
            dto::SoulCategory::Likes => c::SoulCategory::Likes,
            dto::SoulCategory::Voice => c::SoulCategory::Voice,
            dto::SoulCategory::RelationalStyle => c::SoulCategory::RelationalStyle,
            dto::SoulCategory::Vulnerabilities => c::SoulCategory::Vulnerabilities,
            dto::SoulCategory::Fears => c::SoulCategory::Fears,
            dto::SoulCategory::Habits => c::SoulCategory::Habits,
            dto::SoulCategory::Boundaries => c::SoulCategory::Boundaries,
        },
        value: view.value.clone(),
        kind: match view.kind {
            dto::SoulFactKind::Add => c::SoulFactKind::Add,
            dto::SoulFactKind::Adjust => c::SoulFactKind::Adjust,
            dto::SoulFactKind::Authored => c::SoulFactKind::Authored,
            dto::SoulFactKind::Consolidated => c::SoulFactKind::Consolidated,
        },
        policy: match view.policy {
            dto::SoulFactPolicy::Current => c::SoulFactPolicy::Current,
            dto::SoulFactPolicy::Adaptive => c::SoulFactPolicy::Adaptive,
            dto::SoulFactPolicy::Historical => c::SoulFactPolicy::Historical,
        },
        slot: view.slot.clone(),
        confidence: view.confidence,
        evidence_count: view.evidence_count,
        weight: view.weight,
        valid_from: TimestampMillis::new(view.valid_from),
        valid_until: view.valid_until.map(TimestampMillis::new),
        locked: view.locked,
        source_memory_ids: view.source_memory_ids.clone(),
        created_at: TimestampMillis::new(view.created_at),
        supersedes: view.supersedes.clone(),
        superseded_by: view.superseded_by.clone(),
        superseded_at: view.superseded_at.map(TimestampMillis::new),
    }
}

pub(crate) fn config_view(config: &CompanionSoulConfig) -> dto::CompanionSoulConfigView {
    dto::CompanionSoulConfigView {
        soul: identity_view(&config.soul),
        authored_facts: config.authored_facts.iter().map(fact_view).collect(),
        relationship_defaults: defaults_view(&config.relationship_defaults),
        prompting: dto::CompanionPrompting {
            prompt_template_id: config.prompting.prompt_template_id.map(|id| id.to_string()),
            style_notes: config.prompting.style_notes.clone(),
        },
        share_memory_across_chats: config.share_memory_across_chats,
        share_soul_growth_across_chats: config.share_soul_growth_across_chats,
    }
}

pub(crate) fn config_from(config: &dto::CompanionSoulConfigView) -> Result<CompanionSoulConfig, ApiError> {
    Ok(CompanionSoulConfig {
        soul: identity_from(&config.soul),
        authored_facts: config.authored_facts.iter().map(fact_from).collect(),
        relationship_defaults: defaults_from(&config.relationship_defaults),
        prompting: lettuce_companions::CompanionPromptingConfig {
            prompt_template_id: config.prompting.prompt_template_id.as_deref().map(|id| parse_id(id, "companion_soul.prompt_template_id")).transpose()?,
            style_notes: config.prompting.style_notes.clone(),
        },
        share_memory_across_chats: config.share_memory_across_chats,
        share_soul_growth_across_chats: config.share_soul_growth_across_chats,
    })
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DraftDocument {
    #[serde(default)]
    soul: lettuce_companions::CompanionSoulIdentity,
    #[serde(default)]
    authored_facts: Vec<SoulFact>,
    #[serde(default)]
    relationship_defaults: lettuce_companions::RelationshipDefaults,
}

/// The typed draft of a Soul writer document.
pub(crate) fn draft_from_document(
    document: serde_json::Value,
) -> Result<dto::CompanionSoulDraft, ApiError> {
    let document: DraftDocument = serde_json::from_value(document)
        .map_err(|_| api_error(ApiErrorCode::Internal, "the Soul draft is invalid"))?;
    Ok(dto::CompanionSoulDraft {
        soul: identity_view(&document.soul),
        authored_facts: document.authored_facts.iter().map(fact_view).collect(),
        relationship_defaults: defaults_view(&document.relationship_defaults),
    })
}

fn draft_document(draft: &dto::CompanionSoulDraft) -> Result<serde_json::Value, ApiError> {
    serde_json::to_value(DraftDocument {
        soul: identity_from(&draft.soul),
        authored_facts: draft.authored_facts.iter().map(fact_from).collect(),
        relationship_defaults: defaults_from(&draft.relationship_defaults),
    })
    .map_err(|_| {
        api_error(
            ApiErrorCode::Internal,
            "the Soul draft could not be encoded",
        )
    })
}

fn relationship_view(value: &RelationshipState) -> dto::CompanionRelationshipView {
    dto::CompanionRelationshipView {
        closeness: value.closeness,
        trust: value.trust,
        affection: value.affection,
        tension: value.tension,
        stability: value.stability,
        interaction_count: value.interaction_count,
        last_interaction_at: value.last_interaction_at.get(),
    }
}

fn state_view(snapshot: &CompanionStateSnapshot) -> dto::CompanionStateView {
    let state = &snapshot.state;
    let relationship = &state.relationship_state;
    let emotions = &state.emotional_state;
    dto::CompanionStateView {
        conversation_id: snapshot.owner.conversation_id.to_string(),
        character_id: snapshot.owner.character_id.to_string(),
        persona_id: snapshot.owner.persona_id.map(|id| id.to_string()),
        relationship: relationship_view(relationship),
        felt: emotion_view(&emotions.felt),
        expressed: emotion_view(&emotions.expressed),
        blocked: emotion_view(&emotions.blocked),
        momentum: emotion_view(&emotions.momentum),
        active_drivers: emotions.active_drivers.clone(),
        active_signals: state.active_signals.clone(),
        confidence: emotions.confidence,
        emotions_updated_at: emotions.updated_at.get(),
        updated_at: state.updated_at.get(),
    }
}

/// The current companion state of a direct chat, read for the persona the
/// chat uses now; `None` when the chat has no companion state.
pub async fn companion_state_get(
    context: &ApiContext,
    request: dto::CompanionStateGetRequest,
) -> Result<Option<dto::CompanionStateView>, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let aggregate = ConversationReader::get(database, conversation_id)
                .map_err(IntoApiError::into_api_error)?;
            if aggregate.conversation.lifecycle == ConversationLifecycle::Tombstoned {
                return Err(api_error(
                    ApiErrorCode::NotFound,
                    "the conversation was not found",
                ));
            }
            let Some(owner) = companion_state_owner(&aggregate.conversation) else {
                return Ok(None);
            };
            let snapshot = CompanionStateRepository::get(database, owner).map_err(|error| {
                api_error(
                    ApiErrorCode::Internal,
                    format!("the companion state could not be read: {error:?}"),
                )
            })?;
            Ok(snapshot.as_ref().map(state_view))
        })
        .await
}

fn live_conversation(
    database: &lettuce_database::Database,
    conversation_id: ConversationId,
) -> Result<Conversation, ApiError> {
    let conversation = ConversationReader::get(database, conversation_id)
        .map_err(IntoApiError::into_api_error)?
        .conversation;
    if conversation.lifecycle == ConversationLifecycle::Tombstoned {
        return Err(api_error(
            ApiErrorCode::NotFound,
            "the conversation was not found",
        ));
    }
    Ok(conversation)
}

fn character_participants(
    conversation: &Conversation,
) -> Vec<(CharacterId, &ConversationParticipant)> {
    let mut participants = conversation.participants.iter().collect::<Vec<_>>();
    participants.sort_by_key(|participant| participant.ordinal);
    participants
        .into_iter()
        .filter(|participant| participant.enabled)
        .filter_map(|participant| match participant.source {
            ParticipantSource::Character(character_id) => Some((character_id, participant)),
            _ => None,
        })
        .collect()
}

fn active_path(
    database: &lettuce_database::Database,
    conversation: &Conversation,
) -> Result<Vec<MessageId>, ApiError> {
    let mut cursor = None;
    let mut path = Vec::new();
    loop {
        let page = ConversationOverviewReader::timeline_page_after(
            database,
            conversation.id,
            conversation.active_branch_id,
            &PageRequest {
                cursor,
                limit: PageLimit::new(200),
            },
        )
        .map_err(IntoApiError::into_api_error)?;
        path.extend(
            page.items
                .iter()
                .filter(|item| {
                    item.message.visibility == MessageVisibility::Visible
                        && matches!(
                            item.message.role,
                            MessageRole::User | MessageRole::Assistant
                        )
                })
                .map(|item| item.message.id),
        );
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => return Ok(path),
        }
    }
}

fn group_repository_error(
    error: lettuce_companions::GroupCompanionRepositoryError,
) -> ApiError {
    api_error(
        ApiErrorCode::Internal,
        format!("the group companion state could not be read: {error:?}"),
    )
}

/// The companion state of every enabled member of a companion-mode group
/// chat, read for the persona the chat uses now; `None` for a chat that is
/// not a group or has companion mode off. A member without a checkpoint on
/// the active branch shows the initial state of its companion settings.
pub async fn group_companion_state_get(
    context: &ApiContext,
    request: dto::GroupCompanionStateGetRequest,
) -> Result<Option<dto::GroupCompanionStateView>, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let conversation = live_conversation(database, conversation_id)?;
            if !matches!(conversation.kind, ConversationKind::Group(_))
                || !crate::generation::live_sources::live_companion_mode(database, &conversation)
                    .map_err(IntoApiError::into_api_error)?
            {
                return Ok(None);
            }
            let path = active_path(database, &conversation)?;
            let checkpoints =
                GroupCompanionRepository::latest_checkpoints(database, conversation_id, &path)
                    .map_err(group_repository_error)?;
            let persona_id = lettuce_conversations::effective_persona(&conversation)
                .map(|persona| persona.source_id);
            let mut members = Vec::new();
            for (character_id, participant) in character_participants(&conversation) {
                let (_, config) = character_with_config(database, character_id)?;
                let checkpoint = checkpoints
                    .iter()
                    .find(|checkpoint| checkpoint.character_id == character_id);
                let (emotions, member_relationships) = match checkpoint {
                    Some(checkpoint) => (
                        checkpoint.emotional_state.clone(),
                        checkpoint.member_relationships.clone(),
                    ),
                    None => {
                        let initial = initial_group_member_state(&config);
                        (initial.emotional_state, initial.member_relationships)
                    }
                };
                let relationship = GroupCompanionRepository::persona_relationship(
                    database,
                    character_id,
                    persona_id,
                )
                .map_err(group_repository_error)?
                .map(|(_, state)| state)
                .unwrap_or_else(|| initial_group_relationship(&config));
                members.push(dto::GroupCompanionMemberView {
                    character_id: character_id.to_string(),
                    participant_id: participant.id.to_string(),
                    display_name: participant.display_name.clone(),
                    has_checkpoint: checkpoint.is_some(),
                    relationship: relationship_view(&relationship),
                    expressed: emotion_view(&emotions.expressed),
                    felt: emotion_view(&emotions.felt),
                    member_relationships: member_relationships
                        .iter()
                        .map(|stance| dto::GroupMemberRelationshipView {
                            target_character_id: stance.target_character_id.to_string(),
                            closeness: stance.state.closeness,
                            trust: stance.state.trust,
                            affection: stance.state.affection,
                            tension: stance.state.tension,
                        })
                        .collect(),
                });
            }
            Ok(Some(dto::GroupCompanionStateView { members }))
        })
        .await
}

fn emotion_change(value: &lettuce_companions::EmotionVector) -> dto::EmotionChange {
    dto::EmotionChange {
        warmth: value.warmth,
        trust: value.trust,
        calm: value.calm,
        vulnerability: value.vulnerability,
        longing: value.longing,
        hurt: value.hurt,
        tension: value.tension,
        irritation: value.irritation,
        affection_intensity: value.affection_intensity,
        reassurance_need: value.reassurance_need,
    }
}

/// What one message did to each member of a group chat: one row per member
/// whose checkpoint for the message holds at least one effect.
pub async fn group_companion_message_effects(
    context: &ApiContext,
    request: dto::GroupCompanionMessageEffectsRequest,
) -> Result<Vec<dto::GroupCompanionMemberEffectView>, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let message_id: MessageId = parse_id(&request.message_id, "message_id")?;
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let conversation = live_conversation(database, conversation_id)?;
            let checkpoints = GroupCompanionRepository::checkpoints_for_message(
                database,
                conversation_id,
                message_id,
            )
            .map_err(group_repository_error)?;
            Ok(checkpoints
                .into_iter()
                .filter(|checkpoint| !checkpoint.effects.is_empty())
                .map(|checkpoint| dto::GroupCompanionMemberEffectView {
                    character_id: checkpoint.character_id.to_string(),
                    display_name: conversation
                        .participants
                        .iter()
                        .find(|participant| {
                            participant.source
                                == ParticipantSource::Character(checkpoint.character_id)
                        })
                        .map(|participant| participant.display_name.clone())
                        .unwrap_or_default(),
                    effects: checkpoint
                        .effects
                        .iter()
                        .map(|effect| dto::GroupCompanionEffectView {
                            reason: match effect.reason {
                                GroupCompanionEffectReason::Witnessed => {
                                    dto::GroupCompanionEffectReason::Witnessed
                                }
                                GroupCompanionEffectReason::Addressed => {
                                    dto::GroupCompanionEffectReason::Addressed
                                }
                                GroupCompanionEffectReason::SocialEvent => {
                                    dto::GroupCompanionEffectReason::SocialEvent
                                }
                            },
                            social_event: effect.social_event.clone(),
                            target: effect.relationship_change.as_ref().map(|change| {
                                match change.target {
                                    GroupCompanionTarget::Persona { .. } => {
                                        dto::GroupCompanionEffectTarget::Persona
                                    }
                                    GroupCompanionTarget::Character { character_id } => {
                                        dto::GroupCompanionEffectTarget::Character {
                                            character_id: character_id.to_string(),
                                        }
                                    }
                                }
                            }),
                            relationship: effect.relationship_change.as_ref().map(|change| {
                                dto::RelationshipChange {
                                    closeness: change.delta.closeness,
                                    trust: change.delta.trust,
                                    affection: change.delta.affection,
                                    tension: change.delta.tension,
                                    stability: change.delta.stability,
                                }
                            }),
                            felt: emotion_change(&effect.emotion_delta.felt),
                        })
                        .collect(),
                })
                .collect())
        })
        .await
}

/// The companion's authored Soul configuration together with the growth of
/// the Soul in effect: the conversation's own while its character does not
/// share Soul growth, otherwise the character's.
pub async fn companion_soul_get(
    context: &ApiContext,
    request: dto::CompanionSoulGetRequest,
) -> Result<dto::CompanionSoulView, ApiError> {
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let target = target(
                database,
                &request.character_id,
                request.conversation_id.as_deref(),
            )?;
            let state = SoulRepository::get(database, target.owner).map_err(soul_error)?;
            let facts = state
                .as_ref()
                .map(|state| state.facts.iter().map(fact_view).collect::<Vec<_>>())
                .unwrap_or_default();
            let superseded = facts
                .iter()
                .filter(|fact| fact.superseded_by.is_some())
                .count();
            Ok(dto::CompanionSoulView {
                character_id: target.character_id.to_string(),
                conversation_id: target.conversation_id.map(|id| id.to_string()),
                config: config_view(&target.config),
                growth: dto::SoulGrowthView {
                    owner: match target.owner {
                        SoulOwner::Character(_) => dto::SoulOwnerKind::Character,
                        SoulOwner::Conversation { .. } => dto::SoulOwnerKind::Conversation,
                    },
                    revision: state.as_ref().map_or(0, |state| state.revision.get()),
                    active_count: u32::try_from(facts.len() - superseded).unwrap_or(u32::MAX),
                    superseded_count: u32::try_from(superseded).unwrap_or(u32::MAX),
                    facts,
                },
            })
        })
        .await
}

/// Removes every growth entry, locked ones included, and answers how many
/// there were.
pub async fn companion_soul_growth_clear(
    context: &ApiContext,
    request: dto::CompanionSoulGrowthClearRequest,
) -> Result<u32, ApiError> {
    let key = request.client_operation_id.clone();
    let (character, conversation) = (
        request.character_id.clone(),
        request.conversation_id.clone(),
    );
    let at = context.now();
    controlled_atomic(
        context,
        "companion_soul_growth_clear",
        &key,
        &request,
        move |context| target(context.backend().database(), &character, conversation.as_deref()).map(|target| target.owner),
        move |scope, owner| {
            let state = scope.edit_soul_growth(owner, lettuce_companions::SoulUserEdit::ClearAll, at)
                .map_err(|error| EditFailure(soul_error(error)))?;
            Ok(state.map_or(0, |state| u32::try_from(state.facts.len()).unwrap_or(u32::MAX)))
        },
    )
    .await
}

/// Removes one growth entry by its id, even a locked one; false when there
/// is none with that id.
pub async fn companion_soul_growth_remove(
    context: &ApiContext,
    request: dto::CompanionSoulGrowthRemoveRequest,
) -> Result<bool, ApiError> {
    let key = request.client_operation_id.clone();
    let (character, conversation, fact) = (
        request.character_id.clone(),
        request.conversation_id.clone(),
        request.fact_id.clone(),
    );
    let at = context.now();
    controlled_atomic(
        context,
        "companion_soul_growth_remove",
        &key,
        &request,
        move |context| target(context.backend().database(), &character, conversation.as_deref()).map(|target| target.owner),
        move |scope, owner| {
            let state = scope.edit_soul_growth(owner, lettuce_companions::SoulUserEdit::Remove { fact_id: fact.clone() }, at)
                .map_err(|error| EditFailure(soul_error(error)))?;
            Ok(state.is_some_and(|state| state.facts.iter().any(|entry| entry.id == fact)))
        },
    )
    .await
}

/// Locks or unlocks one growth entry; true when the entry exists, whether or
/// not the lock changed.
pub async fn companion_soul_growth_lock(
    context: &ApiContext,
    request: dto::CompanionSoulGrowthLockRequest,
) -> Result<bool, ApiError> {
    let key = request.client_operation_id.clone();
    let (character, conversation, fact, locked) = (
        request.character_id.clone(),
        request.conversation_id.clone(),
        request.fact_id.clone(),
        request.locked,
    );
    let at = context.now();
    controlled_atomic(
        context,
        "companion_soul_growth_lock",
        &key,
        &request,
        move |context| target(context.backend().database(), &character, conversation.as_deref()).map(|target| target.owner),
        move |scope, owner| {
            let state = scope.edit_soul_growth(owner, lettuce_companions::SoulUserEdit::SetLocked { fact_id: fact.clone(), locked }, at)
                .map_err(|error| EditFailure(soul_error(error)))?;
            Ok(state.is_some_and(|state| state.facts.iter().any(|entry| entry.id == fact)))
        },
    )
    .await
}

fn writer_profile(
    database: &lettuce_database::Database,
    model_profile_id: ModelProfileId,
) -> Result<lettuce_conversations::ResolvedInferenceProfile, ApiError> {
    let model = lettuce_models::ModelProfileRepository::get(database, model_profile_id)
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
        .ok_or_else(|| invalid_field("model_profile_id", "the model was not found"))?;
    let account =
        lettuce_models::ProviderAccountRepository::get(database, model.provider_account_id)
            .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
            .ok_or_else(|| {
                invalid_field("model_profile_id", "the model's account was not found")
            })?;
    let global = lettuce_models::GlobalModelSettingsRepository::global_model_settings(database)
        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
        .0;
    let chat_profile = lettuce_models::resolve_chat_profile(
        &lettuce_models::ExpectedModelIdentity {
            model_profile_id: model.id,
            model_revision: model.revision,
            provider_account_id: account.id,
            provider_account_revision: account.revision,
            external_model_id: model.external_model_id.clone(),
            display_name: model.display_name.clone(),
            provider_protocol: account.protocol,
            model_kind: model.kind,
        },
        &model,
        &account,
        &crate::feature_parameter_input(
            &model.config.feature_parameters.companion_soul_writer,
            crate::COMPANION_SOUL_WRITER_DEFAULTS,
            crate::FeatureRequestFields::Sampling,
            account.protocol,
            &global,
        ),
        &lettuce_models::ChatRequirements::default(),
    )
    .map_err(|_| invalid_field("model_profile_id", "the model cannot write a Soul"))?;
    Ok(lettuce_conversations::ResolvedInferenceProfile {
        chat_profile,
        tool_policy: lettuce_conversations::ToolPolicy::Required,
        output_policy: lettuce_conversations::OutputPolicy::Plain,
        safety_policy: lettuce_conversations::SafetyContext::Standard,
        correlation_id: None,
    })
}

/// Queues a Soul writer run over the unsaved draft. The model is the chosen
/// one, then the configured Soul writer model, then the default model; the
/// configured fallback model runs only when it differs. The draft arrives as
/// the job's result and nothing is saved.
pub async fn companion_soul_writer_run(
    context: &ApiContext,
    request: dto::CompanionSoulWriterRunRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let key = request.client_operation_id.clone();
    let action_key = key.clone();
    let action_request = request.clone();
    let accepted = controlled_atomic(
        context,
        "companion_soul_writer_run",
        &key,
        &request,
        move |context| {
            let request = action_request;
            if request.character_name.trim().is_empty() {
                return Err(invalid_field("character_name", "the character needs a name"));
            }
            let database = context.backend().database();
            let current_soul = request
                .current_soul
                .as_ref()
                .map(draft_document)
                .transpose()?;
            let stored = GlobalSettingsStore::load(database)
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
            let configured = stored.settings.companion_soul_writer;
            let chosen: Option<ModelProfileId> = request
                .model_profile_id
                .as_deref()
                .map(|value| parse_id(value, "model_profile_id"))
                .transpose()?;
            let primary_id = chosen
                .or(configured.model_profile_id)
                .or(stored.default_model_profile_id)
                .ok_or_else(|| {
                    invalid_field(
                        "model_profile_id",
                        "no model is available for the Soul writer",
                    )
                })?;
            let primary = writer_profile(database, primary_id)?;
            let fallback = configured
                .fallback_model_profile_id
                .filter(|id| *id != primary_id)
                .and_then(|id| match writer_profile(database, id) {
                    Ok(profile) => Some(profile),
                    Err(error) => {
                        tracing::warn!(reason = %error.message, "the Soul writer fallback model cannot run");
                        None
                    }
                });
            let prompt = match configured.prompt_id.and_then(|id| {
                lettuce_context::PromptRepository::get(database, id)
                    .ok()
                    .flatten()
                    .filter(|document| {
                        document.status == lettuce_context::LifecycleStatus::Active
                            && document.purpose
                                == lettuce_context::PromptPurpose::CompanionSoulWriter
                    })
            }) {
                Some(prompt) => prompt,
                None => crate::generation::built_in_prompts::active_built_in_prompt(
                    database,
                    crate::BuiltInPromptId::CompanionSoulWriter,
                )
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
                .ok_or_else(|| {
                    api_error(ApiErrorCode::Internal, "the Soul writer prompt is missing")
                })?,
            };
            let request_id =
                RequestId::from_uuid(stable_uuid(&["companion-soul-writer", &action_key]));
            let plan = context
                .backend()
                .companion_soul_writer_admission()
                .prepare(crate::CompanionSoulWriterAdmissionRequest {
                    request_id,
                    primary_profile: primary,
                    fallback_profile: fallback,
                    prompt: &prompt,
                    character_name: request.character_name.trim(),
                    character_definition: request.character_definition.as_deref(),
                    character_description: request.character_description.as_deref(),
                    opening_context: request.opening_context.as_deref(),
                    current_soul: current_soul.as_ref(),
                    user_notes: request.user_notes.as_deref(),
                    fallback_format: match configured.structured_fallback_format {
                        lettuce_settings::MemoryStructuredFallbackFormat::Json => {
                            lettuce_companions::SoulWriterFallbackFormat::Json
                        }
                        lettuce_settings::MemoryStructuredFallbackFormat::Xml => {
                            lettuce_companions::SoulWriterFallbackFormat::Xml
                        }
                    },
                    now: context.now(),
                })
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
            Ok(plan)
        },
        move |scope, (spec, run)| {
            scope.admit_soul_writer(spec, run)
                .map(|job| dto::JobAccepted { job_id: job.id.to_string() })
                .map_err(EditFailure::from)
        },
    )
    .await?;
    context.jobs().wake();
    Ok(accepted)
}
