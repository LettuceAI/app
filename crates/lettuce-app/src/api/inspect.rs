//! Reads that explain a chat to its user: the request a reply was
//! generated from, the prompt the next speaker selection would send, who
//! spoke how much, and what a reply changed about a companion.

use std::collections::HashMap;

use lettuce_companions::{CompanionTurnEffect, CompanionTurnEffectRepository, EmotionVector};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_conversations::{
    ContextSectionKind, ConversationOverviewReader, ConversationReader, GenerationOperation,
    InitialInferenceRepository, MessageRenderSource, MessageRole, MessageVisibility,
    ParticipantRole, ParticipantSource, ProviderContextPart,
};
use lettuce_types::{ConversationId, ConversationParticipantId, MessageId, PageLimit, PageRequest};

use super::ApiContext;
use super::error::{IntoApiError, api_error, invalid_field, parse_id};
use super::mapping;
use super::messages::{active_branch, on_timeline};

fn conversation_of(
    context: &ApiContext,
    message_id: MessageId,
) -> Result<ConversationId, ApiError> {
    ConversationOverviewReader::conversation_of_message(context.backend().database(), message_id)
        .map_err(IntoApiError::into_api_error)?
        .ok_or_else(|| api_error(ApiErrorCode::NotFound, "the message was not found"))
}

const fn operation(operation: GenerationOperation) -> dto::PromptOperation {
    match operation {
        GenerationOperation::Send => dto::PromptOperation::Send,
        GenerationOperation::Continue => dto::PromptOperation::Continue,
        GenerationOperation::Regenerate => dto::PromptOperation::Regenerate,
    }
}

const fn section_kind(kind: ContextSectionKind) -> dto::PromptSectionKind {
    match kind {
        ContextSectionKind::Character => dto::PromptSectionKind::Character,
        ContextSectionKind::Persona => dto::PromptSectionKind::Persona,
        ContextSectionKind::Scene => dto::PromptSectionKind::Scene,
        ContextSectionKind::Lorebook => dto::PromptSectionKind::Lorebook,
        ContextSectionKind::Memories => dto::PromptSectionKind::Memories,
        ContextSectionKind::AuthorNote => dto::PromptSectionKind::AuthorNote,
        ContextSectionKind::CompanionState => dto::PromptSectionKind::CompanionState,
        ContextSectionKind::ScheduledNotes => dto::PromptSectionKind::ScheduledNotes,
        ContextSectionKind::GroupCast => dto::PromptSectionKind::GroupCast,
        ContextSectionKind::PromptEntry => dto::PromptSectionKind::PromptEntry,
    }
}

fn prompt_part(context: &ApiContext, part: &ProviderContextPart) -> dto::PromptPart {
    match part {
        ProviderContextPart::Text { text } => dto::PromptPart::Text { text: text.clone() },
        ProviderContextPart::MediaAsset { asset_id, role } => dto::PromptPart::Media {
            asset: context.asset_ref(*asset_id),
            role: mapping::media_role(*role),
        },
        ProviderContextPart::ToolCall(call) => dto::PromptPart::ToolCall {
            name: call.name.clone(),
            arguments: call.arguments.to_string(),
        },
        ProviderContextPart::ToolResult(result) => dto::PromptPart::ToolResult {
            name: result.name.clone(),
            output: result.output.value.to_string(),
        },
    }
}

pub(super) fn snapshot_view(
    context: &ApiContext,
    turn_id: lettuce_types::GenerationTurnId,
    candidate: &lettuce_conversations::MessageCandidate,
    record: &lettuce_conversations::InitialInferenceRecord,
) -> dto::PromptSnapshot {
    let profile = &record.request.profile.chat_profile;
    let parameters = &profile.parameters;
    let budget = &record.request.context.budget;
    let sections = record.request.context.attributions.sections.as_ref();
    dto::PromptSnapshot {
        turn_id: turn_id.to_string(),
        candidate_id: candidate.id.to_string(),
        operation: operation(record.request.operation),
        model: dto::PromptModel {
            display_name: candidate.model.display_name.clone(),
            external_model_id: profile.external_model_id.clone(),
            provider_kind: profile.provider_kind.clone(),
        },
        streaming: profile.streaming_enabled,
        parameters: dto::PromptParameters {
            temperature: parameters.temperature,
            top_p: parameters.top_p,
            top_k: parameters.top_k,
            max_output_tokens: parameters.visible_max_output_tokens,
            context_length: parameters.context_length,
            frequency_penalty: parameters.frequency_penalty,
            presence_penalty: parameters.presence_penalty,
            repetition_penalty: parameters.repetition_penalty,
        },
        messages: record
            .request
            .context
            .messages
            .iter()
            .map(|message| dto::PromptMessage {
                role: mapping::message_role(message.role),
                parts: message
                    .parts
                    .iter()
                    .map(|part| prompt_part(context, part))
                    .collect(),
            })
            .collect(),
        budget: dto::PromptBudget {
            selected_messages: budget.selected_messages,
            omitted_messages: budget.omitted_messages,
            input_bytes: budget.input_bytes,
            estimated_input_tokens: budget.estimated_input_tokens,
            truncated: budget.truncated,
        },
        sections: sections.map(|sections| {
            sections
                .iter()
                .map(|section| dto::PromptSection {
                    kind: section_kind(section.kind),
                    label: section.label.clone(),
                    estimated_tokens: section.estimated_tokens,
                })
                .collect()
        }),
        sections_unavailable: sections
            .is_none()
            .then_some(dto::PromptSectionsUnavailable::PredatesBreakdown),
    }
}

