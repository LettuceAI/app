use std::collections::BTreeMap;

use lettuce_characters::{CharacterRepository, GroupRepository};
use lettuce_companions::{
    CompanionStateRepository, GroupCompanionAuthor, GroupCompanionEvent, GroupCompanionMember,
    GroupCompanionMemberState, GroupCompanionMessageSource, GroupCompanionRepository,
    GroupCompanionRepositoryError, GroupCompanionUserEvent, PreparedGroupCompanionTurn,
    group_audience_away, reduce_group_event,
};
use lettuce_conversations::{
    ConversationAggregate, ConversationKind, ConversationOverviewReader, ConversationParticipant,
    ConversationRepositoryError, MessagePart, MessageRole, MessageVisibility, ParticipantSource,
    TimelineItem,
};
use lettuce_jobs::handle::CancellationToken;
use lettuce_types::{CharacterId, MessageId, PageLimit, PageRequest, PersonaId, TimestampMillis};

use super::companion_turn::{CompanionTurnError, classification_signals, classification_text};
use crate::CompanionEmotionEngine;

pub(crate) fn prepare_group_companion_turn<S, E>(
    sources: &S,
    emotion: Option<&E>,
    aggregate: &ConversationAggregate,
    user_text: Option<&str>,
    now: TimestampMillis,
    cancellation: &CancellationToken,
) -> Result<Option<PreparedGroupCompanionTurn>, CompanionTurnError>
where
    S: CharacterRepository
        + GroupRepository
        + ConversationOverviewReader
        + GroupCompanionRepository
        + CompanionStateRepository,
    E: CompanionEmotionEngine + ?Sized,
{
    if !matches!(aggregate.conversation.kind, ConversationKind::Group(_))
        || !crate::generation::live_sources::live_companion_mode(sources, &aggregate.conversation)?
    {
        return Ok(None);
    }
    if cancellation.is_cancelled() {
        return Err(CompanionTurnError::Cancelled);
    }
    let conversation_id = aggregate.conversation.id;
    let persona_id = lettuce_conversations::effective_persona(&aggregate.conversation)
        .map(|persona| persona.source_id);
    let mut members = Vec::new();
    let mut member_names = Vec::new();
    let mut participants = aggregate
        .conversation
        .participants
        .iter()
        .collect::<Vec<_>>();
    participants.sort_by_key(|participant| participant.ordinal);
    for participant in participants {
        if !participant.enabled {
            continue;
        }
        let ParticipantSource::Character(character_id) = participant.source else {
            continue;
        };
        let Some(details) = CharacterRepository::get(sources, character_id)? else {
            continue;
        };
        let config = details
            .character
            .defaults
            .companion_soul
            .clone()
            .unwrap_or_default();
        member_names.push((character_id, participant.display_name.clone()));
        members.push((character_id, config));
    }
    let path = read_path(sources, aggregate)?;
    let path_ids = path.iter().map(|item| item.message.id).collect::<Vec<_>>();
    let checkpoint_sources = sources
        .path_checkpoint_sources(conversation_id, &path_ids)
        .map_err(repository_error)?
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    let first_checkpointed = path
        .iter()
        .position(|item| checkpoint_sources.contains_key(&item.message.id));
    let pending_start = match first_checkpointed {
        None => path.len(),
        Some(first) => (first..path.len())
            .find(
                |index| match checkpoint_sources.get(&path[*index].message.id) {
                    None => true,
                    Some(stored) => *stored != item_source(&path[*index]),
                },
            )
            .unwrap_or(path.len()),
    };
    let before_ids = path[..pending_start]
        .iter()
        .map(|item| item.message.id)
        .collect::<Vec<_>>();
    let mut states = BTreeMap::new();
    for checkpoint in sources
        .latest_checkpoints(conversation_id, &before_ids)
        .map_err(repository_error)?
    {
        states.insert(
            checkpoint.character_id,
            GroupCompanionMemberState {
                emotional_state: checkpoint.emotional_state,
                member_relationships: checkpoint.member_relationships,
            },
        );
    }
    let mut persona_relationships = BTreeMap::new();
    let mut persona_revisions = BTreeMap::new();
    for (character_id, _) in &members {
        match sources
            .persona_relationship(*character_id, persona_id)
            .map_err(repository_error)?
        {
            Some((revision, state)) => {
                persona_relationships.insert(*character_id, state);
                persona_revisions.insert(*character_id, Some(revision));
            }
            None => {
                persona_revisions.insert(*character_id, None);
            }
        }
    }
    let member_views = members
        .iter()
        .map(|(character_id, config)| GroupCompanionMember {
            character_id: *character_id,
            config,
        })
        .collect::<Vec<_>>();
    let authors = path
        .iter()
        .map(|item| author_of(item, &aggregate.conversation.participants, persona_id))
        .collect::<Vec<_>>();
    let author_map = authors
        .iter()
        .enumerate()
        .filter_map(|(index, author)| match author {
            GroupCompanionAuthor::Member { character_id } => {
                Some((path[index].message.id, *character_id))
            }
            GroupCompanionAuthor::Persona { .. } => None,
        })
        .collect::<BTreeMap<_, _>>();
    let branch_audience = sources
        .branch_audience(conversation_id, aggregate.conversation.active_branch_id)
        .map_err(repository_error)?;
    let pending_ids = path[pending_start..]
        .iter()
        .map(|item| item.message.id)
        .collect::<Vec<_>>();
    let mut recorded_by_message = BTreeMap::<MessageId, Vec<CharacterId>>::new();
    for (message_id, character_id) in sources
        .recorded_persona_applications(conversation_id, &pending_ids, persona_id)
        .map_err(repository_error)?
    {
        recorded_by_message
            .entry(message_id)
            .or_default()
            .push(character_id);
    }
    let mut applied = Vec::new();
    for index in pending_start..path.len() {
        if cancellation.is_cancelled() {
            return Err(CompanionTurnError::Cancelled);
        }
        let item = &path[index];
        let text = classification_text(item_parts(item));
        let author = authors[index];
        let away = group_audience_away(
            &path_ids[..index],
            &author_map,
            &branch_audience.checkpoints,
        );
        let mut present = members
            .iter()
            .map(|(character_id, _)| *character_id)
            .filter(|character_id| !away.contains(character_id))
            .collect::<Vec<_>>();
        if let GroupCompanionAuthor::Member { character_id } = author
            && members.iter().any(|(member, _)| *member == character_id)
            && !present.contains(&character_id)
        {
            present.push(character_id);
        }
        let addressed = addressed_members(
            &text,
            &member_names,
            &present,
            author,
            index.checked_sub(1).map(|previous| authors[previous]),
        );
        let signals = classification_signals(emotion, &text, cancellation, conversation_id)?;
        let event = GroupCompanionEvent {
            message_id: item.message.id,
            source: item_source(item),
            author,
            present: present.clone(),
            addressed,
            signals,
            at: item.message.effective_time,
        };
        let preserved_persona_relationships = recorded_by_message
            .get(&item.message.id)
            .into_iter()
            .flatten()
            .map(|character_id| {
                (
                    *character_id,
                    persona_relationships.get(character_id).cloned(),
                )
            })
            .collect::<Vec<_>>();
        let step = reduce_group_event(
            &member_views,
            &mut states,
            &mut persona_relationships,
            &event,
        );
        for (character_id, state) in preserved_persona_relationships {
            match state {
                Some(state) => {
                    persona_relationships.insert(character_id, state);
                }
                None => {
                    persona_relationships.remove(&character_id);
                }
            }
        }
        applied.push((item.message.id, step));
    }
    let user_event = match user_text {
        Some(text) => {
            if cancellation.is_cancelled() {
                return Err(CompanionTurnError::Cancelled);
            }
            let effective_now = crate::companion::companion_clock::conversation_clock_context(
                sources,
                &aggregate.conversation,
            )
            .map_err(|_| CompanionTurnError::CharacterMissing)?
            .effective_now(now);
            let author = GroupCompanionAuthor::Persona { persona_id };
            let away = group_audience_away(&path_ids, &author_map, &branch_audience.checkpoints);
            let present = members
                .iter()
                .map(|(character_id, _)| *character_id)
                .filter(|character_id| !away.contains(character_id))
                .collect::<Vec<_>>();
            let addressed = addressed_members(text, &member_names, &present, author, None);
            Some(GroupCompanionUserEvent {
                present,
                addressed,
                signals: classification_signals(emotion, text, cancellation, conversation_id)?,
                at: effective_now,
            })
        }
        None => None,
    };
    Ok(Some(PreparedGroupCompanionTurn {
        conversation_id,
        persona_id,
        members,
        applied,
        states,
        persona_relationships,
        persona_revisions,
        user_event,
    }))
}