/// The request a reply was generated from, as recorded when it was sent:
/// the messages, the resolved sampling parameters and, for a request
/// recorded with them, what each source contributed. A reply whose request
/// was never recorded (imported ones) is `NotFound`. The reply shown, or the
/// variant an edit of it supersedes, decides which request it is.
pub async fn message_prompt_snapshot(
    context: &ApiContext,
    request: dto::MessagePromptSnapshotRequest,
) -> Result<dto::PromptSnapshot, ApiError> {
    let message_id: MessageId = parse_id(&request.message_id, "message_id")?;
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let conversation_id = conversation_of(context, message_id)?;
            let branch_id = active_branch(context, conversation_id)?;
            let item = on_timeline(context, conversation_id, branch_id, message_id)?.item;
            if item.message.role != MessageRole::Assistant {
                return Err(invalid_field("message_id", "the message is not a reply"));
            }
            let candidate_id = match (&item.message.active_render_source, &item.active_revision) {
                (MessageRenderSource::Candidate(id), _) => Some(*id),
                (MessageRenderSource::Revision(_), Some(revision)) => {
                    revision.supersedes_candidate_id
                }
                (MessageRenderSource::Revision(_), None) => None,
            }
            .ok_or_else(|| {
                api_error(
                    ApiErrorCode::NotFound,
                    "the reply has no generated variant with a recorded request",
                )
            })?;
            let candidate = ConversationReader::get_candidate(database, candidate_id)
                .map_err(IntoApiError::into_api_error)?;
            let turn = ConversationReader::get_turn(database, candidate.turn_id)
                .map_err(IntoApiError::into_api_error)?;
            let job_id = turn
                .attempts
                .iter()
                .find(|attempt| attempt.id == candidate.attempt_id)
                .and_then(|attempt| attempt.job_id)
                .ok_or_else(|| {
                    api_error(
                        ApiErrorCode::NotFound,
                        "the reply's request was never recorded",
                    )
                })?;
            let record = InitialInferenceRepository::initial_inference_for_attempt(
                database,
                conversation_id,
                turn.id,
                candidate.attempt_id,
                job_id,
            )
            .map_err(IntoApiError::into_api_error)?
            .ok_or_else(|| {
                api_error(
                    ApiErrorCode::NotFound,
                    "the reply's request was never recorded",
                )
            })?;
            Ok(snapshot_view(context, turn.id, &candidate, &record))
        })
        .await
}

/// The prompt the chat's next LLM speaker selection would send, rendered
/// from the live cast and the selected branch, plus `user_message` when the
/// caller asks about one that is not sent yet. Nothing is sent to a
/// provider. A one-to-one chat has no speaker selection (`Unsupported`), and
/// a group without a member that can speak is `Conflict`.
pub async fn conversation_speaker_selection_preview(
    context: &ApiContext,
    request: dto::SpeakerSelectionPreviewRequest,
) -> Result<dto::SpeakerSelectionPreview, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    context
        .blocking(move |context| {
            let backend = context.backend();
            let embedding = context.embedding();
            let runner = backend.prepared_conversation_generation_runner(
                embedding.as_ref(),
                context.inference(),
                &super::worker::UnstoredReplyMedia,
            );
            let prompt = runner
                .speaker_selection_preview(
                    conversation_id,
                    request.user_message.as_deref(),
                    context.now(),
                )
                .map_err(|error| match error {
                    crate::ConversationGenerationInputError::Repository(error) => {
                        error.into_api_error()
                    }
                    crate::ConversationGenerationInputError::SpeakerUnavailable => {
                        api_error(ApiErrorCode::Conflict, "no member of the group can speak")
                    }
                    error => api_error(ApiErrorCode::Internal, format!("{error:?}")),
                })?
                .ok_or_else(|| {
                    api_error(
                        ApiErrorCode::Unsupported,
                        "only a group chat selects its speaker",
                    )
                })?;
            Ok(dto::SpeakerSelectionPreview { prompt })
        })
        .await
}