fn read_path<S>(
    sources: &S,
    aggregate: &ConversationAggregate,
) -> Result<Vec<TimelineItem>, CompanionTurnError>
where
    S: ConversationOverviewReader + ?Sized,
{
    let conversation_id = aggregate.conversation.id;
    let branch_id = aggregate.conversation.active_branch_id;
    let mut cursor = None;
    let mut items = Vec::new();
    loop {
        let page = ConversationOverviewReader::timeline_page_after(
            sources,
            conversation_id,
            branch_id,
            &PageRequest {
                cursor: cursor.clone(),
                limit: PageLimit::new(200),
            },
        )?;
        items.extend(page.items);
        let Some(next) = page.next_cursor else {
            break;
        };
        cursor = Some(next);
    }
    Ok(items
        .into_iter()
        .filter(|item| {
            item.message.visibility == MessageVisibility::Visible
                && matches!(
                    item.message.role,
                    MessageRole::User | MessageRole::Assistant
                )
        })
        .collect())
}

fn item_parts(item: &TimelineItem) -> &[MessagePart] {
    if let Some(revision) = &item.active_revision {
        &revision.parts
    } else if let Some(candidate) = &item.active_candidate {
        &candidate.parts
    } else {
        &[]
    }
}

fn item_source(item: &TimelineItem) -> GroupCompanionMessageSource {
    GroupCompanionMessageSource {
        revision_id: item.active_revision.as_ref().map(|revision| revision.id),
        candidate_id: item.active_candidate.as_ref().map(|candidate| candidate.id),
    }
}