/// Each character's replies on the selected branch: how many of the visible
/// replies it spoke, the share, and its newest one. Counted from the
/// timeline as it is now, so deleting, regenerating with another speaker or
/// switching branch changes it; nothing is counted separately.
pub async fn conversation_participation_stats(
    context: &ApiContext,
    request: dto::ConversationRequest,
) -> Result<dto::ParticipationStats, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let conversation = ConversationReader::get(database, conversation_id)
                .map_err(IntoApiError::into_api_error)?
                .conversation;
            let mut counts: HashMap<ConversationParticipantId, u64> = HashMap::new();
            let mut newest: HashMap<ConversationParticipantId, (MessageId, i64)> = HashMap::new();
            let mut cursor = None;
            loop {
                let page = ConversationReader::timeline_page(
                    database,
                    conversation_id,
                    conversation.active_branch_id,
                    &PageRequest {
                        cursor,
                        limit: PageLimit::new(200),
                    },
                )
                .map_err(IntoApiError::into_api_error)?;
                for item in &page.items {
                    let message = &item.message;
                    let Some(author) = message.author_participant_id else {
                        continue;
                    };
                    if message.role != MessageRole::Assistant
                        || message.visibility != MessageVisibility::Visible
                    {
                        continue;
                    }
                    *counts.entry(author).or_default() += 1;
                    newest
                        .entry(author)
                        .or_insert((message.id, message.created_at.get()));
                }
                match page.next_cursor {
                    Some(next) => cursor = Some(next),
                    None => break,
                }
            }
            let total = counts.values().sum::<u64>();
            let items = conversation
                .participants
                .iter()
                .filter(|participant| participant.role == ParticipantRole::Character)
                .map(|participant| {
                    let count = counts.get(&participant.id).copied().unwrap_or_default();
                    let last = newest.get(&participant.id);
                    dto::ParticipationStat {
                        participant_id: participant.id.to_string(),
                        character_id: match participant.source {
                            ParticipantSource::Character(id) => Some(id.to_string()),
                            _ => None,
                        },
                        name: participant.display_name.clone(),
                        enabled: participant.enabled,
                        muted: participant.muted,
                        message_count: count,
                        percent: if total == 0 {
                            0
                        } else {
                            u32::try_from((count as f64 / total as f64 * 100.0).round() as u64)
                                .unwrap_or(100)
                        },
                        last_spoke_message_id: last.map(|(id, _)| id.to_string()),
                        last_spoke_at: last.map(|(_, at)| *at),
                    }
                })
                .collect();
            Ok(dto::ParticipationStats {
                items,
                total_messages: total,
            })
        })
        .await
}

fn emotion_change(vector: &EmotionVector) -> dto::EmotionChange {
    dto::EmotionChange {
        warmth: vector.warmth,
        trust: vector.trust,
        calm: vector.calm,
        vulnerability: vector.vulnerability,
        longing: vector.longing,
        hurt: vector.hurt,
        tension: vector.tension,
        irritation: vector.irritation,
        affection_intensity: vector.affection_intensity,
        reassurance_need: vector.reassurance_need,
    }
}

fn effect_view(effect: &CompanionTurnEffect) -> dto::MessageCompanionEffect {
    let ids = |ids: &[lettuce_types::MemoryId]| ids.iter().map(ToString::to_string).collect();
    dto::MessageCompanionEffect {
        status: match effect.status {
            lettuce_companions::CompanionTurnEffectStatus::Processing => {
                dto::CompanionEffectStatus::Processing
            }
            lettuce_companions::CompanionTurnEffectStatus::Ready => {
                dto::CompanionEffectStatus::Ready
            }
            lettuce_companions::CompanionTurnEffectStatus::Failed => {
                dto::CompanionEffectStatus::Failed
            }
            lettuce_companions::CompanionTurnEffectStatus::Invalidated => {
                dto::CompanionEffectStatus::Invalidated
            }
        },
        summary: effect.summary.clone(),
        relationship: dto::RelationshipChange {
            closeness: effect.seed.relationship_delta.closeness,
            trust: effect.seed.relationship_delta.trust,
            affection: effect.seed.relationship_delta.affection,
            tension: effect.seed.relationship_delta.tension,
            stability: effect.seed.relationship_delta.stability,
        },
        felt: emotion_change(&effect.seed.emotion_delta.felt),
        expressed: emotion_change(&effect.seed.emotion_delta.expressed),
        blocked: emotion_change(&effect.seed.emotion_delta.blocked),
        signals_added: effect.seed.signal_changes.added.clone(),
        signals_removed: effect.seed.signal_changes.removed.clone(),
        memories_added: ids(&effect.memory_changes.added),
        memories_updated: ids(&effect.memory_changes.updated),
        memories_superseded: ids(&effect.memory_changes.superseded),
    }
}

/// What a companion chat's reply changed: none for a message without an
/// effect. An effect that is still processing settles later, and
/// `ApiEvent::MessageEffectSettled` says when to read it again.
pub async fn message_companion_effect(
    context: &ApiContext,
    request: dto::MessageCompanionEffectRequest,
) -> Result<Option<dto::MessageCompanionEffect>, ApiError> {
    let message_id: MessageId = parse_id(&request.message_id, "message_id")?;
    context
        .blocking(move |context| {
            let conversation_id = conversation_of(context, message_id)?;
            let effect = CompanionTurnEffectRepository::get_for_message(
                context.backend().database(),
                conversation_id,
                message_id,
            )
            .map_err(|_| {
                api_error(
                    ApiErrorCode::Internal,
                    "the companion effect could not be read",
                )
            })?;
            Ok(effect.as_ref().map(effect_view))
        })
        .await
}