fn author_of(
    item: &TimelineItem,
    participants: &[ConversationParticipant],
    persona_id: Option<PersonaId>,
) -> GroupCompanionAuthor {
    if item.message.role == MessageRole::User {
        return GroupCompanionAuthor::Persona { persona_id };
    }
    item.message
        .author_participant_id
        .and_then(|author| {
            participants
                .iter()
                .find(|participant| participant.id == author)
        })
        .and_then(|participant| match participant.source {
            ParticipantSource::Character(character_id) => {
                Some(GroupCompanionAuthor::Member { character_id })
            }
            ParticipantSource::User | ParticipantSource::System => None,
        })
        .unwrap_or(GroupCompanionAuthor::Persona { persona_id })
}

fn addressed_members(
    text: &str,
    member_names: &[(CharacterId, String)],
    present: &[CharacterId],
    author: GroupCompanionAuthor,
    previous: Option<GroupCompanionAuthor>,
) -> Vec<CharacterId> {
    let named = member_names
        .iter()
        .filter(|(_, name)| mentions_name(text, name))
        .map(|(character_id, _)| *character_id)
        .collect::<Vec<_>>();
    if !named.is_empty() {
        return named;
    }
    match author {
        GroupCompanionAuthor::Persona { .. } => present.to_vec(),
        GroupCompanionAuthor::Member { .. } => match previous {
            Some(GroupCompanionAuthor::Member { character_id })
                if present.contains(&character_id) =>
            {
                vec![character_id]
            }
            _ => Vec::new(),
        },
    }
}

fn mentions_name(text: &str, name: &str) -> bool {
    let text = text.to_lowercase();
    let name = name.trim().to_lowercase();
    if name.is_empty() {
        return false;
    }
    let mut start = 0;
    while let Some(offset) = text[start..].find(&name) {
        let begin = start + offset;
        let end = begin + name.len();
        let before = begin == 0
            || text[..begin]
                .chars()
                .next_back()
                .is_none_or(|value| !is_word_character(value));
        let after = end == text.len()
            || text[end..]
                .chars()
                .next()
                .is_none_or(|value| !is_word_character(value));
        if before && after {
            return true;
        }
        start = end;
    }
    false
}

fn is_word_character(value: char) -> bool {
    value.is_alphanumeric() || value == '_'
}

fn repository_error(error: GroupCompanionRepositoryError) -> CompanionTurnError {
    CompanionTurnError::Conversation(ConversationRepositoryError::Storage {
        reason: format!("{error:?}"),
    })
}
